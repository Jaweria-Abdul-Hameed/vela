//! Stop All, pause and resume (R04, R08): halt every running worker safely, with the work preserved, and resume only after
//! reconciliation.
//!
//! Only sessions Vela created are touched: each is cancelled gracefully through its session reference and, when it will not stop,
//! its process tree is killed by the id Vela gave it. Nothing else on the machine is addressed. A merge or push in flight reaches
//! a safe point first: validation finishes or the unpublished merge is discarded to the recorded commit, a push completes or is
//! abandoned; when that cannot be shown within the bounded wait, integration health becomes `UNKNOWN` and reconciliation decides.

use std::sync::Arc;
use std::time::Duration;

use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::events::{
    Event, RunStoppingPayload, StopRequestedPayload, WorkerPausedPayload, WorkerResumedPayload,
};
use vela_domain::fault::{self, FaultPoint};
use vela_domain::ids::{CommitSha, RunId, WorkerId, WorktreePath};
use vela_domain::ports::{AgentSession, GitContext, ProcessRunner, SessionRef, SessionStatus};
use vela_domain::state::{Cause, IntegrationHealth, PausedReason, RunPhase, RunState, WorkerState};
use vela_persistence::repositories::{o01_workers, r01_operations, s12_lane};

use crate::merge_lane::{LaneGit, MergeLane};
use crate::operations::OperationLog;
use crate::orchestrator::Services;
use crate::orchestrator::runs::Runs;
use crate::recovery::RecoveryReport;
use crate::trust::TrustService;

pub mod safe_points;

/// Whether the merge lane is in the middle of a merge for a run.
pub trait LaneProbe: Send + Sync {
    /// True while a merge of `run` is running.
    fn busy(&self, run: &RunId) -> bool;
}

impl LaneProbe for MergeLane {
    fn busy(&self, run: &RunId) -> bool {
        self.is_busy(run)
    }
}

/// What a lane operation did at the safe point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaneSafePoint {
    /// Nothing was in flight.
    Idle,
    /// An in-flight merge finished by itself within the bounded wait.
    Finished,
    /// An unfinished merge was discarded to its recorded commit, and unfinished pushes abandoned.
    Discarded,
    /// The safe point could not be shown; integration health is now `UNKNOWN`.
    Unknown,
}

/// The result of Stop All.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopReport {
    /// Workers paused with their work preserved.
    pub paused: Vec<WorkerId>,
    /// Workers whose session needed its process tree killed.
    pub forced: Vec<WorkerId>,
    /// How the merge lane was left.
    pub lane: LaneSafePoint,
}

/// Stop All over its ports.
pub struct StopService {
    services: Arc<Services>,
    agent: Arc<dyn AgentSession>,
    runner: Arc<dyn ProcessRunner>,
    git: Arc<dyn LaneGit>,
    lane: Arc<dyn LaneProbe>,
    lane_wait: Duration,
}

impl StopService {
    /// A stop service; `lane_wait` bounds how long a merge in flight may finish by itself.
    pub fn new(
        services: Arc<Services>,
        agent: Arc<dyn AgentSession>,
        runner: Arc<dyn ProcessRunner>,
        git: Arc<dyn LaneGit>,
        lane: Arc<dyn LaneProbe>,
        lane_wait: Duration,
    ) -> Self {
        Self {
            services,
            agent,
            runner,
            git,
            lane,
            lane_wait,
        }
    }

    fn health(&self, run: &RunId) -> Result<IntegrationHealth, VelaError> {
        let r = run.as_str().to_owned();
        self.services.store.read(|c| s12_lane::get_health(c, &r))
    }

    fn set_health(&self, run: &RunId, h: IntegrationHealth) -> Result<(), VelaError> {
        let r = run.as_str().to_owned();
        self.services
            .store
            .write(move |tx| s12_lane::set_health(tx.conn(), &r, h))
    }

