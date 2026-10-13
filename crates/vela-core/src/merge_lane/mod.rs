//! The merge lane (S12): reviewed worker branches enter the integration branch one at a time, through deterministic code.
//!
//! Per merge: take the lane lock, record the intent, mark integration health `UNKNOWN`, merge (non-fast-forward, bound to the
//! reviewed head and the recorded reset point), validate the integration worktree, and either keep the merge (health back to
//! `HEALTHY`, then push) or discard it to the recorded commit and send the worker back to fixing. A merge that stopped on a
//! textual conflict is aborted and the ticket escalates; the lane never completes a conflicted merge itself.

use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::{Arc, Mutex};

use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::events::{
    CommandFinishedPayload, CommandStartedPayload, ConflictResolutionStartedPayload, Event,
    FailedGate, IntegrationGateFailedPayload, MergeBinding, MergeCompletedPayload,
    MergeDiscardedPayload, MergeStartedPayload, PushCompletedPayload, PushFailedPayload,
    TestGateFinishedPayload,
};
use vela_domain::fault::{self, FaultPoint};
use vela_domain::ids::{
    BranchName, CommitSha, GateResult, GateStatus, RunId, TicketId, WorkerId, WorktreePath,
};
use vela_domain::policy::{Decision, Dialect, Operation, PolicyEngine, ProfileEntry, RunMode};
use vela_domain::ports::{
    CommandSpec, GitContext, GitMerges, GitRead, GitRemote, MergeOutcome, OutputChunk, OutputSink,
    PortFuture, ProcessRunner, RemoteName, RevisionSpec,
};
use vela_domain::state::{Cause, IntegrationHealth, InterventionKind, WorkerPhase, WorkerState};
use vela_persistence::journal::NewEvent;
use vela_persistence::repositories::s12_lane;

use crate::intervention::{InterventionDetails, Interventions};
use crate::operations::{OperationLog, Recipe, Reconciled};
use crate::orchestrator::Services;
use crate::orchestrator::runs::Runs;
use crate::settings::SettingsService;
use crate::trust::{ProfileCommand, TrustService};

pub mod conflict;

/// The Git operations the lane needs.
pub trait LaneGit: GitRead + GitMerges + GitRemote {}

impl<T: GitRead + GitMerges + GitRemote> LaneGit for T {}

/// One merge request.
#[derive(Debug, Clone)]
pub struct LaneRequest {
    /// The run.
    pub run: RunId,
    /// The project.
    pub project_id: String,
    /// The worker whose branch is merged.
    pub worker: WorkerId,
    /// Its ticket.
    pub ticket: TicketId,
    /// The worker branch.
    pub branch: BranchName,
    /// The commit that passed review (here: the checkpoint); the merge is refused if the branch moved.
    pub reviewed_head: CommitSha,
    /// The integration worktree.
    pub integration: WorktreePath,
    /// The integration branch (the one pushed).
    pub integration_branch: BranchName,
    /// Gates run in the integration worktree after the merge; each must be in the confirmed profile.
    pub validation: Vec<ProfileCommand>,
    /// The remote to push to after validation, when there is one.
    pub remote: Option<RemoteName>,
}

/// How a merge ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaneOutcome {
    /// Merged and validated; integration is healthy again.
    Merged {
        /// The integration head after the merge.
        head: CommitSha,
        /// True when the push to the remote succeeded (false when there is no remote or it failed; see the journal).
        pushed: bool,
    },
    /// Validation failed; the merge was discarded and the worker is back in fixing.
    SentBackToFix {
        /// The failing gate.
        gate: String,
        /// How many times this worker has now been sent back.
        attempts: u32,
    },
    /// The merge conflicted; it was aborted and a human must decide.
    Conflict {
        /// The conflicting paths.
        paths: Vec<String>,
        /// The intervention.
        intervention_id: String,
    },
    /// Validation kept failing past the cap; a human must decide.
    FixCapReached {
        /// The intervention.
        intervention_id: String,
    },
}

#[derive(Default)]
struct Tail(Mutex<Vec<String>>);

