//! OxideForms — a small, self-hosted, file-driven form service.
//!
//! * Forms are plain `.json` files in the `FORMS_DIR` directory; each file's name
//!   is the form's UUID and therefore its route (`/<uuid>`).
//! * Submissions are stored in SQLite.
//! * Append `?admin=true` to a form URL to view responses behind an admin password
//!   (configured via `ADMIN_PASSWORD`).
//!
//! New forms are created by adding a `.json` file to the forms directory (the
//! file name is the form's UUID) — there is no CLI or API for creating forms.
//! The running server picks new/changed files up automatically.

mod auth;
mod db;
mod forms;
mod handlers;
mod i18n;
mod state;
mod templates;

use state::AppState;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".into())
        .parse()
        .unwrap_or(3000);
    let db_path = std::env::var("DB_PATH").unwrap_or_else(|_| "forms.db".into());
    let forms_dir = PathBuf::from(std::env::var("FORMS_DIR").unwrap_or_else(|_| "forms".into()));

    let admin_password = std::env::var("ADMIN_PASSWORD").ok().filter(|s| !s.trim().is_empty());
    let signing_key = admin_password.as_deref().map(auth::signing_key).unwrap_or_default();
    if admin_password.is_none() {
        tracing::warn!("ADMIN_PASSWORD is not set — the ?admin=true submissions view is disabled");
    }

    let db = db::open_db(&db_path);
    let (form_map, warnings) = forms::load_forms(&forms_dir);
    for w in &warnings {
        tracing::warn!("forms: {w}");
    }
    tracing::info!("loaded {} form(s) from {}", form_map.len(), forms_dir.display());

    let state = Arc::new(AppState {
        db,
        forms: tokio::sync::RwLock::new(form_map),
        admin_password,
        signing_key,
    });

    // Watch the forms directory and hot-reload when it changes.
    let watcher_state = state.clone();
    tokio::spawn(async move { forms_watcher(watcher_state, forms_dir).await });

    let app = handlers::router(state);

    let ip: IpAddr = host
        .parse()
        .unwrap_or_else(|_| IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)));
    let addr = SocketAddr::new(ip, port);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("OxideForms is listening on http://{ip}:{port}");

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("shutting down…");
        })
        .await?;

    Ok(())
}

/// Re-scan the forms directory every couple of seconds and swap in the new set if
/// any file was added, removed, or modified (by mtime).
async fn forms_watcher(state: Arc<AppState>, dir: PathBuf) {
    let mut last_sig = String::new();
    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;
        let sig = dir_signature(&dir);
        if sig != last_sig {
            last_sig = sig;
            let (map, warnings) = forms::load_forms(&dir);
            for w in &warnings {
                tracing::warn!("forms: {w}");
            }
            tracing::info!("reloaded {} form(s) from {}", map.len(), dir.display());
            *state.forms.write().await = map;
        }
    }
}

/// A cheap fingerprint of the directory: every `*.json` file path + its mtime.
fn dir_signature(dir: &std::path::Path) -> String {
    let mut parts = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let nanos = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            parts.push(format!("{}:{nanos}", path.display()));
        }
    }
    parts.sort();
    parts.join("\n")
}
