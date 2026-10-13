//! Blocked-worker detection (K02): a worker that has produced no journal event for a while, and has no tracked command running,
//! is suspected of waiting on an approval nobody is answering. Repeated equivalent requests are stopped by the counters in the
//! broker; this module finds the silent ones.

use vela_domain::errors::VelaError;
use vela_domain::ids::{RunId, TicketId, WorkerId};
use vela_domain::state::{InterventionKind, WorkerState};
use vela_persistence::repositories::o01_workers;

use crate::intervention::{InterventionDetails, Interventions};
use crate::orchestrator::Services;

/// The default idle threshold.
pub const DEFAULT_IDLE_MS: i64 = 120_000;

/// A worker that looks stuck.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suspected {
    /// The worker.
    pub worker: WorkerId,
    /// Its ticket.
    pub ticket: TicketId,
    /// How long it has been silent.
    pub idle_ms: i64,
}

/// Workers of `run` that are active, silent for `threshold_ms`, and not running a tracked command.
pub fn find_suspected(
    services: &Services,
    run: &RunId,
    threshold_ms: i64,
) -> Result<Vec<Suspected>, VelaError> {
    let now = services.now_ms();
    let r = run.as_str().to_owned();
    let rows = services.store.read(|c| o01_workers::list_for_run(c, &r))?;
    let mut out = Vec::new();
    for row in rows {
        let state: WorkerState = serde_json::from_str(&row.state).map_err(|e| {
            VelaError::internal(format!("a stored worker state is unreadable: {e}"))
        })?;
        if !matches!(state, WorkerState::Active(_)) {
            continue;
        }
        let wid = row.id.clone();
        let (last, running, updated) = services.store.read(|c| {
            let updated: i64 = c
                .query_row("SELECT updated_ms FROM workers WHERE id = ?1", [&wid], |r| r.get(0))
                .map_err(|e| VelaError::internal(e.to_string()))?;
            let last: Option<i64> = c
                .query_row("SELECT MAX(ts) FROM event_journal WHERE worker_id = ?1", [&wid], |r| r.get(0))
                .map_err(|e| VelaError::internal(e.to_string()))?;
            let started: i64 = c
                .query_row(
                    "SELECT COUNT(*) FROM event_journal WHERE worker_id = ?1 AND kind = 'CommandStarted'",
                    [&wid],
                    |r| r.get(0),
                )
                .map_err(|e| VelaError::internal(e.to_string()))?;
            let finished: i64 = c
                .query_row(
                    "SELECT COUNT(*) FROM event_journal WHERE worker_id = ?1 AND kind = 'CommandFinished'",
                    [&wid],
                    |r| r.get(0),
                )
                .map_err(|e| VelaError::internal(e.to_string()))?;
            Ok((last, started > finished, updated))
        })?;
        if running {
            continue;
        }
        let since = last.unwrap_or(updated).max(updated);
        let idle = now - since;
        if idle >= threshold_ms {
            out.push(Suspected {
                worker: WorkerId::new(row.id).map_err(|e| {
                    VelaError::internal(format!("a stored worker id is invalid: {e}"))
                })?,
                ticket: TicketId::new(row.ticket_id).map_err(|e| {
                    VelaError::internal(format!("a stored ticket id is invalid: {e}"))
                })?,
                idle_ms: idle,
            });
        }
    }
    Ok(out)
}

/// Raises `APPROVAL_STALLED` for a suspected worker. With no approval surface to confirm it, the text says it is a suspicion.
pub async fn escalate(
    services: &Services,
    run: &RunId,
    s: &Suspected,
    ui_source_available: bool,
) -> Result<String, VelaError> {
    Interventions::new(services)
        .raise_for_worker(
            run,
            &s.worker,
            InterventionKind::ApprovalStalled,
            InterventionDetails {
                summary: if ui_source_available {
                    format!("{} has made no progress for {} seconds", s.ticket, s.idle_ms / 1000)
                } else {
                    format!(
                        "{} may be waiting on an approval: no activity for {} seconds, and Vela has no approval surface to confirm it",
                        s.ticket,
                        s.idle_ms / 1000
                    )
                },
                evidence: vec![format!("silent for {} ms with no command running", s.idle_ms)],
                preserved_work: "the worker's worktree and conversation are untouched".to_owned(),
                last_safe_commit: None,
                actions: vec![
                    "open Antigravity and answer the pending prompt".to_owned(),
                    "cancel the worker".to_owned(),
                ],
                consequences: "the worker stays paused until you decide".to_owned(),
            },
        )
        .await
}
