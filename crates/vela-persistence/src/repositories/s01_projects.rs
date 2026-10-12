//! Repository for imported projects (S01): identity, soft removal and listing. Removing a project never touches the repository;
//! it only marks Vela's own record so run history stays intact.

use rusqlite::{params, Connection, OptionalExtension};
use vela_domain::errors::VelaError;

use crate::writer::store_error;

/// A project as Vela records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    /// Identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// The repository path.
    pub repo_path: String,
    /// The `origin` URL, when the repository has one.
    pub remote_url: Option<String>,
    /// The default branch, when known.
    pub default_branch: Option<String>,
    /// The commit `HEAD` pointed at when it was imported.
    pub head_at_import: Option<String>,
    /// Last time it was opened, epoch milliseconds.
    pub last_opened_ms: Option<i64>,
    /// When it was removed from Vela, if it was.
    pub removed_ms: Option<i64>,
}

const COLUMNS: &str =
    "id, name, repo_path, remote_url, default_branch, head_at_import, last_opened_ms, removed_ms";

fn from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: r.get(0)?,
        name: r.get(1)?,
        repo_path: r.get(2)?,
        remote_url: r.get(3)?,
        default_branch: r.get(4)?,
        head_at_import: r.get(5)?,
        last_opened_ms: r.get(6)?,
        removed_ms: r.get(7)?,
    })
}

/// What an import did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Imported {
    /// The project.
    pub project: Project,
    /// True when the repository was already known (possibly removed earlier) and its record was revived or refreshed.
    pub existing: bool,
}

/// Imports a repository: a new record, or the existing one (revived if it was removed) with its identity refreshed.
#[allow(clippy::too_many_arguments)]
pub fn import(
    conn: &Connection,
    id: &str,
    name: &str,
    repo_path: &str,
    remote_url: Option<&str>,
    default_branch: Option<&str>,
    head: Option<&str>,
    now_ms: i64,
) -> Result<Imported, VelaError> {
    let existing: Option<String> = conn
        .query_row("SELECT id FROM projects WHERE repo_path = ?1", [repo_path], |r| r.get(0))
        .optional()
        .map_err(store_error)?;
    let (project_id, was_existing) = match existing {
        Some(found) => {
            conn.execute(
                "UPDATE projects SET removed_ms = NULL, last_opened_ms = ?1, remote_url = ?2, default_branch = ?3, head_at_import = ?4 WHERE id = ?5",
                params![now_ms, remote_url, default_branch, head, found],
            )
            .map_err(store_error)?;
            (found, true)
        }
        None => {
            conn.execute(
                "INSERT INTO projects (id, name, repo_path, created_ms, last_opened_ms, remote_url, default_branch, head_at_import)
                 VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?6, ?7)",
                params![id, name, repo_path, now_ms, remote_url, default_branch, head],
            )
            .map_err(store_error)?;
            (id.to_owned(), false)
        }
    };
    let project = get(conn, &project_id)?.ok_or_else(|| store_error("the imported project disappeared"))?;
    Ok(Imported {
        project,
        existing: was_existing,
    })
}

/// A project by id (removed ones included).
pub fn get(conn: &Connection, id: &str) -> Result<Option<Project>, VelaError> {
    conn.query_row(&format!("SELECT {COLUMNS} FROM projects WHERE id = ?1"), [id], from_row)
        .optional()
        .map_err(store_error)
}

/// Projects not removed, most recently opened first.
pub fn list_active(conn: &Connection, limit: u32) -> Result<Vec<Project>, VelaError> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {COLUMNS} FROM projects WHERE removed_ms IS NULL
             ORDER BY COALESCE(last_opened_ms, created_ms) DESC, id LIMIT ?1"
        ))
        .map_err(store_error)?;
    let rows = stmt.query_map([limit], from_row).map_err(store_error)?;
    rows.collect::<Result<_, _>>().map_err(store_error)
}

/// Marks a project removed from Vela. Returns false when there was no active project with that id.
pub fn remove(conn: &Connection, id: &str, now_ms: i64) -> Result<bool, VelaError> {
    conn.execute(
        "UPDATE projects SET removed_ms = ?1 WHERE id = ?2 AND removed_ms IS NULL",
        params![now_ms, id],
    )
    .map(|n| n > 0)
    .map_err(store_error)
}

/// Marks a project opened now (and un-removed projects only).
pub fn touch(conn: &Connection, id: &str, now_ms: i64) -> Result<bool, VelaError> {
    conn.execute(
        "UPDATE projects SET last_opened_ms = ?1 WHERE id = ?2 AND removed_ms IS NULL",
        params![now_ms, id],
    )
    .map(|n| n > 0)
    .map_err(store_error)
}
