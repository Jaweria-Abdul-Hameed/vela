//! The worker lifecycle (S10): one ticket goes through provisioning, an implementation turn in a fresh conversation, focused
//! validation and a checkpoint commit, ending ready to merge.
//!
//! Every state change goes through the D01 machine as one transaction with its journal events. Nothing the agent says is believed
//! by itself: validation commands run through the process port under the policy engine, and the checkpoint is Vela's own commit.
//! A run with review off (ADR-024) passes through `REVIEWING` without a review, because the machine has no shorter edge.

use std::num::NonZeroU64;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use vela_domain::errors::{ErrorClass, ErrorCode, VelaError};
use vela_domain::events::{
    CheckpointCommittedPayload, CommandFinishedPayload, CommandStartedPayload, Event,
    TestGateFinishedPayload,
};
use vela_domain::fault::{self, FaultPoint};
use vela_domain::ids::{
    BranchName, CommitSha, GateResult, GateStatus, RunId, TicketId, WorkerId, WorktreePath,
};
use vela_domain::policy::{Decision, Dialect, Operation, PolicyEngine, ProfileEntry, RunMode};
use vela_domain::ports::{
    AgentEvent, AgentEventSink, AgentSession, CheckpointMessage, CommandSpec, ConversationId,
    GitCheckpoints, GitContext, GitRead, GitWorktrees, OutputChunk, OutputSink, ProcessRunner,
    Trailer, TurnRequest, WorktreeRequest,
};
use vela_domain::state::{IntegrationHealth, InterventionKind, WorkerPhase, WorkerState};
use vela_persistence::journal::NewEvent;
use vela_persistence::repositories::{o01_workers, s08_build, s10_test_runs};

use crate::intervention::{InterventionDetails, Interventions};
use crate::operations::{OPERATION_TRAILER, OperationLog};
use crate::orchestrator::Services;
use crate::orchestrator::runs::Runs;
use crate::provisioning::ProvisioningService;
use crate::scheduler::{DEFAULT_PROFILE, ProfileAvailabilityService};
use crate::trust::{ProfileCommand, TrustService};

/// The Git operations a worker needs.
pub trait WorkerGit: GitRead + GitWorktrees + GitCheckpoints {}

impl<T: GitRead + GitWorktrees + GitCheckpoints> WorkerGit for T {}

/// How the agent said the ticket ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionStatus {
    /// Finished.
    Complete,
    /// Cannot proceed without a decision.
    Blocked,
    /// Gave up.
    Failed,
}

/// What Vela keeps from the agent's completion block (claims, not facts).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    /// How it ended.
    pub status: CompletionStatus,
    /// Warnings it raised.
    pub warnings: Vec<String>,
    /// Dependencies it says it found.
    pub discovered_dependencies: Vec<String>,
}

/// Prompt text and completion parsing, provider-specific in production (the Antigravity prompts).
pub trait WorkerPrompts: Send + Sync {
    /// The first prompt of a fresh conversation.
    fn worker_prompt(&self, task: &WorkerTask) -> String;
    /// The prompt that reports failing validation and asks for a fix.
    fn fix_prompt(&self, failures: &str) -> String;
    /// The prompt that asks for a corrected completion block.
    fn correction_prompt(&self, problem: &str) -> String;
    /// The first prompt of a recovery-resume session: verified facts only. The default is the ordinary prompt.
    fn recovery_prompt(
        &self,
        task: &WorkerTask,
        _last_checkpoint: Option<&CommitSha>,
        _completed_steps: &[String],
    ) -> String {
        self.worker_prompt(task)
    }
    /// Reads the completion from the agent's final text.
    fn parse_completion(&self, response: &str, base: &str) -> Result<Completion, VelaError>;
}

