//! Promotion (T04): how a finished run is handed to the user. A pull request by default, local-ready when there is no remote,
//! and never a silent merge of the default branch: merging happens only in the opt-in auto-merge mode, and only after the user
//! acknowledged the warning (a consent recorded with its text version).

use std::sync::Arc;

use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::ids::{BranchName, CommitSha, RunId, WorktreePath};
use vela_domain::ports::{GitContext, GitRead, PullRequestPort, PullRequestRequest, RepoTrust};
use vela_persistence::repositories::t04_promotions::{self, Promotion};

use crate::orchestrator::Services;
use crate::settings::SettingsService;
use crate::settings::onboarding::Onboarding;

/// How the run is handed over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// A pull request (the default).
    PullRequest,
    /// Nothing leaves the machine; the integration branch is ready locally.
    LocalOnly,
    /// Merge the pull request when the run ends (opt-in, warned).
    AutoMerge,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Mode::PullRequest => "PULL_REQUEST",
            Mode::LocalOnly => "LOCAL_ONLY",
            Mode::AutoMerge => "AUTO_MERGE",
        }
    }

    fn from_setting(value: &str) -> Mode {
        match value {
            "local_only" => Mode::LocalOnly,
            "auto_merge" => Mode::AutoMerge,
            _ => Mode::PullRequest,
        }
    }
}

/// What a run is promoting.
#[derive(Debug, Clone)]
pub struct PromotionInput {
    /// The run.
    pub run: RunId,
    /// Its project.
    pub project_id: String,
    /// The primary repository.
    pub repo: WorktreePath,
    /// The integration branch.
    pub branch: BranchName,
    /// The integration branch head.
    pub branch_head: CommitSha,
    /// The branch it would merge into.
    pub base: BranchName,
    /// The base head the run started from.
    pub base_head: CommitSha,
    /// A remote is configured and reachable.
    pub has_remote: bool,
    /// Pull request title.
    pub title: String,
    /// Pull request body.
    pub body: String,
}

/// The promotion service.
pub struct PromotionService {
    services: Arc<Services>,
    git: Arc<dyn GitRead>,
    prs: Option<Arc<dyn PullRequestPort>>,
}

impl PromotionService {
    /// A service; `prs` is `None` when GitHub is unavailable (the run then stays local-ready).
    pub fn new(
        services: Arc<Services>,
        git: Arc<dyn GitRead>,
        prs: Option<Arc<dyn PullRequestPort>>,
    ) -> Self {
        Self { services, git, prs }
    }

    fn save(&self, p: &Promotion) -> Result<(), VelaError> {
        let p = p.clone();
        self.services
            .store
            .write(move |tx| t04_promotions::upsert(tx.conn(), &p))
    }

    /// The record of a run.
    pub fn get(&self, run: &RunId) -> Result<Option<Promotion>, VelaError> {
        let r = run.as_str().to_owned();
        self.services.store.read(|c| t04_promotions::get(c, &r))
    }

    fn ctx(&self) -> GitContext {
        GitContext {
            trust: RepoTrust::Trusted,
            operation: self.services.ids.new_operation_id(),
        }
    }

    /// Starts promotion: a draft pull request when the integration branch has content and a remote exists, otherwise
    /// local-ready. Calling it again returns the existing record.
    pub async fn begin(&self, input: &PromotionInput) -> Result<Promotion, VelaError> {
        if let Some(existing) = self.get(&input.run)? {
            return Ok(existing);
        }
        let setting = SettingsService::new(&self.services)
            .effective(Some(&input.project_id))?
            .remove("promotion_mode")
            .unwrap_or_default();
        let mode = Mode::from_setting(&setting);
        if mode == Mode::AutoMerge && !Onboarding::new(&self.services).is_granted("auto_merge")? {
            return Err(VelaError::of_class(
                ErrorClass::PolicyBlock,
                "auto-merge is on but the warning was never acknowledged",
            )
            .with_state_preserved("nothing was promoted")
            .with_user_action(
                "acknowledge the auto-merge warning in Settings, or switch to pull requests",
            ));
        }
        let diff = self
            .git
            .diff_summary(
                &self.ctx(),
                &input.repo,
                &input.base_head,
                &input.branch_head,
            )
            .await?;
        let mut record = Promotion {
            run_id: input.run.as_str().to_owned(),
            mode: mode.name().to_owned(),
            state: "NONE".to_owned(),
            branch: input.branch.as_str().to_owned(),
            base_branch: input.base.as_str().to_owned(),
            pr_url: None,
            note: String::new(),
            updated_ms: self.services.now_ms(),
        };
        if !diff.is_non_empty() {
            record.note = "the run produced no changes, so there is nothing to promote".to_owned();
        } else if mode == Mode::LocalOnly || !input.has_remote || self.prs.is_none() {
            record.state = "LOCAL_READY".to_owned();
            record.note = format!(
                "the result is on branch {} and was not pushed",
                input.branch
            );
        } else if let Some(prs) = &self.prs {
            let url = prs
                .create_draft(&PullRequestRequest {
                    branch: input.branch.clone(),
                    base: input.base.clone(),
                    title: input.title.clone(),
                    body: input.body.clone(),
                })
                .await?;
            record.state = "DRAFT".to_owned();
            record.pr_url = Some(url);
            record.note = "a draft pull request is open".to_owned();
        }
        self.save(&record)?;
        Ok(record)
    }

    /// After final validation passed: marks the draft ready (and merges it in auto-merge mode). Safe to call again.
    pub async fn finalize(&self, run: &RunId) -> Result<Promotion, VelaError> {
        let mut record = self.get(run)?.ok_or_else(|| {
            VelaError::of_class(
                ErrorClass::ValidationFailure,
                "the run has no promotion record",
            )
        })?;
        if record.state != "DRAFT" {
            return Ok(record);
        }
        let (Some(prs), Some(url)) = (&self.prs, record.pr_url.clone()) else {
            return Ok(record);
        };
        prs.mark_ready(&url).await?;
        record.state = "READY".to_owned();
        record.note = "the pull request is ready for your review".to_owned();
        record.updated_ms = self.services.now_ms();
        self.save(&record)?;
        if record.mode == Mode::AutoMerge.name() {
            if !Onboarding::new(&self.services).is_granted("auto_merge")? {
                // Consent was withdrawn while the run was going: stop at ready.
                return Ok(record);
            }
            prs.merge(&url).await?;
            record.state = "MERGED".to_owned();
            record.note = "merged because auto-merge is on".to_owned();
            record.updated_ms = self.services.now_ms();
            self.save(&record)?;
        }
        Ok(record)
    }
}

#[cfg(test)]
mod tests;