    async fn settle_lane(
        &self,
        run: &RunId,
        project_id: &str,
        integration: &WorktreePath,
    ) -> Result<LaneSafePoint, VelaError> {
        let mut waited = Duration::ZERO;
        let had_merge = self.lane.busy(run);
        while self.lane.busy(run) && waited < self.lane_wait {
            tokio::time::sleep(Duration::from_millis(5)).await;
            waited += Duration::from_millis(5);
        }
        if self.lane.busy(run) {
            fault::hit(FaultPoint::StopBoundedWaitExpired);
            self.set_health(run, IntegrationHealth::Unknown)?;
            return Ok(LaneSafePoint::Unknown);
        }
        // Whatever the lane left half-done: discard an unpublished merge, abandon an unfinished push.
        let r = run.as_str().to_owned();
        let pending: Vec<_> = self
            .services
            .store
            .read(r01_operations::list_intended)?
            .into_iter()
            .filter(|o| {
                o.run_id.as_deref() == Some(r.as_str())
                    && matches!(o.kind.as_str(), "merge" | "push")
            })
            .collect();
        if pending.is_empty() {
            return Ok(if had_merge {
                LaneSafePoint::Finished
            } else {
                LaneSafePoint::Idle
            });
        }
        fault::hit(FaultPoint::StopDiscardDecision);
        let ops = OperationLog::new(&self.services);
        let ctx = GitContext {
            trust: TrustService::new(&self.services).git_trust(project_id)?,
            operation: self.services.ids.new_operation_id(),
        };
        let mut unknown = false;
        for op in pending {
            let id = vela_domain::ids::OperationId::new(op.id.clone())
                .map_err(|e| VelaError::internal(e.to_string()))?;
            if op.kind == "push" {
                ops.finish(
                    &id,
                    false,
                    &serde_json::json!({"abandoned": "stopped before the push was recorded"}),
                )?;
                continue;
            }
            let before = serde_json::from_str::<serde_json::Value>(&op.intent)
                .ok()
                .and_then(|v| v.get("before").and_then(|b| b.as_str()).map(str::to_owned))
                .and_then(|s| CommitSha::new(s).ok());
            let discarded = match &before {
                Some(b) => {
                    let _ = self.git.abort_merge(&ctx, integration).await;
                    self.git
                        .discard_unpublished_merge(&ctx, integration, b)
                        .await
                        .is_ok()
                }
                None => false,
            };
            if discarded {
                ops.finish(
                    &id,
                    false,
                    &serde_json::json!({"discarded": "to the recorded integration_before"}),
                )?;
            } else {
                unknown = true;
            }
        }
        if unknown {
            self.set_health(run, IntegrationHealth::Unknown)?;
            Ok(LaneSafePoint::Unknown)
        } else {
            self.set_health(run, IntegrationHealth::Healthy)?;
            Ok(LaneSafePoint::Discarded)
        }
    }

