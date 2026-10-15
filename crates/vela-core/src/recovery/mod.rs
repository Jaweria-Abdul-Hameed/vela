//! Recovery (R02): after a crash or restart Vela compares what it recorded with what is actually there (worktrees, branch heads,
//! sessions, interrupted operations), classifies every difference, and resumes only what is provably safe.
//!
//! It never guesses after a divergence: a worktree or branch that is not as the journal says becomes an `EXTERNAL_DIVERGENCE`
//! intervention and stays untouched. Interventions and provider blocks are restored exactly as recorded and are never cleared or
//! retried here. Orphan sessions Vela itself created are cancelled; nothing else on the machine is ever touched.

use std::collections::BTreeSet;
use std::sync::Arc;

use vela_domain::errors::VelaError;
use vela_domain::events::{
    DiscrepancyDetectedPayload, Event, ReconciliationCompletedPayload, ReconciliationStartedPayload,
};
use vela_domain::ids::{CommitSha, RunId, WorkerId, WorktreePath};
use vela_domain::ports::{
    AgentSession, GitContext, GitRead, GitWorktrees, RepoTrust, RevisionSpec, SessionRef,
    SessionStatus,
};
use vela_domain::state::{
    Cause, IntegrationHealth, InterventionKind, PausedReason, RunPhase, RunState, WorkerState,
};
use vela_persistence::journal::NewEvent;
use vela_persistence::repositories::{o01_workers, s08_build, s12_lane};

use crate::intervention::{InterventionDetails, Interventions};
use crate::operations::{OperationLog, ReconcileReport, Reconciled};
use crate::orchestrator::Services;
use crate::orchestrator::runs::Runs;
use crate::settings::SettingsService;

pub mod continuation;
pub mod environment;
pub mod inventory;
pub mod reconcile;
pub mod resume;

/// The Git reads recovery needs.
pub trait RecoveryGit: GitRead + GitWorktrees {}

impl<T: GitRead + GitWorktrees> RecoveryGit for T {}

/// What kind of difference was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscrepancyClass {
    /// A worker's worktree is not where it was recorded.
    WorktreeMissing,
    /// A worker's branch is not at the commit the journal says.
    RefMoved,
    /// A session Vela created was still running.
    OrphanSession,
    /// The integration branch is not where the journal says, or an unfinished merge makes it unverified.
    IntegrationUnverified,
}

/// One difference between the record and reality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discrepancy {
    /// The class.
    pub class: DiscrepancyClass,
    /// What it concerns (a worker or `integration`).
    pub subject: String,
    /// What the record says.
    pub expected: String,
    /// What was found.
    pub observed: String,
}

/// A worker that recovery will not touch, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    /// The worker.
    pub worker: WorkerId,
    /// Why.
    pub reason: String,
}

/// The outcome of reconciliation.
#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryReport {
    /// What each interrupted operation turned out to be.
    pub operations: Vec<ReconcileReport>,
    /// Every difference found.
    pub discrepancies: Vec<Discrepancy>,
    /// Workers that are safe to resume (a new session at their last checkpoint).
    pub resumable: Vec<WorkerId>,
    /// Workers left exactly as they are.
    pub held: Vec<Held>,
    /// Orphan sessions that were cancelled.
    pub orphans_cancelled: Vec<WorkerId>,
}

impl RecoveryReport {
    /// True when nothing differed and nothing is held.
    pub fn is_clean(&self) -> bool {
        self.discrepancies.is_empty() && self.held.is_empty()
    }
}

/// What recovery looks at.
#[derive(Debug, Clone)]
pub struct RecoveryInput {
    /// The run.
    pub run: RunId,
    /// The project.
    pub project_id: String,
    /// The primary repository.
    pub repo: WorktreePath,
    /// Why reconciliation runs (`startup`, `resume`, ...).
    pub reason: String,
}

/// Recovery over the shared services.
pub struct Recovery {
    services: Arc<Services>,
    git: Arc<dyn RecoveryGit>,
    agent: Arc<dyn AgentSession>,
}

fn trusted_ctx(services: &Services, trust: RepoTrust) -> GitContext {
    GitContext {
        trust,
        operation: services.ids.new_operation_id(),
    }
}

impl Recovery {
    /// Recovery over its ports.
    pub fn new(
        services: Arc<Services>,
        git: Arc<dyn RecoveryGit>,
        agent: Arc<dyn AgentSession>,
    ) -> Self {
        Self {
            services,
            git,
            agent,
        }
    }

    fn append(&self, run: &RunId, worker: Option<&WorkerId>, event: Event) -> NewEvent {
        NewEvent {
            ts: self.services.now_ms(),
            run_id: Some(run.clone()),
            ticket_id: None,
            worker_id: worker.cloned(),
            event,
        }
    }

