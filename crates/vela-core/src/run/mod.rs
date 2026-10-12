//! Build start (S08): the user approves the graph and presses Build; Vela records the base commit, creates the integration
//! branch and worktree, and the run enters `RUNNING`.
//!
//! Order matters and is crash-safe: everything that can be refused (trust, state, a moved base) is checked before the first
//! change; the branch and worktree are created next; the base and the worktree are recorded in one transaction with the
//! `WorktreeProvisioned` event; only then does the run enter `RUNNING`. The three fault points mark the gaps a crash can fall in.

use std::sync::Arc;

use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::events::{Event, WorktreeProvisionedPayload};
use vela_domain::fault::{self, FaultPoint};
use vela_domain::ids::{BranchName, CommitSha, RunId, WorktreePath};
use vela_domain::ports::{GitContext, GitRead, GitWorktrees, RevisionSpec, WorktreeRequest};
use vela_domain::state::{Cause, IntegrationHealth, RunPhase, RunState};
use vela_persistence::journal::NewEvent;
use vela_persistence::repositories::s06_run_graphs;
use vela_persistence::repositories::s08_build::{self, RunBase};

use crate::orchestrator::Services;
use crate::orchestrator::runs::Runs;
use crate::trust::TrustService;

/// The Git operations the build start needs.
pub trait BuildGit: GitRead + GitWorktrees {}

impl<T: GitRead + GitWorktrees> BuildGit for T {}

/// What the user chose when pressing Build.
#[derive(Debug, Clone)]
pub struct StartOptions {
    /// The run's project.
    pub project_id: String,
    /// The primary repository (never modified).
    pub repo: WorktreePath,
    /// The branch to build on.
    pub base_branch: BranchName,
    /// The base commit the user was shown when they approved the graph, if any. When the branch has moved since, the user must
    /// acknowledge the divergence.
    pub expected_base: Option<CommitSha>,
    /// The user acknowledged that the base moved.
    pub acknowledge_divergence: bool,
    /// Where the integration worktree goes (a managed location).
    pub integration_worktree: WorktreePath,
}

/// Build start over the shared services.
pub struct BuildService {
    services: Arc<Services>,
    git: Arc<dyn BuildGit>,
}

impl BuildService {
    /// A service creating branches and worktrees through `git`.
    pub fn new(services: Arc<Services>, git: Arc<dyn BuildGit>) -> Self {
        Self { services, git }
    }

    fn ctx(&self, trust: vela_domain::ports::RepoTrust) -> GitContext {
        GitContext {
            trust,
            operation: self.services.ids.new_operation_id(),
        }
    }

    /// The user approves the graph: `GRAPH_READY` to `AWAITING_START`.
    pub async fn approve_graph(&self, run: &RunId) -> Result<(), VelaError> {
        let runs = Runs::new(&self.services);
        if runs.run_state(run)? != RunState::Active(RunPhase::GraphReady) {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "there is no graph waiting for approval",
            ));
        }
        let (r, now) = (run.as_str().to_owned(), self.services.now_ms());
        if !self
            .services
            .store
            .write(move |tx| s06_run_graphs::approve(tx.conn(), &r, now))?
        {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "the run has no stored graph to approve",
            ));
        }
        runs.transition_run(
            run,
            RunState::Active(RunPhase::AwaitingStart),
            Cause::UserDecision,
            IntegrationHealth::Healthy,
            Vec::new(),
        )
        .await?;
        Ok(())
    }

    /// Starts the build: records the base, creates the integration branch and worktree, enters `RUNNING`.
    pub async fn start(&self, run: &RunId, options: &StartOptions) -> Result<RunBase, VelaError> {
        let runs = Runs::new(&self.services);
        let trust = TrustService::new(&self.services);
        trust.require_trusted(&options.project_id, "Build")?;
        if runs.run_state(run)? != RunState::Active(RunPhase::AwaitingStart) {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "the graph has not been approved, or the build already started",
            ));
        }
        let git_trust = trust.git_trust(&options.project_id)?;
        let revision = RevisionSpec::new(options.base_branch.as_str())
            .map_err(|e| VelaError::of_class(ErrorClass::ValidationFailure, e.to_string()))?;
        let base = self
            .git
            .resolve_ref(&self.ctx(git_trust), &options.repo, &revision)
            .await?;
        if let Some(expected) = &options.expected_base
            && *expected != base
            && !options.acknowledge_divergence
        {
            return Err(VelaError::of_class(
                ErrorClass::UserActionRequired,
                format!(
                    "{} moved after you reviewed the graph (it was {expected}, it is now {base})",
                    options.base_branch
                ),
            )
            .with_state_preserved("nothing was created; the run is still waiting to start")
            .with_user_action(
                "acknowledge the new base to build on it, or review the graph again",
            ));
        }

        runs.transition_run(
            run,
            RunState::Active(RunPhase::Starting),
            Cause::UserDecision,
            IntegrationHealth::Healthy,
            Vec::new(),
        )
        .await?;

        let integration_branch = BranchName::new(format!("vela/{}/integration", run.as_str()))
            .map_err(|e| VelaError::of_class(ErrorClass::ValidationFailure, e.to_string()))?;
        fault::hit(FaultPoint::BuildBeforeBranch);
        self.git
            .add_worktree(
                &self.ctx(git_trust),
                &WorktreeRequest {
                    repo: options.repo.clone(),
                    path: options.integration_worktree.clone(),
                    branch: integration_branch.clone(),
                    base: base.clone(),
                },
            )
            .await?;
        fault::hit(FaultPoint::BuildAfterWorktree);

        let record = RunBase {
            base_sha: base.as_str().to_owned(),
            base_branch: options.base_branch.as_str().to_owned(),
            integration_branch: integration_branch.as_str().to_owned(),
            integration_worktree: options.integration_worktree.as_str().to_owned(),
        };
        let provisioned = NewEvent {
            ts: self.services.now_ms(),
            run_id: Some(run.clone()),
            ticket_id: None,
            worker_id: None,
            event: Event::WorktreeProvisioned(WorktreeProvisionedPayload {
                worktree: options.integration_worktree.clone(),
                branch: integration_branch.clone(),
                base_sha: base.clone(),
            }),
        };
        let (run_text, now, rec) = (
            run.as_str().to_owned(),
            self.services.now_ms(),
            record.clone(),
        );
        self.services.store.write(move |tx| {
            s08_build::set_run_base(tx.conn(), &run_text, &rec)?;
            s08_build::insert_worktree(
                tx.conn(),
                &rec.integration_worktree,
                &run_text,
                "INTEGRATION",
                &rec.integration_branch,
                &rec.base_sha,
                now,
            )?;
            tx.append(&provisioned).map(|_| ())
        })?;
        fault::hit(FaultPoint::BuildBeforeRunning);
        runs.transition_run(
            run,
            RunState::Active(RunPhase::Running),
            Cause::System,
            IntegrationHealth::Healthy,
            Vec::new(),
        )
        .await?;
        Ok(record)
    }

    /// The recorded base of a started run.
    pub fn base_of(&self, run: &RunId) -> Result<Option<RunBase>, VelaError> {
        let r = run.as_str().to_owned();
        self.services.store.read(|c| s08_build::get_run_base(c, &r))
    }
}

#[cfg(test)]
mod tests;
