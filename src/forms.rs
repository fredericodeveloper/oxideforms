//! Form definitions: loaded from `.json` files on disk.
//!
//! Each form is a single file inside the configured forms directory. The file's
//! name (e.g. `8f14e45f-....json`) is the form's UUID and therefore its route,
//! unless the JSON itself declares a `"uuid"` field, which takes precedence.

use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::Path;

use crate::i18n::T;

/// One question on a form.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Field {
    /// Stable identifier; used as the HTML `name` and as the key in stored data.
    pub id: String,

    /// Input kind: `text | email | number | url | date | textarea | select | radio | checkbox`.
    /// Unknown values are treated as plain text at render time.
    #[serde(rename = "type", default)]
    pub kind: String,

    #[serde(default)]
    pub label: String,

    #[serde(default)]
    pub required: bool,

    /// Choices for `select` / `radio` / `checkbox` fields.
    #[serde(default)]
    options: Vec<String>,

    #[serde(default)]
    pub placeholder: Option<String>,
}

#[allow(dead_code)]
impl Field {
    /// Normalised input kind (unknown values fall back to plain text).
    pub fn input_kind(&self) -> &'static str {
        match self.kind.as_str() {
            "email" => "email",
            "number" => "number",
            "url" => "url",
            "date" => "date",
            "textarea" => "textarea",
            "select" => "select",
            "radio" => "radio",
            "checkbox" => "checkbox",
            _ => "text",
        }
    }

    pub fn options(&self) -> &[String] {
        &self.options
    }

    /// Whether this field can carry more than one value (checkbox groups).
    pub fn is_multi(&self) -> bool {
        self.input_kind() == "checkbox"
    }

    pub fn placeholder(&self) -> &str {
        self.placeholder.as_deref().unwrap_or("")
    }

    /// First submitted value for this field ("" when absent) — used to re-populate
    /// the form after a validation error.
    pub fn prefill(&self, values: &HashMap<String, Vec<String>>) -> String {
        values
            .get(&self.id)
            .and_then(|v| v.first().cloned())
            .unwrap_or_default()
    }

    /// Every submitted value for this field (empty when absent) — used to build
    /// the hidden inputs that carry earlier pages' answers through a multi-page form.
    pub fn all_values<'a>(
        &self,
        values: &'a HashMap<String, Vec<String>>,
    ) -> &'a [String] {
        values.get(&self.id).map(Vec::as_slice).unwrap_or_default()
    }

    /// Whether a specific option is selected/checked in the given submission.
    pub fn is_checked(&self, values: &HashMap<String, Vec<String>>, option: &str) -> bool {
        values
            .get(&self.id)
            .map(|v| v.iter().any(|x| x == option))
            .unwrap_or(false)
    }
}

/// One page (section) of a multi-page form: shown one at a time with
/// back/next navigation.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Page {
    pub title: Option<String>,
    pub description: Option<String>,
    fields: Vec<Field>,
}

impl Page {
    pub fn new(title: Option<String>, description: Option<String>, fields: Vec<Field>) -> Self {
        Self {
            title,
            description,
            fields,
        }
    }

    pub fn fields(&self) -> &[Field] {
        &self.fields
    }
}

/// A fully-resolved form ready to be rendered or stored against.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FormDefinition {
    pub uuid: String,
    pub title: String,
    pub description: Option<String>,
    /// When `true`, a visitor may submit this form only once (see `handlers`).
    pub single_submission: bool,
    pages: Vec<Page>,
}

impl FormDefinition {
    pub fn new(
        uuid: String,
        title: String,
        description: Option<String>,
        single_submission: bool,
        pages: Vec<Page>,
    ) -> Self {
        Self {
            uuid,
            title,
            description,
            single_submission,
            pages,
        }
    }

    /// All pages, in order. A form without explicit `pages` has a single page.
    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// The page at `i`, clamped to the valid range (safe for a form with one page).
    pub fn page(&self, i: usize) -> &Page {
        &self.pages[i.min(self.page_count().saturating_sub(1))]
    }

    /// Every field of every page, in order. Validation, persistence and the admin
    /// table are page-agnostic, so they all work off this flattened view.
    pub fn fields(&self) -> Vec<Field> {
        self.pages
            .iter()
            .flat_map(|p| p.fields().to_vec())
            .collect()
    }

