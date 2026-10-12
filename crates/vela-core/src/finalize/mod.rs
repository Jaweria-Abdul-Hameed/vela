//! Finalization (S14): when every ticket is done Vela validates the integration branch one last time, pushes it, promotes it
//! according to the project's mode, cleans up the worktrees it made, and completes the run with a summary.
//!
//! The final review is part of the optional review mode (ADR-024): with review off, `FINAL_REVIEW` is passed through to
//! `FINAL_VALIDATION`, which is never skipped. A failed final gate or a failed push is a human decision, never a success.

use std::sync::Arc;

use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::events::{
    Event, PromotionCompletedPayload, PushCompletedPayload, PushFailedPayload, RunCompletedPayload,
};
use vela_domain::fault::{self, FaultPoint};
use vela_domain::ids::{BranchName, CommitSha, RunId, WorktreePath};
use vela_domain::ports::{
    GitContext, GitRead, GitRemote, GitWorktrees, ProcessRunner, RemoteName, RevisionSpec,
};
use vela_domain::state::{Cause, InterventionKind, RunPhase, RunState, WorkerState};
use vela_persistence::repositories::{o01_workers, s08_build, s12_lane, t04_promotions::Promotion};

use crate::intervention::{InterventionDetails, Interventions};
use crate::merge_lane::{event, run_gates};
use crate::operations::OperationLog;
use crate::orchestrator::Services;
use crate::orchestrator::runs::Runs;
use crate::promotion::{PromotionInput, PromotionService};
use crate::trust::{ProfileCommand, TrustService};

/// The Git operations finalization needs.
pub trait FinalGit: GitRead + GitRemote + GitWorktrees {}

impl<T: GitRead + GitRemote + GitWorktrees> FinalGit for T {}

/// What finalization needs to know.
#[derive(Debug, Clone)]
pub struct FinalInput {
    /// The run.
    pub run: RunId,
    /// The project.
    pub project_id: String,
    /// The primary repository.
    pub repo: WorktreePath,
    /// The integration worktree.
    pub integration: WorktreePath,
    /// The integration branch.
    pub integration_branch: BranchName,
    /// The branch the run started from.
    pub base_branch: BranchName,
    /// The base commit the run started from.
    pub base_head: CommitSha,
    /// The final validation gates (each must be in the confirmed profile).
    pub validation: Vec<ProfileCommand>,
    /// The remote to push to, when there is one.
    pub remote: Option<RemoteName>,
    /// Pull request title.
    pub title: String,
    /// Pull request body.
    pub body: String,
    /// Tickets in the run.
    pub tickets_total: u32,
}

/// What the run produced.
#[derive(Debug, Clone, PartialEq)]
pub struct RunSummary {
    /// Tickets that finished.
    pub tickets_done: u32,
    /// Tickets in the run.
    pub tickets_total: u32,
    /// How the result was handed over.
    pub promotion: Promotion,
    /// Worktrees removed during cleanup.
    pub worktrees_removed: Vec<String>,
}

/// How finalization ended.
#[derive(Debug, Clone, PartialEq)]
pub enum FinalOutcome {
    /// The run completed.
    Completed(RunSummary),
    /// A human must decide; the integration branch and worktree are untouched.
    NeedsHuman {
        /// The intervention.
        intervention_id: String,
    },
}

/// The finalizer.
pub struct Finalizer {
    services: Arc<Services>,
    git: Arc<dyn FinalGit>,
    runner: Arc<dyn ProcessRunner>,
    promotion: Arc<PromotionService>,
}

impl Finalizer {
    /// A finalizer over its ports.
    pub fn new(
        services: Arc<Services>,
        git: Arc<dyn FinalGit>,
        runner: Arc<dyn ProcessRunner>,
        promotion: Arc<PromotionService>,
    ) -> Self {
        Self {
            services,
            git,
            runner,
            promotion,
        }
    }

