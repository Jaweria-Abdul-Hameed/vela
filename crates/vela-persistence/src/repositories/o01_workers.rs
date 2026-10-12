//! Repository for workers (O01): one row per ticket execution, holding the serialized worker state.

use rusqlite::{params, Connection, OptionalExtension};
use vela_domain::errors::VelaError;

use crate::writer::store_error;

/// A worker row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerRow {
    /// Identifier.
    pub id: String,
    /// The run.
    pub run_id: String,
    /// The ticket.
    pub ticket_id: String,
    /// The serialized `WorkerState`.
    pub state: String,
    /// The approval-wait overlay.
    pub waiting_approval: bool,
    /// The conversation id, when one was started.
    pub conversation_id: Option<String>,
    /// The worker branch.
    pub branch: Option<String>,
    /// The worker worktree.
    pub worktree_path: Option<String>,
    /// The commit the worker started from.
    pub base_sha: Option<String>,
}

const COLUMNS: &str = "id, run_id, ticket_id, state, waiting_approval, conversation_id, branch, worktree_path, base_sha";

fn from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<WorkerRow> {
    Ok(WorkerRow {
        id: r.get(0)?,
        run_id: r.get(1)?,
        ticket_id: r.get(2)?,
        state: r.get(3)?,
        waiting_approval: r.get::<_, i64>(4)? != 0,
        conversation_id: r.get(5)?,
        branch: r.get(6)?,
        worktree_path: r.get(7)?,
        base_sha: r.get(8)?,
    })
}

/// Inserts a worker.
pub fn insert(conn: &Connection, id: &str, run_id: &str, ticket_id: &str, state: &str, now_ms: i64) -> Result<(), VelaError> {
    conn.execute(
        "INSERT INTO workers (id, run_id, ticket_id, state, created_ms, updated_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![id, run_id, ticket_id, state, now_ms],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// Sets the state and the approval overlay.
pub fn set_state(conn: &Connection, id: &str, state: &str, waiting_approval: bool, now_ms: i64) -> Result<(), VelaError> {
    conn.execute(
        "UPDATE workers SET state = ?1, waiting_approval = ?2, updated_ms = ?3 WHERE id = ?4",
        params![state, i64::from(waiting_approval), now_ms, id],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// Records where the worker works.
pub fn set_location(
    conn: &Connection,
    id: &str,
    branch: &str,
    worktree_path: &str,
    base_sha: &str,
    now_ms: i64,
) -> Result<(), VelaError> {
    conn.execute(
        "UPDATE workers SET branch = ?1, worktree_path = ?2, base_sha = ?3, updated_ms = ?4 WHERE id = ?5",
        params![branch, worktree_path, base_sha, now_ms, id],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// Records the conversation id.
pub fn set_conversation(conn: &Connection, id: &str, conversation_id: &str, now_ms: i64) -> Result<(), VelaError> {
    conn.execute(
        "UPDATE workers SET conversation_id = ?1, updated_ms = ?2 WHERE id = ?3",
        params![conversation_id, now_ms, id],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// A worker by id.
pub fn get(conn: &Connection, id: &str) -> Result<Option<WorkerRow>, VelaError> {
    conn.query_row(&format!("SELECT {COLUMNS} FROM workers WHERE id = ?1"), [id], from_row)
        .optional()
        .map_err(store_error)
}

/// Every worker of a run, oldest first.
pub fn list_for_run(conn: &Connection, run_id: &str) -> Result<Vec<WorkerRow>, VelaError> {
    let mut stmt = conn
        .prepare(&format!("SELECT {COLUMNS} FROM workers WHERE run_id = ?1 ORDER BY created_ms, id"))
        .map_err(store_error)?;
    let rows = stmt.query_map([run_id], from_row).map_err(store_error)?;
    rows.collect::<Result<_, _>>().map_err(store_error)
}