    /// The commit the journal says a worker's branch should be at: its last checkpoint, else its base.
    fn expected_head(
        &self,
        worker: &WorkerId,
        base: Option<&str>,
    ) -> Result<Option<String>, VelaError> {
        let w = worker.as_str().to_owned();
        let last: Option<String> = self.services.store.read(|c| {
            let mut stmt = c
                .prepare("SELECT payload FROM event_journal WHERE worker_id = ?1 AND kind = 'CheckpointCommitted' ORDER BY seq DESC LIMIT 1")
                .map_err(|e| VelaError::internal(e.to_string()))?;
            let mut rows = stmt.query([&w]).map_err(|e| VelaError::internal(e.to_string()))?;
            match rows.next().map_err(|e| VelaError::internal(e.to_string()))? {
                Some(r) => Ok(r.get::<_, String>(0).ok()),
                None => Ok(None),
            }
        })?;
        let from_journal = last
            .and_then(|p| serde_json::from_str::<serde_json::Value>(&p).ok())
            .and_then(|v| v.get("commit").and_then(|c| c.as_str()).map(str::to_owned));
        Ok(from_journal.or_else(|| base.map(str::to_owned)))
    }

    /// Reconciles the run with reality and applies the run's recovery-continuation choice.
    pub async fn reconcile(&self, input: &RecoveryInput) -> Result<RecoveryReport, VelaError> {
        let run = &input.run;
        let started = self.append(
            run,
            None,
            Event::ReconciliationStarted(ReconciliationStartedPayload {
                reason: input.reason.clone(),
            }),
        );
        self.services
            .store
            .write(move |tx| tx.append(&started).map(|_| ()))?;

        // 1. Interrupted operations: each recipe decides, and an operation without one is never retried.
        let operations = OperationLog::new(&self.services).reconcile_all().await?;

        let trust = crate::trust::TrustService::new(&self.services).git_trust(&input.project_id)?;
        let ctx = trusted_ctx(&self.services, trust);
        let live: BTreeSet<String> = self
            .git
            .list_worktrees(&ctx, &input.repo)
            .await?
            .into_iter()
            .map(|w| w.path.as_str().to_owned())
            .collect();

        let mut report = RecoveryReport {
            operations,
            discrepancies: Vec::new(),
            resumable: Vec::new(),
            held: Vec::new(),
            orphans_cancelled: Vec::new(),
        };
        let ivs = Interventions::new(&self.services);
        let rows = {
            let r = run.as_str().to_owned();
            self.services
                .store
                .read(|c| o01_workers::list_for_run(c, &r))?
        };
        for row in rows {
            let worker =
                WorkerId::new(row.id.clone()).map_err(|e| VelaError::internal(e.to_string()))?;
            let state: WorkerState = serde_json::from_str(&row.state).map_err(|e| {
                VelaError::internal(format!("a stored worker state is unreadable: {e}"))
            })?;
            match state {
                s if s.is_terminal() => continue,
                WorkerState::NeedsHuman { .. } => {
                    // Restored exactly as recorded; never cleared or retried here.
                    report.held.push(Held {
                        worker,
                        reason: "waiting for your decision".to_owned(),
                    });
                    continue;
                }
                WorkerState::Paused { reason, .. } => {
                    if matches!(
                        reason,
                        PausedReason::StopAll
                            | PausedReason::RecoveredAwaitingResume
                            | PausedReason::AwaitingRecoveryConfirmation
                    ) {
                        report.resumable.push(worker);
                    } else {
                        report.held.push(Held {
                            worker,
                            reason: format!("paused: {}", reason.name()),
                        });
                    }
                    continue;
                }
                _ => {}
            }
            let phase_is_provisioning = matches!(
                state,
                WorkerState::Active(vela_domain::state::WorkerPhase::Provisioning)
            );

            // A session Vela created that survived the crash is cancelled (never anything else on the machine).
            let session = SessionRef::worker(worker.clone());
            if matches!(
                self.agent.status(&session).await,
                Ok(SessionStatus::Running)
            ) {
                self.agent.cancel(&session).await?;
                report.orphans_cancelled.push(worker.clone());
                self.note(
                    run,
                    &worker,
                    DiscrepancyClass::OrphanSession,
                    "no running session",
                    "a Vela session was still running",
                    &mut report,
                )?;
            }

            let mut diverged = false;
            if !phase_is_provisioning {
                if let Some(path) = &row.worktree_path
                    && !live.contains(path)
                {
                    self.note(
                        run,
                        &worker,
                        DiscrepancyClass::WorktreeMissing,
                        path,
                        "no such worktree",
                        &mut report,
                    )?;
                    diverged = true;
                }
                if !diverged
                    && let (Some(branch), Some(expected)) = (
                        &row.branch,
                        self.expected_head(&worker, row.base_sha.as_deref())?,
                    )
                {
                    let observed = match RevisionSpec::new(branch.as_str()) {
                        Ok(spec) => self
                            .git
                            .resolve_ref(&ctx, &input.repo, &spec)
                            .await
                            .map(|c| c.as_str().to_owned()),
                        Err(e) => Err(VelaError::internal(e.to_string())),
                    };
                    match observed {
                        Ok(head) if head == expected => {}
                        Ok(head) => {
                            self.note(
                                run,
                                &worker,
                                DiscrepancyClass::RefMoved,
                                &expected,
                                &head,
                                &mut report,
                            )?;
                            diverged = true;
                        }
                        Err(_) => {
                            self.note(
                                run,
                                &worker,
                                DiscrepancyClass::RefMoved,
                                &expected,
                                "the branch is missing",
                                &mut report,
                            )?;
                            diverged = true;
                        }
                    }
                }
            }
            if diverged {
                ivs.raise_for_worker(
                    run,
                    &worker,
                    InterventionKind::ExternalDivergence,
                    InterventionDetails {
                        summary: format!(
                            "{} was changed outside Vela while it was not running",
                            row.ticket_id
                        ),
                        evidence: report
                            .discrepancies
                            .iter()
                            .filter(|d| d.subject == worker.as_str())
                            .map(|d| {
                                format!(
                                    "{}: expected {}, found {}",
                                    d.subject, d.expected, d.observed
                                )
                            })
                            .collect(),
                        preserved_work:
                            "nothing was touched; the worktree and branch are exactly as found"
                                .to_owned(),
                        last_safe_commit: row
                            .base_sha
                            .as_deref()
                            .and_then(|s| CommitSha::new(s).ok()),
                        actions: vec!["inspect it, then resume or cancel".to_owned()],
                        consequences: "Vela will not continue this ticket on a guess".to_owned(),
                    },
                )
                .await?;
                report.held.push(Held {
                    worker,
                    reason: "diverged from the journal".to_owned(),
                });
            } else {
                report.resumable.push(worker);
            }
        }

        // 2. The integration branch: unfinished merges and unexplained heads make it UNKNOWN until a person decides.
        self.check_integration(input, &ctx, &mut report).await?;

        let (count, outcome) = (
            u32::try_from(report.discrepancies.len()).unwrap_or(u32::MAX),
            if report.is_clean() {
                "clean"
            } else {
                "needs attention"
            },
        );
        let done = self.append(
            run,
            None,
            Event::ReconciliationCompleted(ReconciliationCompletedPayload {
                discrepancies: count,
                outcome: outcome.to_owned(),
            }),
        );
        self.services
            .store
            .write(move |tx| tx.append(&done).map(|_| ()))?;

        self.apply_continuation(run, &report).await?;
        Ok(report)
    }