impl OutputSink for Tail {
    fn on_output(&self, chunk: OutputChunk) {
        let mut lines = self.0.lock().expect("tail lock");
        lines.extend(chunk.text.lines().map(str::to_owned));
        let excess = lines.len().saturating_sub(30);
        lines.drain(..excess);
    }
}

/// What running a list of gates found.
#[derive(Debug, Clone)]
pub(crate) struct GateRun {
    /// Every gate that ran, in order (it stops at the first failure).
    pub results: Vec<GateResult>,
    /// The failing gate with its output tail, if any.
    pub failure: Option<(GateResult, String)>,
}

/// Runs `commands` in `root` through the process port, each checked by the policy engine first. Journals the command events and
/// gate results as `run`/`ticket` events.
pub(crate) async fn run_gates(
    services: &Services,
    runner: &dyn ProcessRunner,
    project_id: &str,
    run: &RunId,
    ticket: Option<&TicketId>,
    root: &str,
    commands: &[ProfileCommand],
) -> Result<GateRun, VelaError> {
    let context =
        TrustService::new(services).policy_context(project_id, root, RunMode::Supervised)?;
    let engine = PolicyEngine::new(context);
    let mut out = GateRun {
        results: Vec::new(),
        failure: None,
    };
    for c in commands {
        let entry =
            ProfileEntry::parse(&c.text, Dialect::Cmd, c.cwd.as_deref(), root).map_err(|e| {
                VelaError::of_class(
                    ErrorClass::ValidationFailure,
                    format!("`{}` is not a valid gate here: {e}", c.text),
                )
            })?;
        let decision = engine.evaluate(&Operation::Command {
            dialect: Dialect::Cmd,
            text: entry.canonical_text(),
            cwd: Some(entry.cwd().to_owned()),
        });
        if decision.decision != Decision::Allow {
            return Err(VelaError::of_class(
                ErrorClass::PolicyBlock,
                format!(
                    "the policy does not allow the gate `{}`: {}",
                    c.text, decision.reason
                ),
            ));
        }
        let id = format!("gate-{}", services.ids.new_token());
        let cwd = WorktreePath::new(entry.cwd())
            .map_err(|e| VelaError::internal(format!("the gate directory is invalid: {e}")))?;
        let tail = Tail::default();
        let started = event(
            services,
            run,
            ticket,
            Event::CommandStarted(CommandStartedPayload {
                command_id: id.clone(),
                command: entry.canonical_text(),
                cwd: Some(cwd.clone()),
            }),
        );
        services
            .store
            .write(move |tx| tx.append(&started).map(|_| ()))?;
        let outcome = runner
            .run(
                &CommandSpec {
                    id: id.clone(),
                    program: entry.executable().to_owned(),
                    args: entry.args().to_vec(),
                    cwd,
                    env: Vec::new(),
                    timeout_ms: NonZeroU64::new(15 * 60 * 1000).expect("non-zero"),
                },
                &tail,
            )
            .await?;
        let passed = !outcome.timed_out && outcome.exit_code == Some(0);
        let gate = GateResult::new(
            entry.canonical_text(),
            if passed {
                GateStatus::Pass
            } else {
                GateStatus::Fail
            },
            outcome.exit_code,
            Some(outcome.duration_ms),
            if passed {
                "passed"
            } else if outcome.timed_out {
                "timed out"
            } else {
                "failed"
            },
        )
        .map_err(|e| VelaError::internal(format!("the gate result is inconsistent: {e}")))?;
        let finished = [
            event(
                services,
                run,
                ticket,
                Event::CommandFinished(CommandFinishedPayload {
                    command_id: id,
                    exit_code: outcome.exit_code,
                    duration_ms: outcome.duration_ms,
                    timed_out: outcome.timed_out,
                }),
            ),
            event(
                services,
                run,
                ticket,
                Event::TestGateFinished(TestGateFinishedPayload {
                    result: gate.clone(),
                }),
            ),
        ];
        services.store.write(move |tx| {
            for e in &finished {
                tx.append(e)?;
            }
            Ok(())
        })?;
        out.results.push(gate.clone());
        if !passed {
            let output = tail.0.lock().expect("tail lock").join("\n");
            out.failure = Some((gate, output));
            break;
        }
    }
    Ok(out)
}

