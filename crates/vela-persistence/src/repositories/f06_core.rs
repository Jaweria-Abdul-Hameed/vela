//! Repositories for the F06 tables: projects, runs, operations and settings.

use rusqlite::{params, Connection, OptionalExtension};
use vela_domain::errors::VelaError;

use crate::writer::store_error;

/// A project row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRow {
    /// Identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Repository path.
    pub repo_path: String,
    /// Creation time.
    pub created_ms: i64,
    /// Last time it was opened.
    pub last_opened_ms: Option<i64>,
}

/// Inserts a project (the repository path is unique).
pub fn insert_project(conn: &Connection, id: &str, name: &str, repo_path: &str, now_ms: i64) -> Result<(), VelaError> {
    conn.execute(
        "INSERT INTO projects (id, name, repo_path, created_ms, last_opened_ms) VALUES (?1, ?2, ?3, ?4, ?4)",
        params![id, name, repo_path, now_ms],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// Marks a project opened now.
pub fn touch_project(conn: &Connection, id: &str, now_ms: i64) -> Result<(), VelaError> {
    conn.execute("UPDATE projects SET last_opened_ms = ?1 WHERE id = ?2", params![now_ms, id])
        .map(|_| ())
        .map_err(store_error)
}

fn project_from(r: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectRow> {
    Ok(ProjectRow {
        id: r.get(0)?,
        name: r.get(1)?,
        repo_path: r.get(2)?,
        created_ms: r.get(3)?,
        last_opened_ms: r.get(4)?,
    })
}

/// A project by repository path.
pub fn project_by_path(conn: &Connection, repo_path: &str) -> Result<Option<ProjectRow>, VelaError> {
    conn.query_row(
        "SELECT id, name, repo_path, created_ms, last_opened_ms FROM projects WHERE repo_path = ?1",
        [repo_path],
        project_from,
    )
    .optional()
    .map_err(store_error)
}

/// Recently opened projects, most recent first.
pub fn recent_projects(conn: &Connection, limit: u32) -> Result<Vec<ProjectRow>, VelaError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, repo_path, created_ms, last_opened_ms FROM projects WHERE removed_ms IS NULL
             ORDER BY COALESCE(last_opened_ms, created_ms) DESC, id LIMIT ?1",
        )
        .map_err(store_error)?;
    let rows = stmt.query_map([limit], project_from).map_err(store_error)?;
    rows.collect::<Result<_, _>>().map_err(store_error)
}

/// Inserts a run in `state`.
pub fn insert_run(conn: &Connection, id: &str, project_id: &str, state: &str, now_ms: i64) -> Result<(), VelaError> {
    conn.execute(
        "INSERT INTO build_runs (id, project_id, state, created_ms, updated_ms) VALUES (?1, ?2, ?3, ?4, ?4)",
        params![id, project_id, state, now_ms],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// Sets a run's state.
pub fn set_run_state(conn: &Connection, id: &str, state: &str, now_ms: i64) -> Result<(), VelaError> {
    conn.execute(
        "UPDATE build_runs SET state = ?1, updated_ms = ?2 WHERE id = ?3",
        params![state, now_ms, id],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// A run's state, if it exists.
pub fn run_state(conn: &Connection, id: &str) -> Result<Option<String>, VelaError> {
    conn.query_row("SELECT state FROM build_runs WHERE id = ?1", [id], |r| r.get(0))
        .optional()
        .map_err(store_error)
}

/// Writes or replaces an operation record.
pub fn upsert_operation(
    conn: &Connection,
    id: &str,
    run_id: Option<&str>,
    kind: &str,
    state: &str,
    detail: &str,
    now_ms: i64,
) -> Result<(), VelaError> {
    conn.execute(
        "INSERT INTO operations (id, run_id, kind, state, detail, created_ms, updated_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
         ON CONFLICT(id) DO UPDATE SET state = excluded.state, detail = excluded.detail, updated_ms = excluded.updated_ms",
        params![id, run_id, kind, state, detail, now_ms],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// An operation's state, if it exists.
pub fn operation_state(conn: &Connection, id: &str) -> Result<Option<String>, VelaError> {
    conn.query_row("SELECT state FROM operations WHERE id = ?1", [id], |r| r.get(0))
        .optional()
        .map_err(store_error)
}

/// Sets a setting.
pub fn set_setting(conn: &Connection, key: &str, value: &str, now_ms: i64) -> Result<(), VelaError> {
    conn.execute(
        "INSERT INTO settings (key, value, updated_ms) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_ms = excluded.updated_ms",
        params![key, value, now_ms],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// A setting's value.
pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>, VelaError> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
        .optional()
        .map_err(store_error)
}
