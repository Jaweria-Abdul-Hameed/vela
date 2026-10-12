//! Repositories for preflight reports, adapter capability snapshots and skill installations (S04; K10 persists through the
//! last two).

use rusqlite::{params, Connection, OptionalExtension};
use vela_domain::errors::VelaError;

use crate::writer::store_error;

/// A stored preflight report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportRow {
    /// Identifier.
    pub id: String,
    /// The project.
    pub project_id: String,
    /// `PASS`, `WARN` or `BLOCK`.
    pub verdict: String,
    /// The report as JSON.
    pub report: String,
    /// When it was produced.
    pub created_ms: i64,
}

/// Stores a report.
pub fn insert_report(conn: &Connection, row: &ReportRow) -> Result<(), VelaError> {
    conn.execute(
        "INSERT INTO preflight_reports (id, project_id, verdict, report, created_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![row.id, row.project_id, row.verdict, row.report, row.created_ms],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// The newest reports of a project, newest first.
pub fn list_reports(
    conn: &Connection,
    project_id: &str,
    limit: u32,
) -> Result<Vec<ReportRow>, VelaError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, project_id, verdict, report, created_ms FROM preflight_reports
             WHERE project_id = ?1 ORDER BY created_ms DESC, id DESC LIMIT ?2",
        )
        .map_err(store_error)?;
    let rows = stmt
        .query_map(params![project_id, limit], |r| {
            Ok(ReportRow {
                id: r.get(0)?,
                project_id: r.get(1)?,
                verdict: r.get(2)?,
                report: r.get(3)?,
                created_ms: r.get(4)?,
            })
        })
        .map_err(store_error)?;
    rows.collect::<Result<_, _>>().map_err(store_error)
}

/// Stores an adapter capability snapshot.
pub fn insert_capability_snapshot(
    conn: &Connection,
    id: &str,
    adapter: &str,
    version: &str,
    snapshot: &str,
    now_ms: i64,
) -> Result<(), VelaError> {
    conn.execute(
        "INSERT INTO adapter_capability_snapshots (id, adapter, version, snapshot, created_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, adapter, version, snapshot, now_ms],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// The newest capability snapshot of an adapter: `(version, snapshot)`.
pub fn latest_capability_snapshot(
    conn: &Connection,
    adapter: &str,
) -> Result<Option<(String, String)>, VelaError> {
    conn.query_row(
        "SELECT version, snapshot FROM adapter_capability_snapshots WHERE adapter = ?1
         ORDER BY created_ms DESC, id DESC LIMIT 1",
        [adapter],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
    .map_err(store_error)
}

/// Records a skill installation.
pub fn insert_skill_installation(
    conn: &Connection,
    id: &str,
    adapter: &str,
    scope: &str,
    skill: &str,
    content_hash: &str,
    now_ms: i64,
) -> Result<(), VelaError> {
    conn.execute(
        "INSERT INTO skill_installations (id, adapter, scope, skill, content_hash, installed_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, adapter, scope, skill, content_hash, now_ms],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// The newest installation of a skill in a scope: `(content_hash, installed_ms)`.
pub fn latest_skill_installation(
    conn: &Connection,
    adapter: &str,
    scope: &str,
    skill: &str,
) -> Result<Option<(String, i64)>, VelaError> {
    conn.query_row(
        "SELECT content_hash, installed_ms FROM skill_installations WHERE adapter = ?1 AND scope = ?2 AND skill = ?3
         ORDER BY installed_ms DESC, id DESC LIMIT 1",
        params![adapter, scope, skill],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
    .map_err(store_error)
}
