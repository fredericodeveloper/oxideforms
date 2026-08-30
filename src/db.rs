//! SQLite persistence for form submissions.

use rusqlite::{params, Connection};
use std::sync::Mutex;

/// A single persisted submission row.
pub struct RawSubmission {
    /// The submitted values as a JSON object keyed by field id.
    pub data: String,
    /// Human-readable timestamp captured at insert time.
    pub created_at: String,
}

/// A single `Connection` guarded by a mutex. All operations on it are fast and
/// short-lived, so a coarse lock is the simplest correct approach for this app.
pub type Db = Mutex<Connection>;

/// Open (or create) the database and ensure the schema exists.
pub fn open_db(path: &str) -> Db {
    // SQLite creates the file itself; make sure the parent directory exists first.
    if let Some(parent) = std::path::Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .expect("failed to create the database directory");
        }
    }
    let conn = Connection::open(path).expect("failed to open SQLite database");
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS submissions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            form_uuid TEXT NOT NULL,
            data TEXT NOT NULL,
            client_id TEXT,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_submissions_form ON submissions (form_uuid);",
    )
    .expect("failed to initialise schema");

    // Databases created before `client_id` existed are migrated in place.
    let has_client_id: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('submissions') WHERE name = 'client_id'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);
    if !has_client_id {
        conn.execute_batch(
            "ALTER TABLE submissions ADD COLUMN client_id TEXT;",
        )
        .expect("failed to migrate submissions table");
    }
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_submissions_form_client ON submissions (form_uuid, client_id);",
    )
    .expect("failed to initialise indexes");

    Mutex::new(conn)
}

/// Insert a submission and return its new row id.
pub fn insert_submission(
    db: &Db,
    form_uuid: &str,
    data: &str,
    client_id: &str,
    created_at: &str,
) -> Result<i64, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO submissions (form_uuid, data, client_id, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![form_uuid, data, client_id, created_at],
    )
    .map(|_| conn.last_insert_rowid())
    .map_err(|e| e.to_string())
}

/// Whether the given visitor has already submitted the form at least once.
pub fn client_has_submitted(db: &Db, form_uuid: &str, client_id: &str) -> Result<bool, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM submissions WHERE form_uuid = ?1 AND client_id = ?2)",
        params![form_uuid, client_id],
        |row| row.get(0),
    )
    .map_err(|e| e.to_string())
}

/// List a form's submissions, newest first.
pub fn list_submissions(db: &Db, form_uuid: &str) -> Result<Vec<RawSubmission>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT data, created_at
             FROM submissions WHERE form_uuid = ?1
             ORDER BY id DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![form_uuid], |row| {
            Ok(RawSubmission {
                data: row.get(0)?,
                created_at: row.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}
