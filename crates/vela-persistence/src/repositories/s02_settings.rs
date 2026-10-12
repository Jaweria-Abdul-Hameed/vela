//! Repository for project settings, run settings snapshots and consent records (S02). User-global settings use the
//! `settings` table of F06.

use rusqlite::{params, Connection, OptionalExtension};
use vela_domain::errors::VelaError;

use crate::writer::store_error;

/// One consent record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consent {
    /// Row id (increases with time).
    pub id: i64,
    /// What was consented to (`ui_automation`, `autostart`, ...).
    pub key: String,
    /// The version of the consent text the user saw.
    pub text_version: String,
    /// Granted or revoked.
    pub granted: bool,
    /// `user` or a project id.
    pub scope: String,
    /// When it was recorded.
    pub recorded_ms: i64,
}

/// Sets one project setting.
pub fn set_project_setting(conn: &Connection, project_id: &str, key: &str, value: &str, now_ms: i64) -> Result<(), VelaError> {
    conn.execute(
        "INSERT INTO project_settings (project_id, key, value, updated_ms) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(project_id, key) DO UPDATE SET value = excluded.value, updated_ms = excluded.updated_ms",
        params![project_id, key, value, now_ms],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// One project setting.
pub fn get_project_setting(conn: &Connection, project_id: &str, key: &str) -> Result<Option<String>, VelaError> {
    conn.query_row(
        "SELECT value FROM project_settings WHERE project_id = ?1 AND key = ?2",
        params![project_id, key],
        |r| r.get(0),
    )
    .optional()
    .map_err(store_error)
}

/// Every setting of a project, sorted by key.
pub fn list_project_settings(conn: &Connection, project_id: &str) -> Result<Vec<(String, String)>, VelaError> {
    let mut stmt = conn
        .prepare("SELECT key, value FROM project_settings WHERE project_id = ?1 ORDER BY key")
        .map_err(store_error)?;
    let rows = stmt
        .query_map([project_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(store_error)?;
    rows.collect::<Result<_, _>>().map_err(store_error)
}

/// Every user-global setting, sorted by key.
pub fn list_global_settings(conn: &Connection) -> Result<Vec<(String, String)>, VelaError> {
    let mut stmt = conn
        .prepare("SELECT key, value FROM settings ORDER BY key")
        .map_err(store_error)?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(store_error)?;
    rows.collect::<Result<_, _>>().map_err(store_error)
}

/// Stores a run's immutable settings snapshot. Fails if the run already has one.
pub fn insert_run_snapshot(conn: &Connection, run_id: &str, snapshot_json: &str, now_ms: i64) -> Result<(), VelaError> {
    conn.execute(
        "INSERT INTO run_settings_snapshot (run_id, snapshot, created_ms) VALUES (?1, ?2, ?3)",
        params![run_id, snapshot_json, now_ms],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// A run's settings snapshot.
pub fn get_run_snapshot(conn: &Connection, run_id: &str) -> Result<Option<String>, VelaError> {
    conn.query_row("SELECT snapshot FROM run_settings_snapshot WHERE run_id = ?1", [run_id], |r| r.get(0))
        .optional()
        .map_err(store_error)
}

/// Records a consent decision (append-only).
pub fn insert_consent(
    conn: &Connection,
    key: &str,
    text_version: &str,
    granted: bool,
    scope: &str,
    now_ms: i64,
) -> Result<i64, VelaError> {
    conn.execute(
        "INSERT INTO consents (key, text_version, granted, scope, recorded_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![key, text_version, i64::from(granted), scope, now_ms],
    )
    .map_err(store_error)?;
    Ok(conn.last_insert_rowid())
}

/// The latest consent decision for `key` in `scope`.
pub fn latest_consent(conn: &Connection, key: &str, scope: &str) -> Result<Option<Consent>, VelaError> {
    conn.query_row(
        "SELECT id, key, text_version, granted, scope, recorded_ms FROM consents WHERE key = ?1 AND scope = ?2 ORDER BY id DESC LIMIT 1",
        params![key, scope],
        |r| {
            Ok(Consent {
                id: r.get(0)?,
                key: r.get(1)?,
                text_version: r.get(2)?,
                granted: r.get::<_, i64>(3)? != 0,
                scope: r.get(4)?,
                recorded_ms: r.get(5)?,
            })
        },
    )
    .optional()
    .map_err(store_error)
}
