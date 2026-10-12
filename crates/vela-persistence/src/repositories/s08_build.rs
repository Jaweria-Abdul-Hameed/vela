//! Repository for the build start (S08): a run's base and integration location, and managed worktrees.

use rusqlite::{params, Connection, OptionalExtension};
use vela_domain::errors::VelaError;

use crate::writer::store_error;

/// Where a run was started from and where it integrates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunBase {
    /// The commit every worker branch descends from.
    pub base_sha: String,
    /// The branch the base was read from.
    pub base_branch: String,
    /// The integration branch.
    pub integration_branch: String,
    /// The integration worktree.
    pub integration_worktree: String,
}

/// Records the base and integration location of a run.
pub fn set_run_base(conn: &Connection, run_id: &str, b: &RunBase) -> Result<(), VelaError> {
    conn.execute(
        "UPDATE build_runs SET base_sha = ?2, base_branch = ?3, integration_branch = ?4, integration_worktree = ?5 WHERE id = ?1",
        params![run_id, b.base_sha, b.base_branch, b.integration_branch, b.integration_worktree],
    )
    .map(|_| ())
    .map_err(store_error)
}

type RawBase = (Option<String>, Option<String>, Option<String>, Option<String>);

/// The recorded base, once the run started.
pub fn get_run_base(conn: &Connection, run_id: &str) -> Result<Option<RunBase>, VelaError> {
    let row: Option<RawBase> = conn
        .query_row(
            "SELECT base_sha, base_branch, integration_branch, integration_worktree FROM build_runs WHERE id = ?1",
            [run_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
        .map_err(store_error)?;
    Ok(match row {
        Some((Some(base_sha), Some(base_branch), Some(integration_branch), Some(integration_worktree))) => Some(RunBase {
            base_sha,
            base_branch,
            integration_branch,
            integration_worktree,
        }),
        _ => None,
    })
}

/// Records a managed worktree.
pub fn insert_worktree(
    conn: &Connection,
    path: &str,
    run_id: &str,
    kind: &str,
    branch: &str,
    base_sha: &str,
    now_ms: i64,
) -> Result<(), VelaError> {
    conn.execute(
        "INSERT INTO worktrees (path, run_id, kind, branch, base_sha, created_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![path, run_id, kind, branch, base_sha, now_ms],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// Marks a worktree removed.
pub fn mark_worktree_removed(conn: &Connection, path: &str, now_ms: i64) -> Result<(), VelaError> {
    conn.execute("UPDATE worktrees SET removed_ms = ?2 WHERE path = ?1", params![path, now_ms])
        .map(|_| ())
        .map_err(store_error)
}

/// The live worktrees of a run: `(path, kind, branch)`.
pub fn list_worktrees(conn: &Connection, run_id: &str) -> Result<Vec<(String, String, String)>, VelaError> {
    let mut stmt = conn
        .prepare("SELECT path, kind, branch FROM worktrees WHERE run_id = ?1 AND removed_ms IS NULL ORDER BY created_ms, path")
        .map_err(store_error)?;
    let rows = stmt
        .query_map([run_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(store_error)?;
    rows.collect::<Result<_, _>>().map_err(store_error)
}
