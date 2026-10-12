//! Repository for operation records (R01): intent before, result after, and the recipe that reconciles it.

use rusqlite::{params, Connection, OptionalExtension};
use vela_domain::errors::VelaError;

use crate::writer::store_error;

/// State of an operation record.
pub const INTENDED: &str = "INTENDED";
/// The operation finished and its result was observed.
pub const DONE: &str = "DONE";
/// The operation finished with an observed failure.
pub const FAILED: &str = "FAILED";
/// Recovery reconciled an operation that was still intended.
pub const RECONCILED: &str = "RECONCILED";

/// An operation record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    /// Identifier.
    pub id: String,
    /// The run, when the operation belongs to one.
    pub run_id: Option<String>,
    /// What kind of operation (`checkpoint`, `merge`, `push`, ...).
    pub kind: String,
    /// `INTENDED`, `DONE`, `FAILED` or `RECONCILED`.
    pub state: String,
    /// The intent, as JSON.
    pub intent: String,
    /// The observed result, as JSON.
    pub result: Option<String>,
    /// The reconciliation recipe registered for this operation.
    pub recipe: Option<String>,
}

const COLUMNS: &str = "id, run_id, kind, state, intent, result, recipe";

fn from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Operation> {
    Ok(Operation {
        id: r.get(0)?,
        run_id: r.get(1)?,
        kind: r.get(2)?,
        state: r.get(3)?,
        intent: r.get(4)?,
        result: r.get(5)?,
        recipe: r.get(6)?,
    })
}

/// Records the intent. Must be committed before the action is taken.
pub fn record_intent(
    conn: &Connection,
    id: &str,
    run_id: Option<&str>,
    kind: &str,
    intent_json: &str,
    recipe: Option<&str>,
    now_ms: i64,
) -> Result<(), VelaError> {
    conn.execute(
        "INSERT INTO operations (id, run_id, kind, state, detail, intent, recipe, created_ms, updated_ms)
         VALUES (?1, ?2, ?3, 'INTENDED', '', ?4, ?5, ?6, ?6)",
        params![id, run_id, kind, intent_json, recipe, now_ms],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// Records the observed result. Returns false when the operation was not `INTENDED`.
pub fn record_result(conn: &Connection, id: &str, ok: bool, result_json: &str, now_ms: i64) -> Result<bool, VelaError> {
    conn.execute(
        "UPDATE operations SET state = ?1, result = ?2, updated_ms = ?3 WHERE id = ?4 AND state = 'INTENDED'",
        params![if ok { DONE } else { FAILED }, result_json, now_ms, id],
    )
    .map(|n| n > 0)
    .map_err(store_error)
}

/// Marks an intended operation reconciled by recovery, with what recovery found.
pub fn mark_reconciled(conn: &Connection, id: &str, result_json: &str, now_ms: i64) -> Result<bool, VelaError> {
    conn.execute(
        "UPDATE operations SET state = 'RECONCILED', result = ?1, updated_ms = ?2 WHERE id = ?3 AND state = 'INTENDED'",
        params![result_json, now_ms, id],
    )
    .map(|n| n > 0)
    .map_err(store_error)
}

/// An operation by id.
pub fn get(conn: &Connection, id: &str) -> Result<Option<Operation>, VelaError> {
    conn.query_row(&format!("SELECT {COLUMNS} FROM operations WHERE id = ?1"), [id], from_row)
        .optional()
        .map_err(store_error)
}

/// Operations still `INTENDED` (their result was never recorded): what recovery must reconcile.
pub fn list_intended(conn: &Connection) -> Result<Vec<Operation>, VelaError> {
    let mut stmt = conn
        .prepare(&format!("SELECT {COLUMNS} FROM operations WHERE state = 'INTENDED' ORDER BY created_ms, id"))
        .map_err(store_error)?;
    let rows = stmt.query_map([], from_row).map_err(store_error)?;
    rows.collect::<Result<_, _>>().map_err(store_error)
}
