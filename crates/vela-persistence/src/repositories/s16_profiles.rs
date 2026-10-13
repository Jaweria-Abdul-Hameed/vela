//! Repository for execution profiles (S16).

use rusqlite::{params, Connection, OptionalExtension};
use vela_domain::errors::VelaError;

use crate::writer::store_error;

/// An execution profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileRow {
    /// Identifier.
    pub id: String,
    /// `AVAILABLE`, `BUSY`, `COOLDOWN`, `AUTH_REQUIRED`, `UNAVAILABLE`, `DISABLED` or `POLICY_BLOCKED`.
    pub state: String,
    /// When a cooldown ends.
    pub cooldown_until_ms: Option<i64>,
    /// Consecutive capacity interruptions (the backoff exponent).
    pub attempts: u32,
    /// Why it is in its state.
    pub reason: String,
    /// When a provider policy block was recorded.
    pub blocked_ms: Option<i64>,
    /// When the user's access re-check last succeeded.
    pub access_ok_ms: Option<i64>,
    /// Last change.
    pub updated_ms: i64,
}

const COLS: &str = "id, state, cooldown_until_ms, attempts, reason, blocked_ms, access_ok_ms, updated_ms";

/// The profile, or a fresh `AVAILABLE` one when it was never recorded.
pub fn get(conn: &Connection, id: &str) -> Result<ProfileRow, VelaError> {
    let found = conn
        .query_row(&format!("SELECT {COLS} FROM execution_profiles WHERE id = ?1"), [id], |r| {
            Ok(ProfileRow {
                id: r.get(0)?,
                state: r.get(1)?,
                cooldown_until_ms: r.get(2)?,
                attempts: r.get(3)?,
                reason: r.get(4)?,
                blocked_ms: r.get(5)?,
                access_ok_ms: r.get(6)?,
                updated_ms: r.get(7)?,
            })
        })
        .optional()
        .map_err(store_error)?;
    Ok(found.unwrap_or(ProfileRow {
        id: id.to_owned(),
        state: "AVAILABLE".to_owned(),
        cooldown_until_ms: None,
        attempts: 0,
        reason: String::new(),
        blocked_ms: None,
        access_ok_ms: None,
        updated_ms: 0,
    }))
}

/// Writes the whole row.
pub fn put(conn: &Connection, p: &ProfileRow) -> Result<(), VelaError> {
    conn.execute(
        &format!(
            "INSERT INTO execution_profiles ({COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
             ON CONFLICT(id) DO UPDATE SET state = ?2, cooldown_until_ms = ?3, attempts = ?4, reason = ?5,
                 blocked_ms = ?6, access_ok_ms = ?7, updated_ms = ?8"
        ),
        params![p.id, p.state, p.cooldown_until_ms, p.attempts, p.reason, p.blocked_ms, p.access_ok_ms, p.updated_ms],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// Every recorded profile.
pub fn list(conn: &Connection) -> Result<Vec<ProfileRow>, VelaError> {
    let mut stmt = conn
        .prepare("SELECT id FROM execution_profiles ORDER BY id")
        .map_err(store_error)?;
    let ids: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .map_err(store_error)?
        .collect::<Result<_, _>>()
        .map_err(store_error)?;
    ids.iter().map(|id| get(conn, id)).collect()
}
