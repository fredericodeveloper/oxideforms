//! Axum HTTP handlers and route table.

use crate::auth::{check_password, make_token, now_secs, verify_token};
use crate::db::{self, RawSubmission};
use crate::forms::{build_data, format_value, validate_fields, validate_submission, FormDefinition};
use crate::i18n::{resolve_lang, Lang, T};
use crate::state::AppState;
use crate::templates::{AdminLoginPage, FormPage, NotFoundPage, Row, SuccessPage, SubmissionsPage};
use askama::Template;
use axum::body::to_bytes;
use axum::extract::{Form, Path, Query, Request, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, COOKIE, LOCATION, SET_COOKIE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

const ADMIN_COOKIE: &str = "forms_admin";
/// The cookie that remembers a visitor's preferred UI language.
const LANG_COOKIE: &str = "oxideforms_lang";
/// The cookie that ties a visitor to their submissions (used to enforce
/// single-submission forms; anonymous visitors have no account to rely on).
const CLIENT_ID_COOKIE: &str = "oxideforms_cid";
/// How long a valid admin session lasts.
const SESSION_TTL: u32 = 8 * 60 * 60; // 8 hours
/// How long the language preference is remembered.
const LANG_TTL: u32 = 60 * 60 * 24 * 365; // 1 year
/// How long the visitor identity is remembered — effectively permanent, so a
/// "single submission" stays single across browser restarts.
const CLIENT_ID_TTL: u32 = 60 * 60 * 24 * 365 * 10; // 10 years
/// Upper bound on the size of a form submission body we'll accept.
const MAX_BODY: usize = 5 * 1024 * 1024;
/// The compiled Tailwind bundle, embedded at build time so the Docker image
/// stays a single binary with no runtime static-file directory.
const TAILWIND_CSS: &str = include_str!("../static/css/tailwind.css");

#[derive(Deserialize)]
struct AuthForm {
    password: String,
}

#[derive(Deserialize)]
struct SetLangParams {
    #[serde(default)]
    lang: String,
    #[serde(default)]
    next: String,
}

/// Build the route table.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/healthz", get(health))
        .route("/lang", get(set_lang))
        .route("/static/css/tailwind.css", get(tailwind_css))
        .route("/{uuid}", get(get_form).post(submit_form))
        .route("/{uuid}/page/{n}/{dir}", post(page_nav))
        .route("/{uuid}/submitted", get(submitted))
        .route("/{uuid}/admin", get(admin_gate))
        .route("/{uuid}/admin/auth", post(admin_auth))
        .route("/{uuid}/admin/logout", get(admin_logout))
        .with_state(state)
}

async fn health() -> StatusCode {
    StatusCode::OK
}

/// Serve the compiled Tailwind bundle with aggressive caching: the content is
/// embedded in the binary, so a new version ships with a new image.
async fn tailwind_css() -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "text/css; charset=utf-8".parse().unwrap());
    headers.insert(CACHE_CONTROL, "public, max-age=31536000, immutable".parse().unwrap());
    (headers, TAILWIND_CSS).into_response()
}

/// Render an askama page to a string, converting a (rare) template failure into
/// a small error page instead of panicking.
fn render_page<T: Template>(page: &T) -> String {
    page.render().unwrap_or_else(|e| {
        eprintln!("askama render error: {e}");
        "<!doctype html><html><head><meta charset='utf-8'/></head><body style='font-family:system-ui;background:#0a0b10;color:#e2e8f0;display:grid;place-items:center;min-height:100vh;margin:0'><div style='text-align:center'><h1 style='margin:0'>Template error</h1><p style='color:#94a3b8'>Something went wrong rendering this page.</p></div></body></html>"
            .to_string()
    })
}

/// Build a redirect response with an explicit status (e.g. 303 See Other).
fn redirect(status: StatusCode, url: &str) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(LOCATION, url.parse().expect("valid location"));
    (status, headers).into_response()
}

/// Pull the value of a single named cookie out of the request headers.
fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    for value in headers.get_all(COOKIE) {
        let Ok(s) = value.to_str() else {
            continue;
        };
        for part in s.split(';') {
            let part = part.trim();
            if let Some((k, v)) = part.split_once('=') {
                if k == name {
                    return Some(v);
                }
            }
        }
    }
    None
}

