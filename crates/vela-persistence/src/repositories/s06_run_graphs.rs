//! Repository for run graphs (S06).

use rusqlite::{params, Connection, OptionalExtension};
use vela_domain::errors::VelaError;

use crate::writer::store_error;

/// A stored graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunGraph {
    /// The run.
    pub run_id: String,
    /// The graph snapshot (JSON).
    pub snapshot: String,
    /// The layout (JSON).
    pub layout: String,
    /// Pair verdicts (JSON array).
    pub verdicts: String,
    /// Analyst output: write sets and hazards (JSON object).
    pub analyst: String,
    /// Warnings (JSON array of text).
    pub warnings: String,
    /// `ANALYZED` or `SEQUENTIAL_FALLBACK`.
    pub analysis_state: String,
    /// When the user approved it.
    pub approved_ms: Option<i64>,
    /// What the last scheduling cycle decided (JSON).
    pub last_cycle: String,
    /// When it was built.
    pub created_ms: i64,
}

const COLUMNS: &str = "run_id, snapshot, layout, verdicts, analyst, warnings, analysis_state, approved_ms, last_cycle, created_ms";

/// Stores the graph, replacing an earlier analysis of the same run (and clearing its approval).
pub fn upsert(conn: &Connection, g: &RunGraph) -> Result<(), VelaError> {
    conn.execute(
        "INSERT INTO run_graphs (run_id, snapshot, layout, verdicts, analyst, warnings, analysis_state, approved_ms, last_cycle, created_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, '{}', ?8)
         ON CONFLICT(run_id) DO UPDATE SET snapshot = ?2, layout = ?3, verdicts = ?4, analyst = ?5, warnings = ?6,
             analysis_state = ?7, approved_ms = NULL, last_cycle = '{}', created_ms = ?8",
        params![g.run_id, g.snapshot, g.layout, g.verdicts, g.analyst, g.warnings, g.analysis_state, g.created_ms],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// The run's graph.
pub fn get(conn: &Connection, run_id: &str) -> Result<Option<RunGraph>, VelaError> {
    conn.query_row(
        &format!("SELECT {COLUMNS} FROM run_graphs WHERE run_id = ?1"),
        [run_id],
        |r| {
            Ok(RunGraph {
                run_id: r.get(0)?,
                snapshot: r.get(1)?,
                layout: r.get(2)?,
                verdicts: r.get(3)?,
                analyst: r.get(4)?,
                warnings: r.get(5)?,
                analysis_state: r.get(6)?,
                approved_ms: r.get(7)?,
                last_cycle: r.get(8)?,
                created_ms: r.get(9)?,
            })
        },
    )
    .optional()
    .map_err(store_error)
}

/// Records the user's approval. False when the run has no graph.
pub fn approve(conn: &Connection, run_id: &str, now_ms: i64) -> Result<bool, VelaError> {
    conn.execute("UPDATE run_graphs SET approved_ms = ?2 WHERE run_id = ?1", params![run_id, now_ms])
        .map(|n| n == 1)
        .map_err(store_error)
}

/// Records what the last scheduling cycle decided.
pub fn set_last_cycle(conn: &Connection, run_id: &str, json: &str) -> Result<(), VelaError> {
    conn.execute("UPDATE run_graphs SET last_cycle = ?2 WHERE run_id = ?1", params![run_id, json])
        .map(|_| ())
        .map_err(store_error)
}
