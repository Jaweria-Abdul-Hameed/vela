//! The run driver: the loop that turns an approved graph into merged work.
//!
//! One pass is: a scheduling cycle decides which tickets may start; each started ticket gets a supervised worker task that runs the
//! worker lifecycle and then walks its branch through the merge lane (sending it back to fix when integration validation fails);
//! when every ticket is done the finalizer validates, pushes, promotes and completes the run. The driver never starts work on a
//! run that is not running, never overlaps merges, and returns (rather than spins) when nothing can progress without a human.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::task::JoinSet;
use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::ids::{BranchName, CommitSha, RunId, TicketId, WorkerId, WorktreePath};
use vela_domain::ports::{
    AgentSession, GitCheckpoints, GitMerges, GitRead, GitRemote, GitWorktrees, IssueTrackerAdapter,
    ProcessRunner, RemoteName, RevisionSpec,
};
use vela_domain::state::{Cause, IntegrationHealth, RunPhase, RunState, WorkerState};
use vela_persistence::repositories::{o01_workers, s08_build, s12_lane};

use super::Services;
use super::runs::Runs;
use super::supervisor::{AbortOnDrop, supervise};
use crate::analysis::{Analysis, AnalystHook, GraphService};
use crate::finalize::{FinalInput, FinalOutcome, Finalizer, RunSummary};
use crate::merge_lane::{LaneOutcome, LaneRequest, MergeLane};
use crate::promotion::PromotionService;
use crate::run::{BuildService, StartOptions};
use crate::scheduler::{CycleInput, CycleOutcome, Scheduler};
use crate::trust::ProfileCommand;
use crate::worker::{WorkerDriver, WorkerOutcome, WorkerPrompts, WorkerTask};

/// Every Git operation the driver's parts need, from one adapter.
pub trait DriverGit: GitRead + GitWorktrees + GitCheckpoints + GitMerges + GitRemote {}

impl<T: GitRead + GitWorktrees + GitCheckpoints + GitMerges + GitRemote> DriverGit for T {}

/// What one run needs.
#[derive(Debug, Clone)]
pub struct RunConfig {
    /// The project.
    pub project_id: String,
    /// The primary repository (as a Git path).
    pub repo: WorktreePath,
    /// The primary repository on disk.
    pub repo_root: PathBuf,
    /// The branch to build on.
    pub base_branch: BranchName,
    /// The folder under which Vela creates worktrees.
    pub worktree_root: String,
    /// The confirmed gates run after each ticket, after each merge and at the end.
    pub validation: Vec<ProfileCommand>,
    /// The remote to push to, when there is one.
    pub remote: Option<RemoteName>,
    /// Pull request title.
    pub title: String,
    /// Pull request body.
    pub body: String,
    /// Spec pointers handed to every worker.
    pub spec_pointers: Vec<String>,
    /// How many times a worker may be asked to fix failing checks.
    pub max_fix_attempts: u32,
}

/// What `drive` ended with.
#[derive(Debug, Clone, PartialEq)]
pub enum DriveOutcome {
    /// The run completed.
    Completed(RunSummary),
    /// Nothing can progress without a person: open interventions, failed workers or an unhealthy integration branch.
    Waiting(CycleOutcome),
    /// Finalization needs a human decision.
    FinalNeedsHuman {
        /// The intervention.
        intervention_id: String,
    },
}

/// The driver over its ports.
pub struct RunDriver {
    services: Arc<Services>,
    tracker: Arc<dyn IssueTrackerAdapter>,
    analyst: Option<Arc<dyn AnalystHook>>,
    git: Arc<dyn DriverGit>,
    worker: Arc<WorkerDriver>,
    lane: Arc<MergeLane>,
    build: BuildService,
    finalizer: Finalizer,
    scheduler: Scheduler,
}