/// Resolve the UI language for a request: the remembered language cookie wins,
/// then `Accept-Language`, then the English default.
fn lang_from_headers(headers: &HeaderMap) -> Lang {
    let cookie = cookie_value(headers, LANG_COOKIE);
    let accept = headers.get("accept-language").and_then(|v| v.to_str().ok());
    resolve_lang(cookie, accept)
}

/// Parse a `application/x-www-form-urlencoded` payload (a body or a query
/// string) into a multi-value map, preserving repeated keys (checkbox groups).
fn parse_pairs(raw: &str) -> HashMap<String, Vec<String>> {
    let mut values: HashMap<String, Vec<String>> = HashMap::new();
    for (k, v) in form_urlencoded::parse(raw.as_bytes()) {
        values.entry(k.into_owned()).or_default().push(v.into_owned());
    }
    values
}

/// The `?page=` parameter, parsed and clamped to the form's page range.
fn page_param(values: &HashMap<String, Vec<String>>, form: &FormDefinition) -> usize {
    let raw = values
        .get("page")
        .and_then(|v| v.first())
        .map(String::as_str)
        .unwrap_or("");
    let n: usize = raw.trim().parse().unwrap_or(0);
    n.min(form.page_count().saturating_sub(1))
}

/// Only allow relative, same-origin `next` targets so the language switcher can
/// never turn into an open redirect.
fn sanitize_next(next: &str) -> String {
    let trimmed = next.trim();
    if trimmed.starts_with('/') && !trimmed.starts_with("//") {
        trimmed.to_string()
    } else {
        "/".to_string()
    }
}

/// The `Set-Cookie` header value that grants (or revokes, with a zero TTL) admin.
fn admin_cookie_header(token: &str, max_age: u32) -> axum::http::HeaderValue {
    format!(
        "{ADMIN_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}"
    )
    .parse()
    .expect("cookie header value")
}

/// The `Set-Cookie` header value that binds a visitor identity to the browser.
fn client_id_header(value: &str) -> axum::http::HeaderValue {
    format!(
        "{CLIENT_ID_COOKIE}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={CLIENT_ID_TTL}"
    )
    .parse()
    .expect("cookie header value")
}

/// The visitor's persistent client id: the remembered cookie when present, or a
/// freshly minted one otherwise. Returns `(id, was_freshly_minted)`.
fn client_id(headers: &HeaderMap) -> (String, bool) {
    match cookie_value(headers, CLIENT_ID_COOKIE) {
        Some(v) if !v.trim().is_empty() => (v.to_string(), false),
        _ => (uuid::Uuid::new_v4().to_string(), true),
    }
}

fn not_found(uuid: &str, t: T) -> Response {
    let next = format!("/{uuid}");
    let page = NotFoundPage {
        uuid,
        t,
        next: &next,
    };
    (StatusCode::NOT_FOUND, Html(render_page(&page))).into_response()
}

fn db_error_response(msg: &str) -> Response {
    let html = format!(
        "<!doctype html><html><head><meta charset='utf-8'/></head><body style='font-family:system-ui;background:#0a0b10;color:#e2e8f0;display:grid;place-items:center;min-height:100vh;margin:0'><div style='text-align:center'><h1 style='font-size:1.5rem;margin:0'>Internal error</h1><p style='color:#94a3b8'>{msg}</p></div></body></html>"
    );
    (StatusCode::INTERNAL_SERVER_ERROR, Html(html)).into_response()
}

/// The project's GitHub repository — where the root path (and a few links) point.
const GITHUB_URL: &str = "https://github.com/fredericodeveloper/oxideforms";

/// The site root no longer lists forms; it redirects to the project on GitHub.
async fn root() -> Response {
    redirect(StatusCode::FOUND, GITHUB_URL)
}

