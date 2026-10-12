//! Repository for the persisted remote update queue (T03).

use rusqlite::{params, Connection};
use vela_domain::errors::VelaError;

use crate::writer::store_error;

/// A queued update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteUpdate {
    /// Row id.
    pub id: i64,
    /// The run it belongs to.
    pub run_id: Option<String>,
    /// The ticket it targets.
    pub ticket_id: String,
    /// `status`, `comment` or `close`.
    pub kind: String,
    /// The status text or comment body.
    pub payload: String,
    /// Stable key: enqueueing the same key twice keeps one row.
    pub idempotency_key: String,
    /// Attempts started so far (counted before the effect is tried).
    pub attempts: u32,
    /// Earliest next attempt.
    pub next_attempt_ms: i64,
}

/// Queues an update. Returns false when the key was already queued (nothing changes).
pub fn enqueue(
    conn: &Connection,
    run_id: Option<&str>,
    ticket_id: &str,
    kind: &str,
    payload: &str,
    key: &str,
    now_ms: i64,
) -> Result<bool, VelaError> {
    conn.execute(
        "INSERT OR IGNORE INTO remote_updates
         (run_id, ticket_id, kind, payload, idempotency_key, state, next_attempt_ms, created_ms, updated_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, 'PENDING', ?6, ?6, ?6)",
        params![run_id, ticket_id, kind, payload, key, now_ms],
    )
    .map(|n| n > 0)
    .map_err(store_error)
}

/// Pending updates whose time has come, oldest first.
pub fn due(conn: &Connection, now_ms: i64, limit: u32) -> Result<Vec<RemoteUpdate>, VelaError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, run_id, ticket_id, kind, payload, idempotency_key, attempts, next_attempt_ms
             FROM remote_updates WHERE state = 'PENDING' AND next_attempt_ms <= ?1 ORDER BY id LIMIT ?2",
        )
        .map_err(store_error)?;
    let rows = stmt
        .query_map(params![now_ms, limit], |r| {
            Ok(RemoteUpdate {
                id: r.get(0)?,
                run_id: r.get(1)?,
                ticket_id: r.get(2)?,
                kind: r.get(3)?,
                payload: r.get(4)?,
                idempotency_key: r.get(5)?,
                attempts: r.get(6)?,
                next_attempt_ms: r.get(7)?,
            })
        })
        .map_err(store_error)?;
    rows.collect::<Result<_, _>>().map_err(store_error)
}

/// Records that an attempt is about to start (so a crash mid-effect is visible as `attempts > 0`).
pub fn begin_attempt(conn: &Connection, id: i64, now_ms: i64) -> Result<(), VelaError> {
    conn.execute(
        "UPDATE remote_updates SET attempts = attempts + 1, updated_ms = ?1 WHERE id = ?2",
        params![now_ms, id],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// The effect is on the remote.
pub fn mark_done(conn: &Connection, id: i64, now_ms: i64) -> Result<(), VelaError> {
    conn.execute(
        "UPDATE remote_updates SET state = 'DONE', last_error = '', updated_ms = ?1 WHERE id = ?2",
        params![now_ms, id],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// The attempt failed; try again at `next_attempt_ms`.
pub fn mark_retry(conn: &Connection, id: i64, next_attempt_ms: i64, error: &str, now_ms: i64) -> Result<(), VelaError> {
    conn.execute(
        "UPDATE remote_updates SET next_attempt_ms = ?1, last_error = ?2, updated_ms = ?3 WHERE id = ?4",
        params![next_attempt_ms, error, now_ms, id],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// The update can never succeed (a human must look).
pub fn mark_permanent(conn: &Connection, id: i64, error: &str, now_ms: i64) -> Result<(), VelaError> {
    conn.execute(
        "UPDATE remote_updates SET state = 'FAILED_PERMANENT', last_error = ?1, updated_ms = ?2 WHERE id = ?3",
        params![error, now_ms, id],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// Number of updates in `state`.
pub fn count(conn: &Connection, state: &str) -> Result<u32, VelaError> {
    conn.query_row("SELECT COUNT(*) FROM remote_updates WHERE state = ?1", [state], |r| r.get(0))
        .map_err(store_error)
}
