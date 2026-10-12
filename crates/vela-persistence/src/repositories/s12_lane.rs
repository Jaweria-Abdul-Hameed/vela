//! Repository for the merge lane (S12): persisted integration health and fix attempts.

use rusqlite::{params, Connection, OptionalExtension};
use vela_domain::errors::VelaError;
use vela_domain::state::IntegrationHealth;

use crate::writer::store_error;

/// The run's integration health.
pub fn get_health(conn: &Connection, run_id: &str) -> Result<IntegrationHealth, VelaError> {
    let text: Option<String> = conn
        .query_row(
            "SELECT integration_health FROM build_runs WHERE id = ?1",
            [run_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(store_error)?;
    Ok(match text.as_deref() {
        Some("UNKNOWN") => IntegrationHealth::Unknown,
        Some("UNHEALTHY") => IntegrationHealth::Unhealthy,
        _ => IntegrationHealth::Healthy,
    })
}

/// Records the run's integration health.
pub fn set_health(conn: &Connection, run_id: &str, health: IntegrationHealth) -> Result<(), VelaError> {
    conn.execute(
        "UPDATE build_runs SET integration_health = ?2 WHERE id = ?1",
        params![run_id, health.name()],
    )
    .map(|_| ())
    .map_err(store_error)
}

/// Adds one fix attempt for a worker and returns the new total.
pub fn bump_fix_attempts(conn: &Connection, worker_id: &str) -> Result<u32, VelaError> {
    conn.execute(
        "INSERT INTO integration_fix_counts (worker_id, attempts) VALUES (?1, 1)
         ON CONFLICT(worker_id) DO UPDATE SET attempts = attempts + 1",
        [worker_id],
    )
    .map_err(store_error)?;
    conn.query_row(
        "SELECT attempts FROM integration_fix_counts WHERE worker_id = ?1",
        [worker_id],
        |r| r.get::<_, u32>(0),
    )
    .map_err(store_error)
}