/// Choose the UI language: remember the pick in a cookie, then bounce back to
/// the page the visitor came from (the header switcher's hidden `next` field).
async fn set_lang(Query(params): Query<SetLangParams>) -> Response {
    let next = sanitize_next(&params.next);
    let mut resp = redirect(StatusCode::SEE_OTHER, &next);
    if let Some(lang) = Lang::parse(&params.lang) {
        let cookie = format!(
            "{LANG_COOKIE}={}; Path=/; SameSite=Lax; Max-Age={}",
            lang.code(),
            LANG_TTL
        );
        resp.headers_mut()
            .insert(SET_COOKIE, cookie.parse().expect("cookie header value"));
    }
    resp
}

/// Render a form page for filling in, or (with `?admin=true`) route into the
/// admin view. The `?page=N` parameter picks the section; a value-less GET can
/// still carry answered fields in the query string (used by the “Back” link).
async fn get_form(
    State(state): State<Arc<AppState>>,
    Path(uuid): Path<String>,
    req: Request,
) -> Response {
    let headers = req.headers().clone();
    let t = T::for_lang(lang_from_headers(&headers));
    let values = parse_pairs(req.uri().query().unwrap_or(""));
    let admin = values
        .get("admin")
        .and_then(|v| v.first())
        .map(|v| v == "true")
        .unwrap_or(false);

    if admin {
        return admin_gate_inner(&state, &uuid, &headers, None).await;
    }

    let form = match state.forms.read().await.get(&uuid).cloned() {
        Some(f) => f,
        None => return not_found(&uuid, t),
    };

    let page_index = page_param(&values, &form);

    // Single-submission forms: a visitor who has already answered is bounced
    // straight to the thank-you page instead of seeing the form again.
    let (cid, fresh) = if form.single_submission {
        let (cid, fresh) = client_id(&headers);
        let already = db::client_has_submitted(&state.db, &uuid, &cid).unwrap_or(false);
        if already {
            let mut resp = redirect(StatusCode::SEE_OTHER, &format!("/{uuid}/submitted"));
            if fresh {
                resp.headers_mut()
                    .insert(SET_COOKIE, client_id_header(&cid));
            }
            return resp;
        }
        (cid, fresh)
    } else {
        client_id(&headers)
    };

    let next = if form.page_count() > 1 && page_index > 0 {
        format!("/{uuid}?page={page_index}")
    } else {
        format!("/{uuid}")
    };
    let page = FormPage {
        form: &form,
        page_index,
        errors: &[],
        values: &values,
        t,
        next: &next,
    };
    let mut resp = Html(render_page(&page)).into_response();
    // For single-submission forms, remember the visitor from the first visit so
    // the later submission (and any repeat attempts) can be attributed to them.
    if form.single_submission && fresh {
        resp.headers_mut()
            .insert(SET_COOKIE, client_id_header(&cid));
    }
    resp
}

/// The final step of any form (single-page, or the last page of a multi-page
/// one): validate everything, persist, and redirect to the thank-you page.
/// On validation failure the page that was posted is re-rendered with the errors.
fn perform_full_submit(
    state: &AppState,
    form: &FormDefinition,
    values: &HashMap<String, Vec<String>>,
    t: T,
    headers: &HeaderMap,
) -> Response {
    let page_index = page_param(values, form);

    let errors = validate_submission(form, values, &t);
    if !errors.is_empty() {
        let next = format!("/{}", form.uuid);
        let page = FormPage {
            form,
            page_index,
            errors: &errors,
            values,
            t,
            next: &next,
        };
        return (StatusCode::UNPROCESSABLE_ENTITY, Html(render_page(&page))).into_response();
    }

    // Single-submission forms: reject a repeat answer from a known visitor.
    let (cid, fresh) = client_id(headers);
    if form.single_submission {
        match db::client_has_submitted(&state.db, &form.uuid, &cid) {
            Ok(true) => {
                let mut errors = Vec::new();
                errors.push(t.already_submitted.to_string());
                let next = format!("/{}", form.uuid);
                let page = FormPage {
                    form,
                    page_index,
                    errors: &errors,
                    values,
                    t,
                    next: &next,
                };
                return (StatusCode::UNPROCESSABLE_ENTITY, Html(render_page(&page))).into_response();
            }
            Ok(false) => {}
            Err(err) => return db_error_response(&err),
        }
    }

    let data = build_data(form, values);
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    if let Err(err) =
        db::insert_submission(&state.db, &form.uuid, &data.to_string(), &cid, &now)
    {
        return db_error_response(&err);
    }

    let mut resp = redirect(StatusCode::SEE_OTHER, &format!("/{}/submitted", form.uuid));
    if fresh {
        // First time we can tell this visitor apart — remember them.
        resp.headers_mut()
            .insert(SET_COOKIE, client_id_header(&cid));
    }
    resp
}

