//! Shared application state, passed to handlers via axum's `State` extractor.

use crate::db::Db;
use crate::forms::FormDefinition;
use std::collections::HashMap;

pub struct AppState {
    pub db: Db,
    /// Live set of forms (`uuid -> definition`), swapped out when the forms
    /// directory changes on disk.
    pub forms: tokio::sync::RwLock<HashMap<String, FormDefinition>>,
    /// The configured admin password, if any. `None` disables the admin view.
    pub admin_password: Option<String>,
    /// HMAC signing key derived from the admin password (empty when disabled).
    pub signing_key: Vec<u8>,
}