    async fn escalate(
        &self,
        run: &RunId,
        summary: &str,
        evidence: Vec<String>,
        last_safe: Option<CommitSha>,
    ) -> Result<FinalOutcome, VelaError> {
        let id = Interventions::new(&self.services)
            .raise_for_run(
                run,
                InterventionKind::Other,
                InterventionDetails {
                    summary: summary.to_owned(),
                    evidence,
                    preserved_work: "every merged ticket is on the integration branch, untouched"
                        .to_owned(),
                    last_safe_commit: last_safe,
                    actions: vec![
                        "fix the problem, then resume".to_owned(),
                        "cancel the run".to_owned(),
                    ],
                    consequences: "the run waits; nothing is published until it passes".to_owned(),
                },
            )
            .await?;
        Ok(FinalOutcome::NeedsHuman {
            intervention_id: id,
        })
    }

    /// Finishes a run whose tickets are all done.
    pub async fn finish(&self, input: &FinalInput) -> Result<FinalOutcome, VelaError> {
        let runs = Runs::new(&self.services);
        if runs.run_state(&input.run)? != RunState::Active(RunPhase::Running) {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "the run is not running",
            ));
        }
        let health = s12_health(&self.services, &input.run)?;
        let workers = {
            let r = input.run.as_str().to_owned();
            self.services
                .store
                .read(|c| o01_workers::list_for_run(c, &r))?
        };
        let done = workers
            .iter()
            .filter(|w| {
                serde_json::from_str::<WorkerState>(&w.state)
                    .is_ok_and(|s| s == WorkerState::Complete)
            })
            .count();
        let done = u32::try_from(done).unwrap_or(u32::MAX);
        if done != input.tickets_total {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                format!(
                    "{done} of {} tickets are done; the run cannot finish yet",
                    input.tickets_total
                ),
            ));
        }

        // Everything that can fail happens while the run is still RUNNING, the only phase a human gate can hang off. Integration
        // must be healthy before the run may leave RUNNING at all.
        if !health.permits_final_review() {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "integration is not healthy, so the run cannot finish",
            ));
        }

        let ctx = GitContext {
            trust: TrustService::new(&self.services).git_trust(&input.project_id)?,
            operation: self.services.ids.new_operation_id(),
        };
        let head = self
            .git
            .resolve_ref(
                &ctx,
                &input.integration,
                &RevisionSpec::new("HEAD").map_err(|e| VelaError::internal(e.to_string()))?,
            )
            .await?;

        let gates = run_gates(
            &self.services,
            self.runner.as_ref(),
            &input.project_id,
            &input.run,
            None,
            input.integration.as_str(),
            &input.validation,
        )
        .await?;
        if let Some((failed, output)) = gates.failure {
            return self
                .escalate(
                    &input.run,
                    &format!("final validation failed at `{}`", failed.gate()),
                    vec![output],
                    Some(head),
                )
                .await;
        }

        // Push the integration branch (never forced); the promotion needs it on the remote.
        let mut pushed = false;
        if let Some(remote) = &input.remote {
            fault::hit(FaultPoint::FinalBeforePush);
            let ops = OperationLog::new(&self.services);
            let op = self.services.ids.new_operation_id();
            ops.begin(&op, Some(&input.run), "push", &serde_json::json!({"remote": remote.as_str(), "branch": input.integration_branch.as_str()}))?;
            match self
                .git
                .push(&ctx, &input.integration, remote, &input.integration_branch)
                .await
            {
                Ok(out) => {
                    ops.finish(
                        &op,
                        true,
                        &serde_json::json!({"head": out.remote_head.as_str()}),
                    )?;
                    let ev = event(
                        &self.services,
                        &input.run,
                        None,
                        Event::PushCompleted(PushCompletedPayload {
                            branch: input.integration_branch.clone(),
                            head: out.remote_head,
                        }),
                    );
                    self.services
                        .store
                        .write(move |tx| tx.append(&ev).map(|_| ()))?;
                    pushed = true;
                }
                Err(e) => {
                    ops.finish(&op, false, &serde_json::json!({"error": e.what_failed()}))?;
                    let reason = e.what_failed().to_owned();
                    let ev = event(
                        &self.services,
                        &input.run,
                        None,
                        Event::PushFailed(PushFailedPayload {
                            branch: input.integration_branch.clone(),
                            error: e,
                        }),
                    );
                    self.services
                        .store
                        .write(move |tx| tx.append(&ev).map(|_| ()))?;
                    return self
                        .escalate(
                            &input.run,
                            "pushing the integration branch failed",
                            vec![reason],
                            Some(head),
                        )
                        .await;
                }
            }
            fault::hit(FaultPoint::FinalAfterPush);
        }

        // Promotion per the project's mode.
        let promotion_input = PromotionInput {
            run: input.run.clone(),
            project_id: input.project_id.clone(),
            repo: input.repo.clone(),
            branch: input.integration_branch.clone(),
            branch_head: head.clone(),
            base: input.base_branch.clone(),
            base_head: input.base_head.clone(),
            has_remote: pushed,
            title: input.title.clone(),
            body: input.body.clone(),
        };
        self.promotion.begin(&promotion_input).await?;
        let promotion = self.promotion.finalize(&input.run).await?;
        fault::hit(FaultPoint::FinalAfterPromotion);

        // Cleanup: worker worktrees always; the integration worktree only once its branch is safely elsewhere.
        fault::hit(FaultPoint::FinalDuringCleanup);
        let keep_integration = matches!(promotion.state.as_str(), "LOCAL_READY" | "NONE");
        let removed = self.cleanup(input, &ctx, keep_integration).await?;

        let mut events = vec![Event::RunCompleted(RunCompletedPayload {
            outcome: "COMPLETED".to_owned(),
            tickets_done: done,
            tickets_total: input.tickets_total,
        })];
        if matches!(promotion.state.as_str(), "READY" | "MERGED" | "LOCAL_READY") {
            events.insert(
                0,
                Event::PromotionCompleted(PromotionCompletedPayload {
                    from_branch: input.integration_branch.clone(),
                    to_branch: input.base_branch.clone(),
                    head: head.clone(),
                }),
            );
        }
        // With review off FINAL_REVIEW is passed through; the final validation was done above, while the run could still be
        // handed to a human.
        for phase in [RunPhase::FinalReview, RunPhase::FinalValidation] {
            runs.transition_run(
                &input.run,
                RunState::Active(phase),
                Cause::System,
                health,
                Vec::new(),
            )
            .await?;
        }
        runs.transition_run(
            &input.run,
            RunState::Completed,
            Cause::System,
            health,
            events,
        )
        .await?;
        Ok(FinalOutcome::Completed(RunSummary {
            tickets_done: done,
            tickets_total: input.tickets_total,
            promotion,
            worktrees_removed: removed,
        }))
    }

    async fn cleanup(
        &self,
        input: &FinalInput,
        ctx: &GitContext,
        keep_integration: bool,
    ) -> Result<Vec<String>, VelaError> {
        let listed = {
            let r = input.run.as_str().to_owned();
            self.services
                .store
                .read(|c| s08_build::list_worktrees(c, &r))?
        };
        let mut removed = Vec::new();
        for (path, kind, _branch) in listed {
            if kind == "INTEGRATION" && keep_integration {
                continue;
            }
            let wt = WorktreePath::new(path.clone()).map_err(|e| {
                VelaError::internal(format!("a recorded worktree path is invalid: {e}"))
            })?;
            // A worktree that is already gone is not an error (cleanup is repeatable).
            if self
                .git
                .remove_worktree(ctx, &input.repo, &wt)
                .await
                .is_ok()
            {
                let (p, now) = (path.clone(), self.services.now_ms());
                self.services
                    .store
                    .write(move |tx| s08_build::mark_worktree_removed(tx.conn(), &p, now))?;
                removed.push(path);
            }
        }
        Ok(removed)
    }
}

fn s12_health(
    services: &Services,
    run: &RunId,
) -> Result<vela_domain::state::IntegrationHealth, VelaError> {
    let r = run.as_str().to_owned();
    services.store.read(|c| s12_lane::get_health(c, &r))
}

#[cfg(test)]
mod tests;
