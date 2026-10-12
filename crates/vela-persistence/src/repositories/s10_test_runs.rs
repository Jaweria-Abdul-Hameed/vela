//! Repository for validation command results (S10).

use rusqlite::{params, Connection};
use vela_domain::errors::VelaError;

use crate::writer::store_error;

/// One validation command result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestRun {
    /// Identifier.
    pub id: String,
    /// The run.
    pub run_id: String,
    /// The worker.
    pub worker_id: String,
    /// The ticket.
    pub ticket_id: String,
    /// The (redacted) command line.
    pub command: String,
    /// The exit code, when the process exited by itself.
    pub exit_code: Option<i32>,
    /// True when the command passed.
    pub passed: bool,
    /// True when it was killed for exceeding its timeout.
    pub timed_out: bool,
    /// Wall-clock milliseconds.
    pub duration_ms: i64,
    /// The last lines of (redacted) output.
    pub output_tail: String,
    /// When it finished.
    pub created_ms: i64,
}

/// Stores a result.
pub fn insert(conn: &Connection, t: &TestRun) -> Result<(), VelaError> {
    conn.execute(
        "INSERT INTO test_runs (id, run_id, worker_id, ticket_id, command, exit_code, passed, timed_out, duration_ms, output_tail, created_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            t.id,
            t.run_id,
            t.worker_id,
            t.ticket_id,
            t.command,
            t.exit_code,
            i64::from(t.passed),
            i64::from(t.timed_out),
            t.duration_ms,
            t.output_tail,
            t.created_ms
        ],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// A worker's results, oldest first.
pub fn list_for_worker(conn: &Connection, worker_id: &str) -> Result<Vec<TestRun>, VelaError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, run_id, worker_id, ticket_id, command, exit_code, passed, timed_out, duration_ms, output_tail, created_ms
             FROM test_runs WHERE worker_id = ?1 ORDER BY created_ms, id",
        )
        .map_err(store_error)?;
    let rows = stmt
        .query_map([worker_id], |r| {
            Ok(TestRun {
                id: r.get(0)?,
                run_id: r.get(1)?,
                worker_id: r.get(2)?,
                ticket_id: r.get(3)?,
                command: r.get(4)?,
                exit_code: r.get(5)?,
                passed: r.get::<_, i64>(6)? == 1,
                timed_out: r.get::<_, i64>(7)? == 1,
                duration_ms: r.get(8)?,
                output_tail: r.get(9)?,
                created_ms: r.get(10)?,
            })
        })
        .map_err(store_error)?;
    rows.collect::<Result<_, _>>().map_err(store_error)
}