impl RunDriver {
    /// Assembles the driver. `git` is one adapter that every part uses.
    #[allow(clippy::too_many_arguments)]
    pub fn new<G: DriverGit + 'static>(
        services: Arc<Services>,
        tracker: Arc<dyn IssueTrackerAdapter>,
        analyst: Option<Arc<dyn AnalystHook>>,
        git: Arc<G>,
        agent: Arc<dyn AgentSession>,
        runner: Arc<dyn ProcessRunner>,
        prompts: Arc<dyn WorkerPrompts>,
        promotion: Arc<PromotionService>,
    ) -> Arc<Self> {
        let worker = Arc::new(WorkerDriver::new(
            services.clone(),
            agent,
            runner.clone(),
            git.clone(),
            prompts,
        ));
        let lane = Arc::new(MergeLane::new(
            services.clone(),
            git.clone(),
            runner.clone(),
        ));
        Arc::new(Self {
            build: BuildService::new(services.clone(), git.clone()),
            finalizer: Finalizer::new(services.clone(), git.clone(), runner, promotion),
            scheduler: Scheduler::new(services.clone()),
            tracker,
            analyst,
            git,
            worker,
            lane,
            services,
        })
    }

    /// The merge lane (Stop All asks it whether a merge is in flight).
    pub fn lane(&self) -> Arc<MergeLane> {
        self.lane.clone()
    }

    /// Creates a run on the current head of the base branch and analyzes the issues into a graph.
    pub async fn prepare(&self, cfg: &RunConfig) -> Result<(RunId, Analysis), VelaError> {
        let ctx = self.ctx(cfg)?;
        let head = self
            .git
            .resolve_ref(
                &ctx,
                &cfg.repo,
                &RevisionSpec::new(cfg.base_branch.as_str())
                    .map_err(|e| VelaError::internal(e.to_string()))?,
            )
            .await?;
        let run = self.services.ids.new_run_id();
        let integration = BranchName::new(format!("vela/{}/integration", run.as_str()))
            .map_err(|e| VelaError::internal(e.to_string()))?;
        let runs = Runs::new(&self.services);
        runs.create_run(
            &cfg.project_id,
            run.clone(),
            head,
            integration,
            "all tickets",
        )
        .await?;
        for phase in [RunPhase::Preflight, RunPhase::Analyzing] {
            runs.transition_run(
                &run,
                RunState::Active(phase),
                Cause::System,
                IntegrationHealth::Healthy,
                Vec::new(),
            )
            .await?;
        }
        let analysis = GraphService::new(
            self.services.clone(),
            self.tracker.clone(),
            self.analyst.clone(),
        )
        .analyze(&run)
        .await?;
        Ok((run, analysis))
    }

    fn ctx(&self, cfg: &RunConfig) -> Result<vela_domain::ports::GitContext, VelaError> {
        Ok(vela_domain::ports::GitContext {
            trust: crate::trust::TrustService::new(&self.services).git_trust(&cfg.project_id)?,
            operation: self.services.ids.new_operation_id(),
        })
    }

    /// The user approved the graph and pressed Build: records the base, creates the integration branch and worktree, enters `RUNNING`.
    pub async fn approve_and_start(
        &self,
        run: &RunId,
        cfg: &RunConfig,
        expected_base: Option<CommitSha>,
        acknowledge_divergence: bool,
    ) -> Result<(), VelaError> {
        self.build.approve_graph(run).await?;
        let integration = WorktreePath::new(format!(
            "{}/{}/integration",
            cfg.worktree_root.trim_end_matches(['/', '\\']),
            run.as_str()
        ))
        .map_err(|e| VelaError::of_class(ErrorClass::ValidationFailure, e.to_string()))?;
        self.build
            .start(
                run,
                &StartOptions {
                    project_id: cfg.project_id.clone(),
                    repo: cfg.repo.clone(),
                    base_branch: cfg.base_branch.clone(),
                    expected_base,
                    acknowledge_divergence,
                    integration_worktree: integration,
                },
            )
            .await
            .map(|_| ())
    }

    /// Completes a build start the process died in the middle of (the run is `STARTING`); anything else is left alone. Returns true
    /// when a start was completed.
    pub async fn recover_start(&self, run: &RunId, cfg: &RunConfig) -> Result<bool, VelaError> {
        let integration = WorktreePath::new(format!(
            "{}/{}/integration",
            cfg.worktree_root.trim_end_matches(['/', '\\']),
            run.as_str()
        ))
        .map_err(|e| VelaError::of_class(ErrorClass::ValidationFailure, e.to_string()))?;
        Ok(self
            .build
            .resume_start(
                run,
                &StartOptions {
                    project_id: cfg.project_id.clone(),
                    repo: cfg.repo.clone(),
                    base_branch: cfg.base_branch.clone(),
                    expected_base: None,
                    acknowledge_divergence: true,
                    integration_worktree: integration,
                },
            )
            .await?
            .is_some())
    }

    fn integration_head_and_health(
        &self,
        run: &RunId,
        ctx: &vela_domain::ports::GitContext,
    ) -> Result<(WorktreePath, String, IntegrationHealth), VelaError> {
        let r = run.as_str().to_owned();
        let base = self
            .services
            .store
            .read(|c| s08_build::get_run_base(c, &r))?
            .ok_or_else(|| {
                VelaError::of_class(
                    ErrorClass::ValidationFailure,
                    "the run has not been started",
                )
            })?;
        let health = self.services.store.read(|c| s12_lane::get_health(c, &r))?;
        let _ = ctx;
        Ok((
            WorktreePath::new(base.integration_worktree)
                .map_err(|e| VelaError::internal(e.to_string()))?,
            base.integration_branch,
            health,
        ))
    }

    async fn task_for(
        &self,
        cfg: &RunConfig,
        run: &RunId,
        ticket: &TicketId,
    ) -> Result<(WorkerTask, Option<CommitSha>), VelaError> {
        let r = run.as_str().to_owned();
        let rows = self
            .services
            .store
            .read(|c| o01_workers::list_for_run(c, &r))?;
        let row = rows
            .into_iter()
            .find(|w| w.ticket_id == ticket.as_str())
            .ok_or_else(|| VelaError::internal("a scheduled ticket has no worker record"))?;
        let (title, body) = match self.tracker.read_ticket(ticket).await {
            Ok(t) => (t.title, t.body),
            Err(_) => (ticket.as_str().to_owned(), String::new()),
        };
        let worker =
            WorkerId::new(row.id.clone()).map_err(|e| VelaError::internal(e.to_string()))?;
        let checkpoint = {
            let w = row.id.clone();
            self.services.store.read(|c| {
                let mut stmt = c
                    .prepare("SELECT payload FROM event_journal WHERE worker_id = ?1 AND kind = 'CheckpointCommitted' ORDER BY seq DESC LIMIT 1")
                    .map_err(|e| VelaError::internal(e.to_string()))?;
                let mut q = stmt.query([&w]).map_err(|e| VelaError::internal(e.to_string()))?;
                Ok(match q.next().map_err(|e| VelaError::internal(e.to_string()))? {
                    Some(r) => r
                        .get::<_, String>(0)
                        .ok()
                        .and_then(|p| serde_json::from_str::<serde_json::Value>(&p).ok())
                        .and_then(|v| v.get("commit").and_then(|c| c.as_str()).map(str::to_owned))
                        .and_then(|s| CommitSha::new(s).ok()),
                    None => None,
                })
            })?
        };
        let bad = |e: vela_domain::ids::IdError| {
            VelaError::internal(format!("a stored worker field is invalid: {e}"))
        };
        Ok((
            WorkerTask {
                run: run.clone(),
                worker,
                ticket: ticket.clone(),
                title,
                body,
                project_id: cfg.project_id.clone(),
                repo: cfg.repo.clone(),
                repo_root: cfg.repo_root.clone(),
                worktree: WorktreePath::new(row.worktree_path.unwrap_or_default()).map_err(bad)?,
                branch: BranchName::new(row.branch.unwrap_or_default()).map_err(bad)?,
                base: CommitSha::new(row.base_sha.unwrap_or_default()).map_err(bad)?,
                validation: cfg.validation.clone(),
                max_fix_attempts: cfg.max_fix_attempts,
                spec_pointers: cfg.spec_pointers.clone(),
            },
            checkpoint,
        ))
    }

    /// One worker's whole job: lifecycle (or recovery-resume), then the merge lane with fix-and-retry.
    async fn work(
        self: Arc<Self>,
        cfg: RunConfig,
        task: WorkerTask,
        resume_from: Option<Option<CommitSha>>,
    ) -> Result<(), VelaError> {
        let outcome = match resume_from {
            None => self.worker.run(&task).await?,
            Some(checkpoint) => self.worker.resume(&task, checkpoint.as_ref(), &[]).await?,
        };
        let WorkerOutcome::ReadyToMerge { mut head } = outcome else {
            return Ok(());
        };
        let ctx = self.ctx(&cfg)?;
        let (integration, integration_branch, _) =
            self.integration_head_and_health(&task.run, &ctx)?;
        let integration_branch =
            BranchName::new(integration_branch).map_err(|e| VelaError::internal(e.to_string()))?;
        loop {
            let request = LaneRequest {
                run: task.run.clone(),
                project_id: cfg.project_id.clone(),
                worker: task.worker.clone(),
                ticket: task.ticket.clone(),
                branch: task.branch.clone(),
                reviewed_head: head.clone(),
                integration: integration.clone(),
                integration_branch: integration_branch.clone(),
                validation: cfg.validation.clone(),
                remote: cfg.remote.clone(),
            };
            match self.lane.merge(&request).await? {
                LaneOutcome::Merged { .. }
                | LaneOutcome::Conflict { .. }
                | LaneOutcome::FixCapReached { .. } => return Ok(()),
                LaneOutcome::SentBackToFix { gate, .. } => {
                    match self.worker.fix_integration(&task, &gate).await? {
                        WorkerOutcome::ReadyToMerge { head: next } => head = next,
                        WorkerOutcome::NeedsHuman { .. } | WorkerOutcome::Paused { .. } => {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    /// Runs the build until the run completes, or until nothing can progress without a person.
    pub async fn drive(
        self: &Arc<Self>,
        run: &RunId,
        cfg: &RunConfig,
    ) -> Result<DriveOutcome, VelaError> {
        let runs = Runs::new(&self.services);
        let mut tasks: JoinSet<String> = JoinSet::new();
        let mut running: BTreeSet<String> = BTreeSet::new();
        let ticket_total = crate::analysis::stored_graph(&self.services, run)?
            .map(|g| u32::try_from(g.snapshot.tickets.len()).unwrap_or(u32::MAX))
            .ok_or_else(|| {
                VelaError::of_class(ErrorClass::ValidationFailure, "the run has no graph")
            })?;
        loop {
            if runs.run_state(run)? != RunState::Active(RunPhase::Running) {
                return Err(VelaError::of_class(
                    ErrorClass::ValidationFailure,
                    "the run is not running",
                ));
            }
            let ctx = self.ctx(cfg)?;
            let (integration, integration_branch, health) =
                self.integration_head_and_health(run, &ctx)?;
            let head = self
                .git
                .resolve_ref(
                    &ctx,
                    &integration,
                    &RevisionSpec::new("HEAD").map_err(|e| VelaError::internal(e.to_string()))?,
                )
                .await?;
            let outcome = self
                .scheduler
                .cycle(&CycleInput {
                    run: run.clone(),
                    worktree_root: cfg.worktree_root.clone(),
                    integration_head: head,
                    health,
                })
                .await?;

            // New tickets start; workers that are active with no task (after a restart or a resolved intervention) are resumed.
            let mut to_start: Vec<(TicketId, bool)> =
                outcome.started.iter().map(|t| (t.clone(), false)).collect();
            let rows = {
                let r = run.as_str().to_owned();
                self.services
                    .store
                    .read(|c| o01_workers::list_for_run(c, &r))?
            };
            for row in rows {
                if !running.contains(&row.id)
                    && matches!(
                        serde_json::from_str::<WorkerState>(&row.state),
                        Ok(WorkerState::Active(_))
                    )
                    && let Ok(t) = TicketId::new(row.ticket_id.clone())
                    && !outcome.started.contains(&t)
                {
                    to_start.push((t, true));
                }
            }
            for (ticket, resume) in to_start {
                let (task, checkpoint) = self.task_for(cfg, run, &ticket).await?;
                running.insert(task.worker.as_str().to_owned());
                let (this, cfg2, services) = (self.clone(), cfg.clone(), self.services.clone());
                let (run2, worker2) = (run.clone(), task.worker.clone());
                let worker_id = task.worker.as_str().to_owned();
                let fut = async move {
                    this.work(cfg2, task, if resume { Some(checkpoint) } else { None })
                        .await
                };
                let mut handle = AbortOnDrop(supervise(services, run2, worker2, fut));
                tasks.spawn(async move {
                    let _ = (&mut handle.0).await;
                    worker_id
                });
            }

            if outcome.all_done && tasks.is_empty() {
                let (repo_base_head, base_branch) = {
                    let r = run.as_str().to_owned();
                    let base = self
                        .services
                        .store
                        .read(|c| s08_build::get_run_base(c, &r))?
                        .ok_or_else(|| VelaError::internal("the run has no recorded base"))?;
                    (
                        CommitSha::new(base.base_sha)
                            .map_err(|e| VelaError::internal(e.to_string()))?,
                        base.base_branch,
                    )
                };
                let result = self
                    .finalizer
                    .finish(&FinalInput {
                        run: run.clone(),
                        project_id: cfg.project_id.clone(),
                        repo: cfg.repo.clone(),
                        integration: integration.clone(),
                        integration_branch: BranchName::new(integration_branch)
                            .map_err(|e| VelaError::internal(e.to_string()))?,
                        base_branch: BranchName::new(base_branch)
                            .map_err(|e| VelaError::internal(e.to_string()))?,
                        base_head: repo_base_head,
                        validation: cfg.validation.clone(),
                        remote: cfg.remote.clone(),
                        title: cfg.title.clone(),
                        body: cfg.body.clone(),
                        tickets_total: ticket_total,
                    })
                    .await?;
                return Ok(match result {
                    FinalOutcome::Completed(summary) => DriveOutcome::Completed(summary),
                    FinalOutcome::NeedsHuman { intervention_id } => {
                        DriveOutcome::FinalNeedsHuman { intervention_id }
                    }
                });
            }
            if tasks.is_empty() {
                return Ok(DriveOutcome::Waiting(outcome));
            }
            // Wait for any worker task to end, then plan again.
            if let Some(Ok(done)) = tasks.join_next().await {
                running.remove(&done);
            }
        }
    }
}

#[cfg(test)]
mod tests;