    fn note(
        &self,
        run: &RunId,
        worker: &WorkerId,
        class: DiscrepancyClass,
        expected: &str,
        observed: &str,
        report: &mut RecoveryReport,
    ) -> Result<(), VelaError> {
        let ev = self.append(
            run,
            Some(worker),
            Event::DiscrepancyDetected(DiscrepancyDetectedPayload {
                subject: format!("{class:?}"),
                expected: expected.to_owned(),
                observed: observed.to_owned(),
            }),
        );
        self.services
            .store
            .write(move |tx| tx.append(&ev).map(|_| ()))?;
        report.discrepancies.push(Discrepancy {
            class,
            subject: worker.as_str().to_owned(),
            expected: expected.to_owned(),
            observed: observed.to_owned(),
        });
        Ok(())
    }

    async fn check_integration(
        &self,
        input: &RecoveryInput,
        ctx: &GitContext,
        report: &mut RecoveryReport,
    ) -> Result<(), VelaError> {
        let run = &input.run;
        let r = run.as_str().to_owned();
        let Some(base) = self
            .services
            .store
            .read(|c| s08_build::get_run_base(c, &r))?
        else {
            return Ok(());
        };
        let health = self.services.store.read(|c| s12_lane::get_health(c, &r))?;
        let unresolved_merge = report
            .operations
            .iter()
            .any(|o| matches!(o.outcome, Reconciled::NeedsHuman(_)) && self.is_merge(&o.id));
        let expected = self
            .last_integration_head(run)?
            .unwrap_or_else(|| base.base_sha.clone());
        let observed = match RevisionSpec::new("HEAD") {
            Ok(spec) => self
                .git
                .resolve_ref(
                    ctx,
                    &WorktreePath::new(base.integration_worktree.clone())
                        .map_err(|e| VelaError::internal(e.to_string()))?,
                    &spec,
                )
                .await
                .map(|c| c.as_str().to_owned())
                .unwrap_or_else(|_| "unreadable".to_owned()),
            Err(_) => "unreadable".to_owned(),
        };
        let moved = observed != expected;
        if moved || unresolved_merge || health != IntegrationHealth::Healthy {
            let why = if unresolved_merge {
                "a merge was interrupted"
            } else if moved {
                "the integration head moved"
            } else {
                "integration was not healthy"
            };
            let ev = self.append(
                run,
                None,
                Event::DiscrepancyDetected(DiscrepancyDetectedPayload {
                    subject: "integration".to_owned(),
                    expected: expected.clone(),
                    observed: format!("{observed} ({why})"),
                }),
            );
            let r2 = r.clone();
            self.services.store.write(move |tx| {
                s12_lane::set_health(tx.conn(), &r2, IntegrationHealth::Unknown)?;
                tx.append(&ev).map(|_| ())
            })?;
            report.discrepancies.push(Discrepancy {
                class: DiscrepancyClass::IntegrationUnverified,
                subject: "integration".to_owned(),
                expected,
                observed: format!("{observed} ({why})"),
            });
            // A run that is running takes a human gate; one already held keeps the gate it has.
            let _ = Interventions::new(&self.services)
                .raise_for_run(
                    run,
                    InterventionKind::IntegrationUnknown,
                    InterventionDetails {
                        summary: format!("the integration branch cannot be trusted: {why}"),
                        evidence: vec![format!(
                            "expected {}, found {observed}",
                            report
                                .discrepancies
                                .last()
                                .map_or("", |d| d.expected.as_str())
                        )],
                        preserved_work: "every worker branch is untouched".to_owned(),
                        last_safe_commit: CommitSha::new(base.base_sha.clone()).ok(),
                        actions: vec![
                            "verify or restore the integration worktree, then resume".to_owned(),
                        ],
                        consequences: "no merge or new worker starts until integration is verified"
                            .to_owned(),
                    },
                )
                .await;
        }
        Ok(())
    }

