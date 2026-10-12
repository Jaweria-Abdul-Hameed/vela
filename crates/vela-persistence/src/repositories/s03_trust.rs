//! Repository for repository trust (S03).

use rusqlite::{params, Connection, OptionalExtension};
use vela_domain::errors::VelaError;

use crate::writer::store_error;

/// A project's trust record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustRow {
    /// The project.
    pub project_id: String,
    /// `UNTRUSTED` or `TRUSTED`.
    pub state: String,
    /// The repository identity the trust applies to.
    pub repository_identity: String,
    /// Version of the consent text the user saw.
    pub consent_text_version: Option<String>,
    /// The confirmed command profile, as a JSON array.
    pub command_profile: String,
    /// When it last changed.
    pub changed_ms: i64,
}

/// Writes (or replaces) the trust record.
pub fn upsert(conn: &Connection, row: &TrustRow) -> Result<(), VelaError> {
    conn.execute(
        "INSERT INTO project_trust (project_id, state, repository_identity, consent_text_version, command_profile, changed_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(project_id) DO UPDATE SET state = ?2, repository_identity = ?3, consent_text_version = ?4,
             command_profile = ?5, changed_ms = ?6",
        params![
            row.project_id,
            row.state,
            row.repository_identity,
            row.consent_text_version,
            row.command_profile,
            row.changed_ms
        ],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// The record, if the project ever had one.
pub fn get(conn: &Connection, project_id: &str) -> Result<Option<TrustRow>, VelaError> {
    conn.query_row(
        "SELECT project_id, state, repository_identity, consent_text_version, command_profile, changed_ms
         FROM project_trust WHERE project_id = ?1",
        [project_id],
        |r| {
            Ok(TrustRow {
                project_id: r.get(0)?,
                state: r.get(1)?,
                repository_identity: r.get(2)?,
                consent_text_version: r.get(3)?,
                command_profile: r.get(4)?,
                changed_ms: r.get(5)?,
            })
        },
    )
    .optional()
    .map_err(store_error)
}