/// One ticket's work order.
#[derive(Debug, Clone)]
pub struct WorkerTask {
    /// The run.
    pub run: RunId,
    /// The worker.
    pub worker: WorkerId,
    /// The ticket.
    pub ticket: TicketId,
    /// Title (untrusted).
    pub title: String,
    /// Body (untrusted).
    pub body: String,
    /// The project.
    pub project_id: String,
    /// The primary repository.
    pub repo: WorktreePath,
    /// The primary repository on disk (the source of allowlisted provisioning copies).
    pub repo_root: PathBuf,
    /// The worker's worktree.
    pub worktree: WorktreePath,
    /// The worker's branch.
    pub branch: BranchName,
    /// The commit the branch starts from.
    pub base: CommitSha,
    /// Focused validation commands; each must be in the confirmed command profile.
    pub validation: Vec<ProfileCommand>,
    /// How many times the agent may be asked to fix failing validation.
    pub max_fix_attempts: u32,
    /// Spec pointers for the prompt.
    pub spec_pointers: Vec<String>,
}

/// How a worker's run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerOutcome {
    /// Checkpointed and ready to merge.
    ReadyToMerge {
        /// The checkpoint commit (the head the merge lane will merge).
        head: CommitSha,
    },
    /// A human must decide; the intervention preserves the work.
    NeedsHuman {
        /// The intervention.
        intervention_id: String,
    },
    /// The provider ran out of capacity: this worker is paused where it was and resumes when the profile's cooldown ends.
    Paused {
        /// When the profile is available again, epoch milliseconds.
        until_ms: i64,
    },
}

#[derive(Default)]
struct Observed {
    events: Mutex<Vec<AgentEvent>>,
}

impl AgentEventSink for Observed {
    fn on_event(&self, event: AgentEvent) {
        self.events.lock().expect("events lock").push(event);
    }
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

/// Drives one worker through its states.
pub struct WorkerDriver {
    services: Arc<Services>,
    agent: Arc<dyn AgentSession>,
    runner: Arc<dyn ProcessRunner>,
    git: Arc<dyn WorkerGit>,
    prompts: Arc<dyn WorkerPrompts>,
    /// What a failed turn reported before it failed, by worker, so a capacity error the provider raised as an error is still
    /// recognised as one.
    failed_turn_events: Mutex<std::collections::HashMap<String, Vec<AgentEvent>>>,
}

fn failure(msg: impl Into<String>) -> VelaError {
    VelaError::of_class(ErrorClass::AgentFailure, msg.into())
}

impl WorkerDriver {
    /// A driver over its ports.
    pub fn new(
        services: Arc<Services>,
        agent: Arc<dyn AgentSession>,
        runner: Arc<dyn ProcessRunner>,
        git: Arc<dyn WorkerGit>,
        prompts: Arc<dyn WorkerPrompts>,
    ) -> Self {
        Self {
            services,
            agent,
            runner,
            git,
            prompts,
            failed_turn_events: Mutex::new(std::collections::HashMap::new()),
        }
    }

    fn ctx(&self, project_id: &str) -> Result<GitContext, VelaError> {
        Ok(GitContext {
            trust: TrustService::new(&self.services).git_trust(project_id)?,
            operation: self.services.ids.new_operation_id(),
        })
    }

    async fn advance(&self, t: &WorkerTask, to: WorkerPhase) -> Result<(), VelaError> {
        Runs::new(&self.services)
            .advance_worker(&t.run, &t.worker, to, IntegrationHealth::Healthy)
            .await
            .map(|_| ())
    }

    fn append(&self, t: &WorkerTask, event: Event) -> Result<(), VelaError> {
        let ev = NewEvent {
            ts: self.services.now_ms(),
            run_id: Some(t.run.clone()),
            ticket_id: Some(t.ticket.clone()),
            worker_id: Some(t.worker.clone()),
            event,
        };
        self.services
            .store
            .write(move |tx| tx.append(&ev).map(|_| ()))
    }