/// Accept a form submission, validate it, persist it, and redirect to a
/// thank-you page. For multi-page forms this is the last page's target; the
/// posted page number (hidden field) decides where errors are re-rendered.
async fn submit_form(
    State(state): State<Arc<AppState>>,
    Path(uuid): Path<String>,
    req: Request,
) -> Response {
    let headers = req.headers().clone();
    let t = T::for_lang(lang_from_headers(&headers));

    let form = match state.forms.read().await.get(&uuid).cloned() {
        Some(f) => f,
        None => return not_found(&uuid, t),
    };

    let body = match to_bytes(req.into_body(), MAX_BODY).await {
        Ok(b) => b,
        Err(_) => return StatusCode::PAYLOAD_TOO_LARGE.into_response(),
    };

    let values = parse_pairs(&String::from_utf8_lossy(&body));
    perform_full_submit(&state, &form, &values, t, &headers)
}

/// Multi-page navigation, POST-only: the browser never sees a GET with values
/// in the URL. `dir` is `next` (validate page n, respond with page n+1) or
/// `back` (re-validate page n from the snapshot form, respond with page n).
/// Posting the last page is the full submit.
async fn page_nav(
    State(state): State<Arc<AppState>>,
    Path((uuid, n, dir)): Path<(String, usize, String)>,
    req: Request,
) -> Response {
    let headers = req.headers().clone();
    let t = T::for_lang(lang_from_headers(&headers));

    let form = match state.forms.read().await.get(&uuid).cloned() {
        Some(f) => f,
        None => return not_found(&uuid, t),
    };

    if dir != "next" && dir != "back" {
        return not_found(&uuid, t);
    }
    let n = n.min(form.page_count().saturating_sub(1));
    let is_back = dir == "back";

    let body = match to_bytes(req.into_body(), MAX_BODY).await {
        Ok(b) => b,
        Err(_) => return StatusCode::PAYLOAD_TOO_LARGE.into_response(),
    };
    let values = parse_pairs(&String::from_utf8_lossy(&body));

    if n == form.page_count() - 1 && !is_back {
        return perform_full_submit(&state, &form, &values, t, &headers);
    }

    let errors = validate_fields(form.page(n).fields(), &values, &t);
    if !errors.is_empty() {
        let next = format!("/{}?page={n}", form.uuid);
        let page = FormPage {
            form: &form,
            page_index: n,
            errors: &errors,
            values: &values,
            t,
            next: &next,
        };
        return (StatusCode::UNPROCESSABLE_ENTITY, Html(render_page(&page))).into_response();
    }

    let target = (if is_back { n } else { n + 1 }).min(form.page_count().saturating_sub(1));
    let next = format!("/{}?page={target}", form.uuid);
    let page = FormPage {
        form: &form,
        page_index: target,
        errors: &[],
        values: &values,
        t,
        next: &next,
    };
    Html(render_page(&page)).into_response()
}

/// Thank-you page after a successful submission.
async fn submitted(
    State(state): State<Arc<AppState>>,
    Path(uuid): Path<String>,
    headers: HeaderMap,
) -> Response {
    let t = T::for_lang(lang_from_headers(&headers));
    match state.forms.read().await.get(&uuid).cloned() {
        Some(form) => {
            let next = format!("/{uuid}/submitted");
            let page = SuccessPage {
                form: &form,
                t,
                next: &next,
            };
            Html(render_page(&page)).into_response()
        }
        None => not_found(&uuid, t),
    }
}

/// `GET /{uuid}/admin` — convenience path equivalent to `?admin=true`.
async fn admin_gate(
    State(state): State<Arc<AppState>>,
    Path(uuid): Path<String>,
    headers: HeaderMap,
) -> Response {
    admin_gate_inner(&state, &uuid, &headers, None).await
}

