//! Interactive-session degradation and reconciliation for UI-automation work (K07, ADR-012, AT-017).
//!
//! Approvals delivered through UI Automation need an unlocked interactive desktop. When it goes away (locked, disconnected, the
//! secure desktop, or simply unknown) the workers whose approvals depend on it are paused with `INTERACTIVE_SESSION_UNAVAILABLE`
//! and every other worker carries on. This is not a failure and nothing is bypassed. A screen that is merely switched off is still
//! an available session, so it causes no transition.
//!
//! When the session returns, the paused workers resume *without* any decision being replayed: every approval request that was
//! still pending when the session was lost is marked `UNDELIVERABLE` (it can no longer be answered), so the only way a prompt can be answered now is through a fresh
//! observation, which the broker re-correlates and re-classifies against current policy before delivering anything.

use std::sync::{Arc, Mutex};

use vela_domain::errors::VelaError;
use vela_domain::events::{Event, WorkerPausedPayload, WorkerResumedPayload};
use vela_domain::ids::{RunId, WorkerId};
use vela_domain::ports::{SessionAvailability, SessionProbe};
use vela_domain::state::{Cause, IntegrationHealth, PausedReason, WorkerState};
use vela_persistence::repositories::{k01_approvals, o01_workers};

use crate::orchestrator::Services;
use crate::orchestrator::runs::Runs;

/// What a poll found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transition {
    /// Nothing changed.
    None,
    /// The session was lost; these workers were paused.
    Lost {
        /// Why it is unavailable.
        availability: SessionAvailability,
        /// The workers paused.
        paused: Vec<WorkerId>,
        /// Pending approval requests marked stale.
        stale: usize,
    },
    /// The session came back; these workers resumed and will re-raise any prompt that is still showing.
    Returned {
        /// The workers resumed.
        resumed: Vec<WorkerId>,
        /// Pending approval requests marked stale (none are replayed).
        stale: usize,
    },
}

/// Watches the interactive session for one project's UI-automation approvals.
pub struct SessionGuard {
    services: Arc<Services>,
    probe: Arc<dyn SessionProbe>,
    last: Mutex<SessionAvailability>,
}

impl SessionGuard {
    /// A guard that assumes the session is available until a poll says otherwise.
    pub fn new(services: Arc<Services>, probe: Arc<dyn SessionProbe>) -> Self {
        Self {
            services,
            probe,
            last: Mutex::new(SessionAvailability::Available),
        }
    }

    /// The availability seen at the last poll.
    pub fn last(&self) -> SessionAvailability {
        *self.last.lock().expect("session lock")
    }

    fn stale_pending(&self, workers: &[WorkerId]) -> Result<usize, VelaError> {
        let now = self.services.now_ms();
        let ids: Vec<String> = workers.iter().map(|w| w.as_str().to_owned()).collect();
        self.services.store.write(move |tx| {
            let mut n = 0;
            for w in &ids {
                for a in k01_approvals::list_for_worker(tx.conn(), w)? {
                    if a.state == "PENDING" || a.state == "ASKED" {
                        k01_approvals::set_state(tx.conn(), &a.id, "UNDELIVERABLE", now)?;
                        n += 1;
                    }
                }
            }
            Ok(n)
        })
    }

    /// Polls the probe once. `affected` are the workers whose approvals depend on UI automation: only they are ever paused or
    /// resumed here; every other worker is untouched.
    pub async fn poll(&self, run: &RunId, affected: &[WorkerId]) -> Result<Transition, VelaError> {
        let now = self.probe.interactive_session();
        let before = std::mem::replace(&mut *self.last.lock().expect("session lock"), now);
        let (was_ok, is_ok) = (before.permits_ui_automation(), now.permits_ui_automation());
        let runs = Runs::new(&self.services);
        let run_id = run.as_str().to_owned();

        if was_ok && !is_ok {
            let rows = self
                .services
                .store
                .read(|c| o01_workers::list_for_run(c, &run_id))?;
            let mut paused = Vec::new();
            for row in rows {
                let Ok(id) = WorkerId::new(row.id.clone()) else {
                    continue;
                };
                if !affected.contains(&id) {
                    continue;
                }
                let Ok(WorkerState::Active(phase)) =
                    serde_json::from_str::<WorkerState>(&row.state)
                else {
                    continue;
                };
                runs.transition_worker(
                    run,
                    &id,
                    WorkerState::Paused {
                        reason: PausedReason::InteractiveSessionUnavailable,
                        resume_state: phase,
                    },
                    Cause::System,
                    IntegrationHealth::Healthy,
                    vec![Event::WorkerPaused(WorkerPausedPayload {
                        reason: "interactive_session_unavailable".to_owned(),
                    })],
                )
                .await?;
                paused.push(id);
            }
            let stale = self.stale_pending(&paused)?;
            return Ok(Transition::Lost {
                availability: now,
                paused,
                stale,
            });
        }

        if !was_ok && is_ok {
            let rows = self
                .services
                .store
                .read(|c| o01_workers::list_for_run(c, &run_id))?;
            let mut resumed = Vec::new();
            let mut stale = 0;
            for row in rows {
                let Ok(id) = WorkerId::new(row.id.clone()) else {
                    continue;
                };
                let Ok(WorkerState::Paused {
                    reason: PausedReason::InteractiveSessionUnavailable,
                    resume_state,
                }) = serde_json::from_str::<WorkerState>(&row.state)
                else {
                    continue;
                };
                // Anything that was pending is stale before the worker can ask again.
                stale += self.stale_pending(std::slice::from_ref(&id))?;
                runs.transition_worker(
                    run,
                    &id,
                    WorkerState::Active(resume_state),
                    Cause::System,
                    IntegrationHealth::Healthy,
                    vec![Event::WorkerResumed(WorkerResumedPayload {
                        from_state: "PAUSED".to_owned(),
                    })],
                )
                .await?;
                resumed.push(id);
            }
            return Ok(Transition::Returned { resumed, stale });
        }
        Ok(Transition::None)
    }
}

#[cfg(test)]
mod tests;