    /// Runs the worker from `PROVISIONING` to `READY_TO_MERGE`. An `Err` leaves the worker for the caller's supervisor to fail
    /// (see [`crate::orchestrator::supervisor::supervise`]).
    pub async fn run(&self, t: &WorkerTask) -> Result<WorkerOutcome, VelaError> {
        // PROVISIONING: the worktree, then the confirmed provisioning commands.
        self.git
            .add_worktree(
                &self.ctx(&t.project_id)?,
                &WorktreeRequest {
                    repo: t.repo.clone(),
                    path: t.worktree.clone(),
                    branch: t.branch.clone(),
                    base: t.base.clone(),
                },
            )
            .await?;
        {
            let (path, run, branch, base, now) = (
                t.worktree.as_str().to_owned(),
                t.run.as_str().to_owned(),
                t.branch.as_str().to_owned(),
                t.base.as_str().to_owned(),
                self.services.now_ms(),
            );
            self.services.store.write(move |tx| {
                s08_build::insert_worktree(tx.conn(), &path, &run, "WORKER", &branch, &base, now)
            })?;
        }
        ProvisioningService::new(self.services.clone(), self.runner.clone())
            .provision(
                &t.project_id,
                &t.repo_root,
                std::path::Path::new(t.worktree.as_str()),
            )
            .await?;
        fault::hit(FaultPoint::WorkerAfterProvisioning);

        // CONTEXT_LOADING: the prompt is built from pointers; nothing is copied into the worktree.
        self.advance(t, WorkerPhase::ContextLoading).await?;
        let prompt = self.prompts.worker_prompt(t);

        self.advance(t, WorkerPhase::Implementing).await?;
        self.implement_and_finish(t, prompt).await
    }