/// Shared admin logic: show submissions when the cookie is valid, otherwise the
/// password prompt (or an "admin not configured" notice).
async fn admin_gate_inner(
    state: &AppState,
    uuid: &str,
    headers: &HeaderMap,
    login_error: Option<&str>,
) -> Response {
    let t = T::for_lang(lang_from_headers(headers));
    let next = format!("/{uuid}?admin=true");

    let form = match state.forms.read().await.get(uuid).cloned() {
        Some(f) => f,
        None => return not_found(uuid, t),
    };

    if state.admin_password.is_none() {
        let page = AdminLoginPage {
            form: &form,
            error: Some(t.admin_not_configured_view),
            t,
            next: &next,
        };
        return (StatusCode::SERVICE_UNAVAILABLE, Html(render_page(&page))).into_response();
    }

    if let Some(token) = cookie_value(headers, ADMIN_COOKIE) {
        if verify_token(&state.signing_key, token) {
            let raws = match db::list_submissions(&state.db, uuid) {
                Ok(r) => r,
                Err(err) => return db_error_response(&err),
            };
            let (columns, rows) = build_submission_view(&form, raws);
            let page = SubmissionsPage {
                form: &form,
                columns: &columns,
                rows: &rows,
                t,
                next: &next,
            };
            return Html(render_page(&page)).into_response();
        }
    }

    let page = AdminLoginPage {
        form: &form,
        error: login_error,
        t,
        next: &next,
    };
    Html(render_page(&page)).into_response()
}

/// Handle the admin password form. On success, set a signed session cookie and
/// redirect back to the admin view (which will now show the submissions).
async fn admin_auth(
    State(state): State<Arc<AppState>>,
    Path(uuid): Path<String>,
    headers: HeaderMap,
    form: Form<AuthForm>,
) -> Response {
    let t = T::for_lang(lang_from_headers(&headers));
    let next = format!("/{uuid}?admin=true");

    let form_def = match state.forms.read().await.get(&uuid).cloned() {
        Some(f) => f,
        None => return not_found(&uuid, t),
    };

    let Some(admin_pw) = &state.admin_password else {
        let page = AdminLoginPage {
            form: &form_def,
            error: Some(t.admin_not_configured),
            t,
            next: &next,
        };
        return (StatusCode::SERVICE_UNAVAILABLE, Html(render_page(&page))).into_response();
    };

    if check_password(admin_pw, &form.password) {
        let expiry = now_secs() + SESSION_TTL as i64;
        let token = make_token(&state.signing_key, expiry);
        let mut resp = redirect(StatusCode::SEE_OTHER, &format!("/{uuid}?admin=true"));
        resp.headers_mut()
            .insert(SET_COOKIE, admin_cookie_header(&token, SESSION_TTL));
        resp
    } else {
        let page = AdminLoginPage {
            form: &form_def,
            error: Some(t.wrong_password),
            t,
            next: &next,
        };
        Html(render_page(&page)).into_response()
    }
}

/// Clear the admin session cookie and go back to the public form.
async fn admin_logout(Path(uuid): Path<String>) -> Response {
    let mut resp = redirect(StatusCode::SEE_OTHER, &format!("/{uuid}"));
    resp.headers_mut()
        .insert(SET_COOKIE, admin_cookie_header("", 0));
    resp
}

/// Turn raw submissions into the (columns, rows) shape the table template expects.
fn build_submission_view(form: &FormDefinition, raws: Vec<RawSubmission>) -> (Vec<String>, Vec<Row>) {
    let columns = form
        .fields()
        .iter()
        .map(|f| {
            if f.label.trim().is_empty() {
                f.id.clone()
            } else {
                f.label.clone()
            }
        })
        .collect();

    let rows = raws
        .iter()
        .map(|r| {
            let data: serde_json::Value =
                serde_json::from_str(&r.data).unwrap_or(serde_json::Value::Null);
            let values = form.fields().iter().map(|f| format_value(f, &data)).collect();
            Row {
                values,
                created_at: r.created_at.clone(),
            }
        })
        .collect();

    (columns, rows)
}
