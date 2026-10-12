//! Repository for promotion records (T04).

use rusqlite::{params, Connection, OptionalExtension};
use vela_domain::errors::VelaError;

use crate::writer::store_error;

/// How a run was handed over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Promotion {
    /// The run.
    pub run_id: String,
    /// `PULL_REQUEST`, `LOCAL_ONLY` or `AUTO_MERGE`.
    pub mode: String,
    /// `NONE`, `DRAFT`, `READY`, `LOCAL_READY` or `MERGED`.
    pub state: String,
    /// The integration branch.
    pub branch: String,
    /// The branch it targets.
    pub base_branch: String,
    /// The pull request, when there is one.
    pub pr_url: Option<String>,
    /// A short explanation for the user.
    pub note: String,
    /// When it last changed.
    pub updated_ms: i64,
}

/// Writes (or replaces) the run's promotion record.
pub fn upsert(conn: &Connection, p: &Promotion) -> Result<(), VelaError> {
    conn.execute(
        "INSERT INTO promotions (run_id, mode, state, branch, base_branch, pr_url, note, updated_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(run_id) DO UPDATE SET mode = ?2, state = ?3, branch = ?4, base_branch = ?5, pr_url = ?6,
             note = ?7, updated_ms = ?8",
        params![p.run_id, p.mode, p.state, p.branch, p.base_branch, p.pr_url, p.note, p.updated_ms],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// The run's promotion record.
pub fn get(conn: &Connection, run_id: &str) -> Result<Option<Promotion>, VelaError> {
    conn.query_row(
        "SELECT run_id, mode, state, branch, base_branch, pr_url, note, updated_ms FROM promotions WHERE run_id = ?1",
        [run_id],
        |r| {
            Ok(Promotion {
                run_id: r.get(0)?,
                mode: r.get(1)?,
                state: r.get(2)?,
                branch: r.get(3)?,
                base_branch: r.get(4)?,
                pr_url: r.get(5)?,
                note: r.get(6)?,
                updated_ms: r.get(7)?,
            })
        },
    )
    .optional()
    .map_err(store_error)
}