    /// Recovery-resume (R02): a new session at the last checkpoint, never the old conversation as authority. It refuses to start
    /// on a profile that is not available (for example a provider policy block) and continues from the worker's recorded phase.
    pub async fn resume(
        &self,
        t: &WorkerTask,
        last_checkpoint: Option<&CommitSha>,
        completed_steps: &[String],
    ) -> Result<WorkerOutcome, VelaError> {
        if let Some(p) = self.services.get::<ProfileAvailabilityService>()
            && !p.0.is_available(DEFAULT_PROFILE)
        {
            return Err(VelaError::of_class(
                ErrorClass::ExternalTemporary,
                "the execution profile is not available, so no session is started",
            ));
        }
        let state = Runs::new(&self.services).worker_status(&t.worker)?.state();
        let WorkerState::Active(phase) = state else {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "only a worker in an active phase can be resumed",
            ));
        };
        let need_checkpoint = || {
            last_checkpoint.cloned().ok_or_else(|| {
                VelaError::of_class(
                    ErrorClass::ValidationFailure,
                    "the worker has no verified checkpoint to continue from",
                )
            })
        };
        match phase {
            WorkerPhase::Provisioning | WorkerPhase::ContextLoading | WorkerPhase::Implementing => {
                let ctx = self.ctx(&t.project_id)?;
                let listed = self.git.list_worktrees(&ctx, &t.repo).await?;
                if !listed.iter().any(|w| w.path == t.worktree) {
                    self.git
                        .add_worktree(
                            &ctx,
                            &WorktreeRequest {
                                repo: t.repo.clone(),
                                path: t.worktree.clone(),
                                branch: t.branch.clone(),
                                base: last_checkpoint.cloned().unwrap_or_else(|| t.base.clone()),
                            },
                        )
                        .await?;
                }
                if phase == WorkerPhase::Provisioning {
                    self.advance(t, WorkerPhase::ContextLoading).await?;
                }
                if phase != WorkerPhase::Implementing {
                    self.advance(t, WorkerPhase::Implementing).await?;
                }
                let prompt = self
                    .prompts
                    .recovery_prompt(t, last_checkpoint, completed_steps);
                self.implement_and_finish(t, prompt).await
            }
            WorkerPhase::FocusedValidation => {
                self.advance(t, WorkerPhase::Checkpointing).await?;
                self.checkpoint_and_ready(t).await
            }
            WorkerPhase::Checkpointing => self.checkpoint_and_ready(t).await,
            WorkerPhase::Reviewing => {
                self.advance(t, WorkerPhase::ReadyToMerge).await?;
                Ok(WorkerOutcome::ReadyToMerge {
                    head: need_checkpoint()?,
                })
            }
            WorkerPhase::ReadyToMerge => Ok(WorkerOutcome::ReadyToMerge {
                head: need_checkpoint()?,
            }),
            other => Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                format!(
                    "a worker in {} belongs to the merge lane, not to a new session",
                    other.name()
                ),
            )),
        }
    }

    /// After a failed integration validation the worker is in `FIXING`: the same conversation is asked to fix it, the focused gates
    /// run again, and the worker is checkpointed and returned to `READY_TO_MERGE` with a new head for the lane.
    pub async fn fix_integration(
        &self,
        t: &WorkerTask,
        gate: &str,
    ) -> Result<WorkerOutcome, VelaError> {
        let conversation = {
            let w = t.worker.as_str().to_owned();
            self.services
                .store
                .read(|c| o01_workers::get(c, &w))?
                .and_then(|r| r.conversation_id)
                .and_then(|id| ConversationId::new(id).ok())
        };
        let ask = format!(
            "After your branch was merged into the integration branch, the check `{gate}` failed. Find the cause in your worktree, fix it, and report again."
        );
        let mut turn = match self
            .turn(t, self.prompts.fix_prompt(&ask), conversation)
            .await
        {
            Ok(turn) => turn,
            Err(e) => return self.blocked_or_fail(t, e).await,
        };
        if let Some(outcome) = self.handle_blocks(t, &turn.observed).await? {
            return Ok(outcome);
        }
        let completion = self.completion(t, &mut turn).await?;
        if completion.status != CompletionStatus::Complete {
            return Err(failure("the worker could not fix the integration failure"));
        }
        self.advance(t, WorkerPhase::Validating).await?;
        let failures = self.validate(t).await?;
        if !failures.is_empty() {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                format!("the fix does not pass the focused checks:\n{failures}"),
            ));
        }
        self.advance(t, WorkerPhase::Checkpointing).await?;
        self.checkpoint_and_ready(t).await
    }

    async fn implement_and_finish(
        &self,
        t: &WorkerTask,
        prompt: String,
    ) -> Result<WorkerOutcome, VelaError> {
        // IMPLEMENTING: a fresh conversation, then validation with bounded fix turns.
        let mut turn = match self.turn(t, prompt, None).await {
            Ok(turn) => turn,
            Err(e) => return self.blocked_or_fail(t, e).await,
        };
        if let Some(outcome) = self.handle_blocks(t, &turn.observed).await? {
            return Ok(outcome);
        }
        let mut completion = self.completion(t, &mut turn).await?;
        match completion.status {
            CompletionStatus::Complete => {}
            CompletionStatus::Blocked => {
                let id = self
                    .raise(
                        t,
                        InterventionKind::Ambiguity,
                        "the worker cannot continue without a decision",
                        &completion.warnings,
                    )
                    .await?;
                return Ok(WorkerOutcome::NeedsHuman {
                    intervention_id: id,
                });
            }
            CompletionStatus::Failed => {
                return Err(failure(
                    "the worker reported that it could not do the ticket",
                ));
            }
        }
        fault::hit(FaultPoint::WorkerAfterImplementing);

        let mut attempts = 0;
        loop {
            let failures = self.validate(t).await?;
            if failures.is_empty() {
                break;
            }
            if attempts >= t.max_fix_attempts {
                return Err(VelaError::of_class(
                    ErrorClass::ValidationFailure,
                    format!("validation still fails after {attempts} fix attempts:\n{failures}"),
                ));
            }
            attempts += 1;
            let conversation = turn.conversation.clone();
            turn = match self
                .turn(t, self.prompts.fix_prompt(&failures), conversation)
                .await
            {
                Ok(turn) => turn,
                Err(e) => return self.blocked_or_fail(t, e).await,
            };
            if let Some(outcome) = self.handle_blocks(t, &turn.observed).await? {
                return Ok(outcome);
            }
            completion = self.completion(t, &mut turn).await?;
            if completion.status != CompletionStatus::Complete {
                return Err(failure(
                    "the worker stopped while fixing failing validation",
                ));
            }
        }

        // FOCUSED_VALIDATION passed: CHECKPOINTING.
        self.advance(t, WorkerPhase::FocusedValidation).await?;
        self.advance(t, WorkerPhase::Checkpointing).await?;
        self.checkpoint_and_ready(t).await
    }

    /// From `CHECKPOINTING`: Vela's own checkpoint commit, then through `REVIEWING` to `READY_TO_MERGE`.
    async fn checkpoint_and_ready(&self, t: &WorkerTask) -> Result<WorkerOutcome, VelaError> {
        fault::hit(FaultPoint::WorkerBeforeCheckpoint);
        let head = self.checkpoint(t).await?;
        fault::hit(FaultPoint::WorkerAfterCheckpoint);

        // Review is off (ADR-024): REVIEWING is passed through without a review.
        self.advance(t, WorkerPhase::Reviewing).await?;
        self.advance(t, WorkerPhase::ReadyToMerge).await?;
        Ok(WorkerOutcome::ReadyToMerge { head })
    }

    async fn turn(
        &self,
        t: &WorkerTask,
        prompt: String,
        conversation: Option<ConversationId>,
    ) -> Result<TurnState, VelaError> {
        let observed = Observed::default();
        let result = self
            .agent
            .run_turn(
                &TurnRequest {
                    worker: t.worker.clone(),
                    worktree: t.worktree.clone(),
                    prompt,
                    conversation,
                    timeout_ms: NonZeroU64::new(30 * 60 * 1000).expect("non-zero"),
                },
                &observed,
            )
            .await;
        let result = match result {
            Ok(r) => r,
            Err(e) => {
                let events = observed.events.lock().expect("events lock").clone();
                self.failed_turn_events
                    .lock()
                    .expect("failed events lock")
                    .insert(t.worker.as_str().to_owned(), events);
                return Err(e);
            }
        };
        if let Some(c) = &result.conversation {
            let (w, c, now) = (
                t.worker.as_str().to_owned(),
                c.as_str().to_owned(),
                self.services.now_ms(),
            );
            self.services
                .store
                .write(move |tx| o01_workers::set_conversation(tx.conn(), &w, &c, now))?;
        }
        if !result.finished {
            return Err(VelaError::of_class(
                ErrorClass::ExternalTemporary,
                "the agent turn did not finish within its timeout",
            ));
        }
        Ok(TurnState {
            conversation: result.conversation,
            response: result.response,
            observed,
        })
    }

    /// Provider-level problems the turn reported: authentication, policy block, exhausted capacity, an approval nobody answered.
    async fn handle_blocks(
        &self,
        t: &WorkerTask,
        observed: &Observed,
    ) -> Result<Option<WorkerOutcome>, VelaError> {
        let events = observed.events.lock().expect("events lock").clone();
        for e in events {
            let (kind, why) = match e {
                AgentEvent::AuthRequired { message } => (InterventionKind::AuthRequired, message),
                AgentEvent::ProviderPolicyBlock { evidence } => {
                    (InterventionKind::ProviderPolicyBlock, evidence)
                }
                AgentEvent::CapacityError { reset_after_ms, .. } => {
                    // Only this worker pauses; the profile cools down with the provider's reset time when it gave one.
                    let until_ms = crate::capacity::CapacityService::new(self.services.clone())
                        .on_capacity(
                            &t.run,
                            DEFAULT_PROFILE,
                            reset_after_ms,
                            std::slice::from_ref(&t.worker),
                        )
                        .await?;
                    return Ok(Some(WorkerOutcome::Paused { until_ms }));
                }
                AgentEvent::ApprovalBlocked { description } => {
                    (InterventionKind::ApprovalAsk, description)
                }
                _ => continue,
            };
            let id = self.raise(t, kind, &why, &[]).await?;
            return Ok(Some(WorkerOutcome::NeedsHuman {
                intervention_id: id,
            }));
        }
        Ok(None)
    }

    /// Provider authentication and policy blocks are human decisions that preserve the work; everything else fails the worker.
    async fn blocked_or_fail(
        &self,
        t: &WorkerTask,
        e: VelaError,
    ) -> Result<WorkerOutcome, VelaError> {
        let failed_events = self
            .failed_turn_events
            .lock()
            .expect("failed events lock")
            .remove(t.worker.as_str())
            .unwrap_or_default();
        if let Some(reset_after_ms) = failed_events.iter().find_map(|ev| match ev {
            AgentEvent::CapacityError { reset_after_ms, .. } => Some(*reset_after_ms),
            _ => None,
        }) {
            let until_ms = crate::capacity::CapacityService::new(self.services.clone())
                .on_capacity(
                    &t.run,
                    DEFAULT_PROFILE,
                    reset_after_ms,
                    std::slice::from_ref(&t.worker),
                )
                .await?;
            return Ok(WorkerOutcome::Paused { until_ms });
        }
        let kind = match e.code() {
            Some(ErrorCode::ProviderPolicyBlock) => InterventionKind::ProviderPolicyBlock,
            Some(ErrorCode::AuthRequired) => InterventionKind::AuthRequired,
            // An access failure nobody could classify: a person looks at it, with its evidence. Never retried.
            None if e.class() == ErrorClass::ExternalAuth => InterventionKind::Other,
            _ => return Err(e),
        };
        let id = self.raise(t, kind, e.what_failed(), &[]).await?;
        Ok(WorkerOutcome::NeedsHuman {
            intervention_id: id,
        })
    }

    async fn raise(
        &self,
        t: &WorkerTask,
        kind: InterventionKind,
        summary: &str,
        evidence: &[String],
    ) -> Result<String, VelaError> {
        Interventions::new(&self.services)
            .raise_for_worker(
                &t.run,
                &t.worker,
                kind,
                InterventionDetails {
                    summary: summary.to_owned(),
                    evidence: evidence.to_vec(),
                    preserved_work: format!(
                        "the worktree {} and branch {} are untouched",
                        t.worktree, t.branch
                    ),
                    last_safe_commit: Some(t.base.clone()),
                    actions: vec![
                        "resolve the problem, then resume".to_owned(),
                        "cancel the worker".to_owned(),
                    ],
                    consequences: "the worker waits; other workers continue".to_owned(),
                },
            )
            .await
    }

    /// Parses the completion; one correction turn when it is invalid, then failure.
    async fn completion(
        &self,
        t: &WorkerTask,
        turn: &mut TurnState,
    ) -> Result<Completion, VelaError> {
        match self
            .prompts
            .parse_completion(&turn.response, t.base.as_str())
        {
            Ok(c) => Ok(c),
            Err(first) => {
                let conversation = turn.conversation.clone();
                let corrected = self
                    .turn(
                        t,
                        self.prompts.correction_prompt(first.what_failed()),
                        conversation,
                    )
                    .await?;
                let parsed = self
                    .prompts
                    .parse_completion(&corrected.response, t.base.as_str());
                *turn = corrected;
                parsed.map_err(|e| {
                    failure(format!(
                        "the worker's completion stayed invalid after one correction: {}",
                        e.what_failed()
                    ))
                })
            }
        }
    }

    /// Runs every validation command; returns the failure report (empty when all passed).
    async fn validate(&self, t: &WorkerTask) -> Result<String, VelaError> {
        let root = t.worktree.as_str();
        let context = TrustService::new(&self.services).policy_context(
            &t.project_id,
            root,
            RunMode::Supervised,
        )?;
        let engine = PolicyEngine::new(context);
        let mut report = String::new();
        for c in &t.validation {
            let entry = ProfileEntry::parse(&c.text, Dialect::Cmd, c.cwd.as_deref(), root)
                .map_err(|e| {
                    VelaError::of_class(
                        ErrorClass::ValidationFailure,
                        format!("`{}` is not a valid command here: {e}", c.text),
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
                        "the policy does not allow the validation command `{}`: {}",
                        c.text, decision.reason
                    ),
                ));
            }
            let command_id = format!("val-{}", self.services.ids.new_token());
            self.append(
                t,
                Event::CommandStarted(CommandStartedPayload {
                    command_id: command_id.clone(),
                    command: entry.canonical_text(),
                    cwd: Some(t.worktree.clone()),
                }),
            )?;
            let tail = Tail::default();
            let outcome = self
                .runner
                .run(
                    &CommandSpec {
                        id: command_id.clone(),
                        program: entry.executable().to_owned(),
                        args: entry.args().to_vec(),
                        cwd: WorktreePath::new(entry.cwd()).map_err(|e| {
                            VelaError::internal(format!("the command directory is invalid: {e}"))
                        })?,
                        env: Vec::new(),
                        timeout_ms: NonZeroU64::new(15 * 60 * 1000).expect("non-zero"),
                    },
                    &tail,
                )
                .await?;
            let passed = !outcome.timed_out && outcome.exit_code == Some(0);
            let output = tail.0.lock().expect("tail lock").join("\n");
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
                    "passed".to_owned()
                } else if outcome.timed_out {
                    "timed out".to_owned()
                } else {
                    "failed".to_owned()
                },
            )
            .map_err(|e| VelaError::internal(format!("the gate result is inconsistent: {e}")))?;
            let row = s10_test_runs::TestRun {
                id: format!("tr-{}", self.services.ids.new_token()),
                run_id: t.run.as_str().to_owned(),
                worker_id: t.worker.as_str().to_owned(),
                ticket_id: t.ticket.as_str().to_owned(),
                command: entry.canonical_text(),
                exit_code: outcome.exit_code,
                passed,
                timed_out: outcome.timed_out,
                duration_ms: i64::try_from(outcome.duration_ms).unwrap_or(i64::MAX),
                output_tail: output.clone(),
                created_ms: self.services.now_ms(),
            };
            let events = [
                Event::CommandFinished(CommandFinishedPayload {
                    command_id,
                    exit_code: outcome.exit_code,
                    duration_ms: outcome.duration_ms,
                    timed_out: outcome.timed_out,
                }),
                Event::TestGateFinished(TestGateFinishedPayload { result: gate }),
            ];
            let news: Vec<NewEvent> = events
                .into_iter()
                .map(|event| NewEvent {
                    ts: self.services.now_ms(),
                    run_id: Some(t.run.clone()),
                    ticket_id: Some(t.ticket.clone()),
                    worker_id: Some(t.worker.clone()),
                    event,
                })
                .collect();
            self.services.store.write(move |tx| {
                s10_test_runs::insert(tx.conn(), &row)?;
                for n in &news {
                    tx.append(n)?;
                }
                Ok(())
            })?;
            if !passed {
                report.push_str(&format!(
                    "`{}` {}:\n{output}\n",
                    entry.canonical_text(),
                    if outcome.timed_out {
                        "timed out"
                    } else {
                        "failed"
                    }
                ));
            }
        }
        Ok(report)
    }

    /// Vela's own checkpoint commit, recorded as an operation so a crash between the commit and its record reconciles by trailer.
    /// The commit of the worker's newest recorded checkpoint, if there is one.
    fn last_recorded_checkpoint(&self, t: &WorkerTask) -> Result<Option<CommitSha>, VelaError> {
        let w = t.worker.as_str().to_owned();
        let payload: Option<String> = self.services.store.read(|c| {
            let mut stmt = c
                .prepare("SELECT payload FROM event_journal WHERE worker_id = ?1 AND kind = 'CheckpointCommitted' ORDER BY seq DESC LIMIT 1")
                .map_err(|e| VelaError::internal(e.to_string()))?;
            let mut rows = stmt
                .query([&w])
                .map_err(|e| VelaError::internal(e.to_string()))?;
            Ok(rows
                .next()
                .map_err(|e| VelaError::internal(e.to_string()))?
                .and_then(|r| r.get::<_, String>(0).ok()))
        })?;
        Ok(payload
            .and_then(|p| serde_json::from_str::<serde_json::Value>(&p).ok())
            .and_then(|v| v.get("commit").and_then(|c| c.as_str()).map(str::to_owned))
            .and_then(|c| CommitSha::new(c).ok()))
    }

    async fn checkpoint(&self, t: &WorkerTask) -> Result<CommitSha, VelaError> {
        let ctx = self.ctx(&t.project_id)?;
        let status = self.git.status(&ctx, &t.worktree).await?;
        if status.is_clean() {
            // A checkpoint that was made and journaled before a crash is simply continued from: the worktree is clean because its
            // work is already committed, and the branch is at the recorded commit.
            if let Some(recorded) = self.last_recorded_checkpoint(t)?
                && status.head.as_ref() == Some(&recorded)
                && recorded != t.base
            {
                return Ok(recorded);
            }
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "the worker finished but changed nothing in the worktree",
            ));
        }
        let ops = OperationLog::new(&self.services);
        let op = ctx.operation.clone();
        ops.begin(
            &op,
            Some(&t.run),
            "checkpoint",
            &serde_json::json!({"worktree": t.worktree.as_str(), "ticket": t.ticket.as_str()}),
        )?;
        let bad = |e: vela_domain::ids::IdError| {
            VelaError::internal(format!("the checkpoint message is invalid: {e}"))
        };
        let message = CheckpointMessage::new(
            format!("checkpoint: {}", t.ticket),
            vec![
                Trailer::new("Vela-Run", t.run.as_str()).map_err(bad)?,
                Trailer::new("Vela-Ticket", t.ticket.as_str()).map_err(bad)?,
                Trailer::new(OPERATION_TRAILER, op.as_str()).map_err(bad)?,
            ],
        )
        .map_err(bad)?;
        let commit = self
            .git
            .commit_checkpoint(&ctx, &t.worktree, &message)
            .await?;
        ops.finish(&op, true, &serde_json::json!({"commit": commit.as_str()}))?;
        self.append(
            t,
            Event::CheckpointCommitted(CheckpointCommittedPayload {
                commit: commit.clone(),
                fixed_point: t.base.clone(),
            }),
        )?;
        Ok(commit)
    }
}

