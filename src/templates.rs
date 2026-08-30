//! Askama view structs — one per template under `templates/`.
//!
//! Fields are only "read" from within askama's generated `render` code, which the
//! dead-code lint can't see, so we silence those false positives for this module.
#![allow(dead_code)]

use askama::Template;
use std::collections::HashMap;

use crate::forms::{Field, FormDefinition, Page};
use crate::i18n::T;

/// One row in the submissions table: the answer per field, in field order.
pub struct Row {
    pub values: Vec<String>,
    pub created_at: String,
}

impl Row {
    pub fn values(&self) -> &[String] {
        &self.values
    }
}

/// The form filling-in page, showing exactly one page (section) of the form.
/// Every page carries `t` (the resolved language table) and `next` (where the
/// header language switcher should return the visitor to after switching).
#[derive(Template)]
#[template(path = "form.html", ext = "html")]
pub struct FormPage<'a> {
    pub form: &'a FormDefinition,
    /// 0-based index of the page (section) being shown/submitted.
    pub page_index: usize,
    pub errors: &'a [String],
    pub values: &'a HashMap<String, Vec<String>>,
    pub t: T,
    pub next: &'a str,
}

impl<'a> FormPage<'a> {
    /// The page (section) being shown/submitted.
    pub fn current_page(&self) -> &Page {
        self.form.page(self.page_index)
    }

    pub fn is_last_page(&self) -> bool {
        self.page_index >= self.form.page_count().saturating_sub(1)
    }

    pub fn page_number(&self) -> usize {
        self.page_index + 1
    }

    pub fn page_count(&self) -> usize {
        self.form.page_count()
    }

    /// `0..page_count()` — the progress bar's segments.
    pub fn indices(&self) -> Vec<usize> {
        (0..self.page_count()).collect()
    }

    pub fn multi_page(&self) -> bool {
        self.form.page_count() > 1
    }

    /// Where the page's form posts: validate this page and render the next one,
    /// or the form root for the final page (and single-page forms), which
    /// performs the actual submit.
    pub fn action(&self) -> String {
        if self.is_last_page() {
            format!("/{}", self.form.uuid)
        } else {
            format!("/{}/page/{}/next", self.form.uuid, self.page_index)
        }
    }

    /// Where the “Back” form posts: re-render the previous page.
    pub fn back_action(&self) -> String {
        format!(
            "/{}/page/{}/back",
            self.form.uuid,
            self.page_index.saturating_sub(1)
        )
    }

    /// A textarea's placeholder: the field's own, or a localized default.
    pub fn textarea_placeholder(&self, field: &Field) -> String {
        let p = field.placeholder();
        if p.is_empty() {
            self.t.default_textarea_placeholder.to_string()
        } else {
            p.to_string()
        }
    }

    /// The pages that precede the current one — rendered as hidden inputs so a
    /// later POST carries their answers along.
    pub fn pages_before(&self) -> &[Page] {
        &self.form.pages()[..self.page_index.min(self.form.page_count())]
    }
}

#[derive(Template)]
#[template(path = "form_success.html", ext = "html")]
pub struct SuccessPage<'a> {
    pub form: &'a FormDefinition,
    pub t: T,
    pub next: &'a str,
}

#[derive(Template)]
#[template(path = "admin_login.html", ext = "html")]
pub struct AdminLoginPage<'a> {
    pub form: &'a FormDefinition,
    pub error: Option<&'a str>,
    pub t: T,
    pub next: &'a str,
}

#[derive(Template)]
#[template(path = "submissions.html", ext = "html")]
pub struct SubmissionsPage<'a> {
    pub form: &'a FormDefinition,
    pub columns: &'a [String],
    pub rows: &'a [Row],
    pub t: T,
    pub next: &'a str,
}

#[derive(Template)]
#[template(path = "not_found.html", ext = "html")]
pub struct NotFoundPage<'a> {
    pub uuid: &'a str,
    pub t: T,
    pub next: &'a str,
}