pub(crate) fn event(
    services: &Services,
    run: &RunId,
    ticket: Option<&TicketId>,
    event: Event,
) -> NewEvent {
    NewEvent {
        ts: services.now_ms(),
        run_id: Some(run.clone()),
        ticket_id: ticket.cloned(),
        worker_id: None,
        event,
    }
}

/// The merge lane.
pub struct MergeLane {
    services: Arc<Services>,
    git: Arc<dyn LaneGit>,
    runner: Arc<dyn ProcessRunner>,
    locks: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl MergeLane {
    /// A lane over its ports.
    pub fn new(
        services: Arc<Services>,
        git: Arc<dyn LaneGit>,
        runner: Arc<dyn ProcessRunner>,
    ) -> Self {
        Self {
            services,
            git,
            runner,
            locks: Mutex::new(HashMap::new()),
        }
    }

    fn lock_for(&self, run: &RunId) -> Arc<tokio::sync::Mutex<()>> {
        self.locks
            .lock()
            .expect("locks lock")
            .entry(run.as_str().to_owned())
            .or_default()
            .clone()
    }

    fn ctx(&self, project_id: &str) -> Result<GitContext, VelaError> {
        Ok(GitContext {
            trust: TrustService::new(&self.services).git_trust(project_id)?,
            operation: self.services.ids.new_operation_id(),
        })
    }

    /// True while a merge of `run` is running (the lane lock is held).
    pub fn is_busy(&self, run: &RunId) -> bool {
        self.lock_for(run).try_lock().is_err()
    }

    /// The run's persisted integration health.
    pub fn health(&self, run: &RunId) -> Result<IntegrationHealth, VelaError> {
        let r = run.as_str().to_owned();
        self.services.store.read(|c| s12_lane::get_health(c, &r))
    }

    fn set_health(&self, run: &RunId, health: IntegrationHealth) -> Result<(), VelaError> {
        let r = run.as_str().to_owned();
        self.services
            .store
            .write(move |tx| s12_lane::set_health(tx.conn(), &r, health))
    }

    async fn worker_to(
        &self,
        r: &LaneRequest,
        to: WorkerState,
        events: Vec<Event>,
    ) -> Result<(), VelaError> {
        let health = self.health(&r.run)?;
        Runs::new(&self.services)
            .transition_worker(&r.run, &r.worker, to, Cause::System, health, events)
            .await
            .map(|_| ())
    }

    /// Merges one reviewed worker branch. Merges of one run are serialized by the lane lock.
    pub async fn merge(&self, r: &LaneRequest) -> Result<LaneOutcome, VelaError> {
        let lock = self.lock_for(&r.run);
        let _guard = lock.lock().await;
        let ctx = self.ctx(&r.project_id)?;

        let before = self
            .git
            .resolve_ref(
                &ctx,
                &r.integration,
                &RevisionSpec::new("HEAD").map_err(|e| VelaError::internal(e.to_string()))?,
            )
            .await?;

        // Intent first, then everything that can change the integration branch.
        let ops = OperationLog::new(&self.services);
        let op = ctx.operation.clone();
        ops.begin(
            &op,
            Some(&r.run),
            "merge",
            &serde_json::json!({
                "integration": r.integration.as_str(),
                "before": before.as_str(),
                "source": r.branch.as_str(),
                "reviewed_head": r.reviewed_head.as_str(),
            }),
        )?;
        self.worker_to(
            r,
            WorkerState::Active(WorkerPhase::Merging),
            vec![Event::MergeStarted(MergeStartedPayload {
                source_branch: r.branch.clone(),
                reviewed_head: r.reviewed_head.clone(),
                integration_before: before.clone(),
            })],
        )
        .await?;
        self.set_health(&r.run, IntegrationHealth::Unknown)?;
        fault::hit(FaultPoint::LaneBeforeMerge);

        let merged = match self
            .git
            .merge(&ctx, &r.integration, &before, &r.branch, &r.reviewed_head)
            .await
        {
            Ok(m) => m,
            Err(e) => {
                // The merge was refused or failed: put the lane back where it was, then report.
                self.restore(r, &ctx, &before).await;
                ops.finish(&op, false, &serde_json::json!({"error": e.what_failed()}))?;
                return Err(e);
            }
        };
        let (head, merged_source) = match merged {
            MergeOutcome::Merged {
                head,
                merged_source,
            } => (head, merged_source),
            MergeOutcome::Conflicted { paths } => {
                self.git.abort_merge(&ctx, &r.integration).await?;
                self.set_health(&r.run, IntegrationHealth::Healthy)?;
                ops.finish(&op, false, &serde_json::json!({"conflict": paths}))?;
                let id = self.escalate_conflict(r, &paths).await?;
                return Ok(LaneOutcome::Conflict {
                    paths,
                    intervention_id: id,
                });
            }
        };
        fault::hit(FaultPoint::LaneAfterMerge);

        // INTEGRATION_VALIDATION
        self.worker_to(
            r,
            WorkerState::Active(WorkerPhase::IntegrationValidation),
            Vec::new(),
        )
        .await?;
        let gates = run_gates(
            &self.services,
            self.runner.as_ref(),
            &r.project_id,
            &r.run,
            Some(&r.ticket),
            r.integration.as_str(),
            &r.validation,
        )
        .await;
        fault::hit(FaultPoint::LaneAfterValidation);
        let gates = match gates {
            Ok(g) => g,
            Err(e) => {
                self.restore(r, &ctx, &before).await;
                ops.finish(&op, false, &serde_json::json!({"error": e.what_failed()}))?;
                return Err(e);
            }
        };

        let binding = MergeBinding::new(r.reviewed_head.clone(), merged_source)
            .map_err(|e| VelaError::internal(format!("the merge binding is inconsistent: {e}")))?;

        if let Some((failed, output)) = gates.failure {
            return self
                .fail_and_discard(r, &ctx, &before, binding, failed, output, &op)
                .await;
        }

        // Valid: the merge stays. Health returns to HEALTHY with the worker's completion in one story.
        self.set_health(&r.run, IntegrationHealth::Healthy)?;
        let completed = event(
            &self.services,
            &r.run,
            Some(&r.ticket),
            Event::MergeCompleted(MergeCompletedPayload {
                source_branch: r.branch.clone(),
                binding,
                integration_before: before.clone(),
                integration_after: head.clone(),
            }),
        );
        self.services
            .store
            .write(move |tx| tx.append(&completed).map(|_| ()))?;
        self.worker_to(r, WorkerState::Complete, Vec::new()).await?;
        ops.finish(&op, true, &serde_json::json!({"head": head.as_str()}))?;

        let pushed = self.push(r, &ctx).await;
        Ok(LaneOutcome::Merged { head, pushed })
    }

    /// Best-effort return to the pre-merge state after an error; marks health unhealthy when that cannot be done.
    async fn restore(&self, r: &LaneRequest, ctx: &GitContext, before: &CommitSha) {
        let _ = self.git.abort_merge(ctx, &r.integration).await;
        let reset = self
            .git
            .discard_unpublished_merge(ctx, &r.integration, before)
            .await;
        let health = match (reset, self.head_is(r, ctx, before).await) {
            (Ok(()), _) | (_, true) => IntegrationHealth::Healthy,
            _ => IntegrationHealth::Unhealthy,
        };
        let _ = self.set_health(&r.run, health);
    }

    async fn head_is(&self, r: &LaneRequest, ctx: &GitContext, expected: &CommitSha) -> bool {
        match RevisionSpec::new("HEAD") {
            Ok(spec) => self
                .git
                .resolve_ref(ctx, &r.integration, &spec)
                .await
                .is_ok_and(|h| &h == expected),
            Err(_) => false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn fail_and_discard(
        &self,
        r: &LaneRequest,
        ctx: &GitContext,
        before: &CommitSha,
        binding: MergeBinding,
        failed: GateResult,
        output: String,
        op: &vela_domain::ids::OperationId,
    ) -> Result<LaneOutcome, VelaError> {
        let gate_name = failed.gate().to_owned();
        let failed_event = event(
            &self.services,
            &r.run,
            Some(&r.ticket),
            Event::IntegrationGateFailed(IntegrationGateFailedPayload {
                source_branch: r.branch.clone(),
                binding,
                integration_before: before.clone(),
                result: FailedGate::new(failed).map_err(|e| {
                    VelaError::internal(format!("the failing gate is inconsistent: {e}"))
                })?,
            }),
        );
        self.services
            .store
            .write(move |tx| tx.append(&failed_event).map(|_| ()))?;

        // Discard the unpublished merge back to the recorded commit.
        let discarded = self
            .git
            .discard_unpublished_merge(ctx, &r.integration, before)
            .await;
        fault::hit(FaultPoint::LaneAfterDiscard);
        let ops = OperationLog::new(&self.services);
        if let Err(e) = discarded {
            self.set_health(&r.run, IntegrationHealth::Unhealthy)?;
            ops.finish(
                op,
                false,
                &serde_json::json!({"discard_failed": e.what_failed()}),
            )?;
            Interventions::new(&self.services)
                .raise_for_run(
                    &r.run,
                    InterventionKind::IntegrationUnhealthy,
                    InterventionDetails {
                        summary: "a failed merge could not be discarded; the integration branch is not trustworthy".to_owned(),
                        evidence: vec![e.what_failed().to_owned(), output],
                        preserved_work: format!("branch {} is untouched", r.branch),
                        last_safe_commit: Some(before.clone()),
                        actions: vec!["inspect the integration worktree and restore it to the safe commit".to_owned()],
                        consequences: "no worker starts and no merge happens until the integration branch is restored".to_owned(),
                    },
                )
                .await?;
            return Err(e);
        }
        self.set_health(&r.run, IntegrationHealth::Healthy)?;
        let discarded_event = event(
            &self.services,
            &r.run,
            Some(&r.ticket),
            Event::MergeDiscarded(MergeDiscardedPayload {
                source_branch: r.branch.clone(),
                integration_before: before.clone(),
                reason: format!("integration validation failed at `{gate_name}`"),
            }),
        );
        self.services
            .store
            .write(move |tx| tx.append(&discarded_event).map(|_| ()))?;
        ops.finish(
            op,
            false,
            &serde_json::json!({"discarded": true, "gate": gate_name}),
        )?;

        let attempts = {
            let w = r.worker.as_str().to_owned();
            self.services
                .store
                .write(move |tx| s12_lane::bump_fix_attempts(tx.conn(), &w))?
        };
        let cap: u32 = SettingsService::new(&self.services)
            .run_value(&r.run, None, "integration_fix_attempts")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3);
        self.worker_to(r, WorkerState::Active(WorkerPhase::Fixing), Vec::new())
            .await?;
        if attempts > cap {
            let id = Interventions::new(&self.services)
                .raise_for_worker(
                    &r.run,
                    &r.worker,
                    InterventionKind::IntegrationRegression,
                    InterventionDetails {
                        summary: format!(
                            "{} keeps failing integration validation at `{gate_name}`",
                            r.ticket
                        ),
                        evidence: vec![output],
                        preserved_work: format!(
                            "branch {} and its worktree are untouched",
                            r.branch
                        ),
                        last_safe_commit: Some(before.clone()),
                        actions: vec![
                            "fix it by hand and resume".to_owned(),
                            "cancel the worker".to_owned(),
                        ],
                        consequences: "this ticket waits; others continue".to_owned(),
                    },
                )
                .await?;
            return Ok(LaneOutcome::FixCapReached {
                intervention_id: id,
            });
        }
        Ok(LaneOutcome::SentBackToFix {
            gate: gate_name,
            attempts,
        })
    }

    async fn escalate_conflict(
        &self,
        r: &LaneRequest,
        paths: &[String],
    ) -> Result<String, VelaError> {
        let ev = event(
            &self.services,
            &r.run,
            Some(&r.ticket),
            Event::ConflictResolutionStarted(ConflictResolutionStartedPayload {
                conflicting_paths: paths.to_vec(),
            }),
        );
        self.services
            .store
            .write(move |tx| tx.append(&ev).map(|_| ()))?;
        self.worker_to(
            r,
            WorkerState::Active(WorkerPhase::ConflictResolution),
            Vec::new(),
        )
        .await?;
        Interventions::new(&self.services)
            .raise_for_worker(
                &r.run,
                &r.worker,
                InterventionKind::MergeConflict,
                InterventionDetails {
                    summary: format!("{} conflicts with the integration branch", r.ticket),
                    evidence: paths.to_vec(),
                    preserved_work: format!("branch {} is untouched and the integration branch is unchanged", r.branch),
                    last_safe_commit: None,
                    actions: vec![
                        "merge the integration branch into the worker branch and resolve the conflicts".to_owned(),
                        "cancel the worker".to_owned(),
                    ],
                    consequences: "this ticket waits; others continue".to_owned(),
                },
            )
            .await
    }

    /// Pushes the integration branch without force. A failure is journaled and reported as `false`; it never undoes the merge,
    /// and pushing again later is always safe.
    async fn push(&self, r: &LaneRequest, ctx: &GitContext) -> bool {
        let Some(remote) = &r.remote else {
            return false;
        };
        let ops = OperationLog::new(&self.services);
        let op = self.services.ids.new_operation_id();
        let _ = ops.begin(&op, Some(&r.run), "push", &serde_json::json!({"remote": remote.as_str(), "branch": r.integration_branch.as_str()}));
        let result = self
            .git
            .push(ctx, &r.integration, remote, &r.integration_branch)
            .await;
        let (ev, ok) = match result {
            Ok(out) => (
                Event::PushCompleted(PushCompletedPayload {
                    branch: r.integration_branch.clone(),
                    head: out.remote_head,
                }),
                true,
            ),
            Err(e) => (
                Event::PushFailed(PushFailedPayload {
                    branch: r.integration_branch.clone(),
                    error: e,
                }),
                false,
            ),
        };
        let _ = ops.finish(&op, ok, &serde_json::json!({}));
        let news = event(&self.services, &r.run, Some(&r.ticket), ev);
        let _ = self
            .services
            .store
            .write(move |tx| tx.append(&news).map(|_| ()));
        ok
    }
}

/// Reconciles an interrupted merge: the merge was recorded as intended and nothing says how it ended. The integration branch may
/// have moved without validation, so a person (or the lane's discard) decides; the lane never guesses.
pub struct MergeRecipe;

impl Recipe for MergeRecipe {
    fn kind(&self) -> &'static str {
        "merge"
    }

    fn reconcile<'a>(
        &'a self,
        operation: &'a vela_persistence::repositories::r01_operations::Operation,
    ) -> PortFuture<'a, Reconciled> {
        Box::pin(async move {
            Ok(Reconciled::NeedsHuman(format!(
                "merge {} was interrupted before its result was recorded; the integration branch is unverified",
                operation.id
            )))
        })
    }
}

/// Reconciles an interrupted push: a push never forces and is idempotent, so it is simply safe to do again.
pub struct PushRecipe;

impl Recipe for PushRecipe {
    fn kind(&self) -> &'static str {
        "push"
    }

    fn reconcile<'a>(
        &'a self,
        _operation: &'a vela_persistence::repositories::r01_operations::Operation,
    ) -> PortFuture<'a, Reconciled> {
        Box::pin(async { Ok(Reconciled::NotStarted) })
    }
}

/// Registers the lane's reconciliation recipes with the operation framework.
pub fn register_recipes(services: &Services) {
    let _ = OperationLog::new(services);
    if let Some(registry) = services.get::<crate::operations::RecipeRegistry>() {
        registry.register(Arc::new(MergeRecipe));
        registry.register(Arc::new(PushRecipe));
    }
}

#[cfg(test)]
mod tests;