struct TurnState {
    conversation: Option<ConversationId>,
    response: String,
    observed: Observed,
}

/// One line of a ticket's timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineEntry {
    /// Journal position.
    pub seq: i64,
    /// Time, epoch milliseconds.
    pub ts: i64,
    /// The event kind.
    pub kind: String,
}

/// The persisted timeline of a ticket in a run (`timeline.get`).
pub fn timeline(
    services: &Services,
    run: &RunId,
    ticket: &TicketId,
) -> Result<Vec<TimelineEntry>, VelaError> {
    let (r, t) = (run.as_str().to_owned(), ticket.as_str().to_owned());
    services.store.read(|c| {
        let mut stmt = c
            .prepare("SELECT seq, ts, kind FROM event_journal WHERE run_id = ?1 AND ticket_id = ?2 ORDER BY seq")
            .map_err(|e| VelaError::internal(e.to_string()))?;
        let rows = stmt
            .query_map([&r, &t], |row| {
                Ok(TimelineEntry {
                    seq: row.get(0)?,
                    ts: row.get(1)?,
                    kind: row.get(2)?,
                })
            })
            .map_err(|e| VelaError::internal(e.to_string()))?;
        Ok(rows.filter_map(Result::ok).collect())
    })
}

/// Everything known about one worker (`ticket.inspect`).
#[derive(Debug, Clone, PartialEq)]
pub struct Inspection {
    /// The worker's state.
    pub state: WorkerState,
    /// The branch.
    pub branch: Option<String>,
    /// The worktree.
    pub worktree: Option<String>,
    /// The conversation, once started.
    pub conversation: Option<String>,
    /// Validation results.
    pub tests: Vec<s10_test_runs::TestRun>,
    /// The timeline.
    pub timeline: Vec<TimelineEntry>,
}

/// Inspects the worker of a ticket.
pub fn inspect(
    services: &Services,
    run: &RunId,
    worker: &WorkerId,
    ticket: &TicketId,
) -> Result<Inspection, VelaError> {
    let w = worker.as_str().to_owned();
    let (row, tests) = services.store.read(|c| {
        Ok((
            o01_workers::get(c, &w)?,
            s10_test_runs::list_for_worker(c, &w)?,
        ))
    })?;
    let row = row.ok_or_else(|| {
        VelaError::of_class(ErrorClass::ValidationFailure, "there is no such worker")
    })?;
    let state: WorkerState = serde_json::from_str(&row.state)
        .map_err(|e| VelaError::internal(format!("the stored worker state is unreadable: {e}")))?;
    Ok(Inspection {
        state,
        branch: row.branch,
        worktree: row.worktree_path,
        conversation: row.conversation_id,
        tests,
        timeline: timeline(services, run, ticket)?,
    })
}

#[cfg(test)]
mod tests;