    fn is_merge(&self, op_id: &str) -> bool {
        let id = op_id.to_owned();
        self.services
            .store
            .read(|c| vela_persistence::repositories::r01_operations::get(c, &id))
            .ok()
            .flatten()
            .is_some_and(|o| o.kind == "merge")
    }

    fn last_integration_head(&self, run: &RunId) -> Result<Option<String>, VelaError> {
        let r = run.as_str().to_owned();
        self.services.store.read(|c| {
            let mut stmt = c
                .prepare("SELECT payload FROM event_journal WHERE run_id = ?1 AND kind = 'MergeCompleted' ORDER BY seq DESC LIMIT 1")
                .map_err(|e| VelaError::internal(e.to_string()))?;
            let mut rows = stmt.query([&r]).map_err(|e| VelaError::internal(e.to_string()))?;
            Ok(match rows.next().map_err(|e| VelaError::internal(e.to_string()))? {
                Some(row) => row
                    .get::<_, String>(0)
                    .ok()
                    .and_then(|p| serde_json::from_str::<serde_json::Value>(&p).ok())
                    .and_then(|v| v.get("integration_after").and_then(|c| c.as_str()).map(str::to_owned)),
                None => None,
            })
        })
    }

    /// Applies the user's recovery-continuation choice to a run that was running when Vela stopped: `ask` always pauses for a
    /// confirmation; `auto_safe` continues only when reconciliation found nothing at all.
    async fn apply_continuation(
        &self,
        run: &RunId,
        report: &RecoveryReport,
    ) -> Result<(), VelaError> {
        let runs = Runs::new(&self.services);
        if runs.run_state(run)? != RunState::Active(RunPhase::Running) {
            return Ok(());
        }
        let choice =
            SettingsService::new(&self.services).effective(None)?["recovery_continuation"].clone();
        let gate = self
            .services
            .get::<continuation::InteractiveGate>()
            .map(|g| *g)
            .unwrap_or_default();
        let reason = match continuation::decide(
            continuation::Policy::parse(&choice),
            report.is_clean(),
            gate,
        ) {
            continuation::Continuation::Resume => return Ok(()),
            continuation::Continuation::Pause(reason) => reason,
        };
        let health = self
            .services
            .store
            .read(|c| s12_lane::get_health(c, run.as_str()))?;
        runs.transition_run(
            run,
            RunState::Paused {
                reason,
                resume_state: RunPhase::Running,
            },
            Cause::System,
            health,
            Vec::new(),
        )
        .await
        .map(|_| ())
    }
}

#[cfg(test)]
mod tests;