    /// Stops every running worker of `run` and pauses the run with reason `STOP_ALL`.
    pub async fn stop_all(
        &self,
        run: &RunId,
        project_id: &str,
        integration: &WorktreePath,
    ) -> Result<StopReport, VelaError> {
        let runs = Runs::new(&self.services);
        let r = run.as_str().to_owned();
        let rows = self
            .services
            .store
            .read(|c| o01_workers::list_for_run(c, &r))?;
        let mut active = Vec::new();
        for row in rows {
            if let Ok(WorkerState::Active(phase)) = serde_json::from_str::<WorkerState>(&row.state)
            {
                active.push((
                    WorkerId::new(row.id).map_err(|e| VelaError::internal(e.to_string()))?,
                    phase,
                ));
            }
        }
        let health = self.health(run)?;
        // A stop the process died in the middle of is carried on from where it was: the run is already `STOPPING`.
        let resume_state = match runs.run_state(run)? {
            RunState::Stopping { resume_state, .. } => resume_state,
            _ => {
                runs.transition_run(
                    run,
                    RunState::Stopping {
                        resume_state: RunPhase::Running,
                        gate: None,
                    },
                    Cause::UserDecision,
                    health,
                    vec![
                        Event::StopRequested(StopRequestedPayload {
                            requested_by: "user".to_owned(),
                        }),
                        Event::RunStopping(RunStoppingPayload {
                            active_workers: u32::try_from(active.len()).unwrap_or(u32::MAX),
                        }),
                    ],
                )
                .await?;
                RunPhase::Running
            }
        };

        let lane = self.settle_lane(run, project_id, integration).await?;

        let mut report = StopReport {
            paused: Vec::new(),
            forced: Vec::new(),
            lane,
        };
        for (worker, phase) in active {
            let session = SessionRef::worker(worker.clone());
            let _ = self.agent.cancel(&session).await;
            if matches!(
                self.agent.status(&session).await,
                Ok(SessionStatus::Running)
            ) {
                // The session would not stop: kill the tree Vela started for it, by the id Vela gave it, and nothing else.
                self.runner.kill_tree(worker.as_str()).await?;
                report.forced.push(worker.clone());
            }
            runs.transition_worker(
                run,
                &worker,
                WorkerState::Paused {
                    reason: PausedReason::StopAll,
                    resume_state: phase,
                },
                Cause::UserDecision,
                self.health(run)?,
                vec![Event::WorkerPaused(WorkerPausedPayload {
                    reason: "stop all".to_owned(),
                })],
            )
            .await?;
            report.paused.push(worker);
        }
        runs.transition_run(
            run,
            RunState::Paused {
                reason: PausedReason::StopAll,
                resume_state,
            },
            Cause::System,
            self.health(run)?,
            Vec::new(),
        )
        .await?;
        Ok(report)
    }

    /// After a restart: a run left in `STOPPING` has its stop finished (workers paused, the merge lane settled, the run paused with
    /// reason `STOP_ALL`). Returns the report when there was a stop to finish.
    pub async fn finish_interrupted(
        &self,
        run: &RunId,
        project_id: &str,
        integration: &WorktreePath,
    ) -> Result<Option<StopReport>, VelaError> {
        if matches!(
            Runs::new(&self.services).run_state(run)?,
            RunState::Stopping { .. }
        ) {
            return self.stop_all(run, project_id, integration).await.map(Some);
        }
        Ok(None)
    }

    /// Resumes a run paused by Stop All, only after reconciliation found nothing wrong and integration is healthy.
    pub async fn resume(
        &self,
        run: &RunId,
        reconciled: &RecoveryReport,
    ) -> Result<Vec<WorkerId>, VelaError> {
        let runs = Runs::new(&self.services);
        if !reconciled.discrepancies.is_empty() || !reconciled.held.is_empty() {
            return Err(VelaError::of_class(
                ErrorClass::UserActionRequired,
                "reconciliation found something that needs a decision, so the run is not resumed",
            )
            .with_user_action("resolve the open interventions, then resume"));
        }
        let health = self.health(run)?;
        if health != IntegrationHealth::Healthy {
            return Err(VelaError::of_class(
                ErrorClass::UserActionRequired,
                "integration is not verified healthy, so the run is not resumed",
            ));
        }
        let mut resumed = Vec::new();
        for worker in &reconciled.resumable {
            if let WorkerState::Paused {
                reason: PausedReason::StopAll,
                resume_state,
            } = runs.worker_status(worker)?.state()
            {
                runs.transition_worker(
                    run,
                    worker,
                    WorkerState::Active(resume_state),
                    Cause::UserDecision,
                    health,
                    vec![Event::WorkerResumed(WorkerResumedPayload {
                        from_state: "PAUSED".to_owned(),
                    })],
                )
                .await?;
                resumed.push(worker.clone());
            }
        }
        runs.transition_run(
            run,
            RunState::Active(RunPhase::Running),
            Cause::UserDecision,
            health,
            Vec::new(),
        )
        .await?;
        Ok(resumed)
    }
}

#[cfg(test)]
mod tests;