    /// Whether the form has at least one required field (drives the “required” hint).
    pub fn has_required(&self) -> bool {
        self.pages
            .iter()
            .any(|p| p.fields().iter().any(|f| f.required))
    }
}

/// Raw on-disk shape of a form definition file.
#[derive(Deserialize)]
struct FormJson {
    #[serde(default)]
    uuid: Option<String>,
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    single_submission: bool,
    #[serde(default)]
    fields: Vec<Field>,
    /// Optional multi-page layout. When present it
    /// takes precedence over a flat `fields` list.
    #[serde(default)]
    pages: Option<Vec<PageJson>>,
}

/// Raw on-disk shape of one page inside `pages`.
#[derive(Deserialize)]
struct PageJson {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    fields: Vec<Field>,
}


/// Load every `*.json` file in `dir` into a `uuid -> FormDefinition` map.
/// Returns the map plus human-readable warnings for files that could not be used.
pub fn load_forms(dir: &Path) -> (HashMap<String, FormDefinition>, Vec<String>) {
    let mut map = HashMap::new();
    let mut warnings = Vec::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(err) => {
            warnings.push(format!("forms dir '{}' not readable: {err}", dir.display()));
            return (map, warnings);
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(err) => {
                warnings.push(format!("{}: {err}", path.display()));
                continue;
            }
        };

        match serde_json::from_str::<FormJson>(&content) {
            Ok(fj) => {
                let uuid = fj
                    .uuid
                    .clone()
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or(stem.clone());
                let title = if fj.title.trim().is_empty() {
                    stem
                } else {
                    fj.title
                };
                // `pages` (multi-page sections) takes precedence over a
                // flat `fields` list, which becomes a single-page form.
                let pages = match fj.pages {
                    Some(ps) if !ps.is_empty() => ps
                        .iter()
                        .map(|p| {
                            Page::new(p.title.clone(), p.description.clone(), p.fields.clone())
                        })
                        .collect(),
                    _ => vec![Page::new(None, None, fj.fields)],
                };
                let def = FormDefinition::new(uuid, title, fj.description, fj.single_submission, pages);
                map.insert(def.uuid.clone(), def);
            }
            Err(err) => warnings.push(format!("{}: {err}", path.display())),
        }
    }

    (map, warnings)
}

/// Validate a set of fields against the submitted values.
/// Returns a list of human-readable error messages (empty when valid). The
/// messages are rendered in the language given by `t` (the field label itself
/// is form content and is left as written in the `.json` file).
pub fn validate_fields(
    fields: &[Field],
    values: &HashMap<String, Vec<String>>,
    t: &T,
) -> Vec<String> {
    let mut errors = Vec::new();
    for field in fields {
        if !field.required {
            continue;
        }
        let present = values
            .get(&field.id)
            .map(|v| v.iter().any(|x| !x.trim().is_empty()))
            .unwrap_or(false);
        if !present {
            let label = if field.label.trim().is_empty() {
                field.id.clone()
            } else {
                field.label.clone()
            };
            errors.push(t.required_error(&label));
        }
    }
    errors
}

/// Validate a parsed submission against all of the form's required fields.
pub fn validate_submission(
    form: &FormDefinition,
    values: &HashMap<String, Vec<String>>,
    t: &T,
) -> Vec<String> {
    validate_fields(&form.fields(), values, t)
}

/// Collapse a parsed submission into the JSON object that gets persisted.
/// Single-value fields are stored as strings, checkbox groups as arrays.
pub fn build_data(form: &FormDefinition, values: &HashMap<String, Vec<String>>) -> Value {
    let mut obj = Map::new();
    for field in form.fields() {
        let vs = values.get(&field.id).cloned().unwrap_or_default();
        let value: Value = if field.is_multi() {
            Value::Array(vs.into_iter().map(Value::String).collect())
        } else {
            Value::String(vs.into_iter().next().unwrap_or_default())
        };
        obj.insert(field.id.clone(), value);
    }
    Value::Object(obj)
}

/// Render a persisted value for a field in a human-friendly string (for the submissions table).
pub fn format_value(field: &Field, data: &Value) -> String {
    match data.get(&field.id) {
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|x| x.as_str())
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join(", "),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}
