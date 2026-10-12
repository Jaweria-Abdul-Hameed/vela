//! Repository for intervention records (S15).

use rusqlite::{params, Connection, OptionalExtension};
use vela_domain::errors::VelaError;

use crate::writer::store_error;

/// An intervention record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Intervention {
    /// Identifier.
    pub id: String,
    /// The run.
    pub run_id: String,
    /// The worker, for worker-level kinds.
    pub worker_id: Option<String>,
    /// The kind (`AMBIGUITY`, ...).
    pub kind: String,
    /// `OPEN` or `RESOLVED`.
    pub state: String,
    /// The serialized state the run or worker returns to.
    pub resume_state: String,
    /// What is needed.
    pub summary: String,
    /// Evidence (a JSON array of short strings).
    pub evidence: String,
    /// What was preserved.
    pub preserved_work: String,
    /// The last commit known to be safe.
    pub last_safe_commit: Option<String>,
    /// The actions offered (a JSON array).
    pub actions: String,
    /// What each action leads to.
    pub consequences: String,
    /// When it was raised.
    pub created_ms: i64,
    /// When it was resolved.
    pub resolved_ms: Option<i64>,
    /// How it was resolved.
    pub resolution: Option<String>,
}

const COLUMNS: &str = "id, run_id, worker_id, kind, state, resume_state, summary, evidence, preserved_work, last_safe_commit, actions, consequences, created_ms, resolved_ms, resolution";

fn from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Intervention> {
    Ok(Intervention {
        id: r.get(0)?,
        run_id: r.get(1)?,
        worker_id: r.get(2)?,
        kind: r.get(3)?,
        state: r.get(4)?,
        resume_state: r.get(5)?,
        summary: r.get(6)?,
        evidence: r.get(7)?,
        preserved_work: r.get(8)?,
        last_safe_commit: r.get(9)?,
        actions: r.get(10)?,
        consequences: r.get(11)?,
        created_ms: r.get(12)?,
        resolved_ms: r.get(13)?,
        resolution: r.get(14)?,
    })
}

/// Inserts an open intervention.
pub fn insert(conn: &Connection, i: &Intervention) -> Result<(), VelaError> {
    conn.execute(
        "INSERT INTO human_interventions
         (id, run_id, worker_id, kind, state, resume_state, summary, evidence, preserved_work, last_safe_commit, actions, consequences, created_ms)
         VALUES (?1, ?2, ?3, ?4, 'OPEN', ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            i.id,
            i.run_id,
            i.worker_id,
            i.kind,
            i.resume_state,
            i.summary,
            i.evidence,
            i.preserved_work,
            i.last_safe_commit,
            i.actions,
            i.consequences,
            i.created_ms
        ],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// An intervention by id.
pub fn get(conn: &Connection, id: &str) -> Result<Option<Intervention>, VelaError> {
    conn.query_row(&format!("SELECT {COLUMNS} FROM human_interventions WHERE id = ?1"), [id], from_row)
        .optional()
        .map_err(store_error)
}

/// Open interventions of a run, oldest first.
pub fn list_open(conn: &Connection, run_id: &str) -> Result<Vec<Intervention>, VelaError> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {COLUMNS} FROM human_interventions WHERE run_id = ?1 AND state = 'OPEN' ORDER BY created_ms, id"
        ))
        .map_err(store_error)?;
    let rows = stmt.query_map([run_id], from_row).map_err(store_error)?;
    rows.collect::<Result<_, _>>().map_err(store_error)
}

/// Marks an open intervention resolved. Returns false when it was not open.
pub fn resolve(conn: &Connection, id: &str, resolution: &str, now_ms: i64) -> Result<bool, VelaError> {
    conn.execute(
        "UPDATE human_interventions SET state = 'RESOLVED', resolved_ms = ?1, resolution = ?2 WHERE id = ?3 AND state = 'OPEN'",
        params![now_ms, resolution, id],
    )
    .map(|n| n > 0)
    .map_err(store_error)
}
