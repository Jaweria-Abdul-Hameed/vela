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

        // IMPLEMENTING: a fresh conversation, then validation with bounded fix turns.
        self.advance(t, WorkerPhase::Implementing).await?;
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
            .await?;
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
                AgentEvent::CapacityError { message } => {
                    return Err(VelaError::of_class(
                        ErrorClass::ExternalTemporary,
                        format!("the provider has no capacity: {message}"),
                    ));
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
        let kind = match e.code() {
            Some(ErrorCode::ProviderPolicyBlock) => InterventionKind::ProviderPolicyBlock,
            Some(ErrorCode::AuthRequired) => InterventionKind::AuthRequired,
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
    async fn checkpoint(&self, t: &WorkerTask) -> Result<CommitSha, VelaError> {
        let ctx = self.ctx(&t.project_id)?;
        let status = self.git.status(&ctx, &t.worktree).await?;
        if status.is_clean() {
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
