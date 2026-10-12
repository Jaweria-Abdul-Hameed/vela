//! Merge operations (G04, ADR-011): integrating a reviewed worker head into an integration worktree with a non-fast-forward
//! merge, merging the integration tip into a worker branch, reporting conflicts without losing the recoverable state, and
//! discarding one unpublished merge. There is no force, no rebase and no history rewrite anywhere in this crate except the
//! single `reset` in [`GitCli::discard_unpublished_merge_commit`], which refuses unless it is provably safe.

use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::ids::{BranchName, CommitSha, WorktreePath};
use vela_domain::ports::{GitContext, GitMerges, MergeOutcome, PortFuture, Trailer};

use crate::read::GitCli;
use crate::read::hardening::Mode;

fn sha_of(text: &str) -> Result<CommitSha, VelaError> {
    CommitSha::new(text.trim())
        .map_err(|e| VelaError::of_class(ErrorClass::InternalBug, e.to_string()))
}

impl GitCli {
    async fn head_sha(&self, ctx: &GitContext, wt: &WorktreePath) -> Result<CommitSha, VelaError> {
        sha_of(
            &self
                .run_ok(ctx, wt, Mode::Read, &["rev-parse", "--verify", "HEAD"])
                .await?,
        )
    }

    async fn is_ancestor(
        &self,
        ctx: &GitContext,
        wt: &WorktreePath,
        ancestor: &CommitSha,
        of: &str,
    ) -> Result<bool, VelaError> {
        let out = self
            .run_raw(
                ctx,
                wt,
                Mode::Read,
                &["merge-base", "--is-ancestor", ancestor.as_str(), of],
            )
            .await?;
        match out.exit_code {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(crate::read::classify_failure("merge-base", &out)),
        }
    }

    async fn require_clean(&self, ctx: &GitContext, wt: &WorktreePath) -> Result<(), VelaError> {
        let status = self
            .run_ok(
                ctx,
                wt,
                Mode::Read,
                &["status", "--porcelain", "--untracked-files=all"],
            )
            .await?;
        if status.trim().is_empty() {
            Ok(())
        } else {
            Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "the worktree has uncommitted changes; the operation was refused",
            ))
        }
    }

    /// Merges `source` (whose tip must still be `expected_head`, the head that was reviewed) into the branch checked out in
    /// `integration`, whose head must still be `integration_before`. The merge is always a non-fast-forward commit carrying
    /// `trailers`. A conflict leaves the merge in progress (recoverable with [`GitMerges::abort_merge`]) and is reported.
    pub async fn merge_branch(
        &self,
        ctx: &GitContext,
        integration: &WorktreePath,
        integration_before: &CommitSha,
        source: &BranchName,
        expected_head: &CommitSha,
        trailers: &[Trailer],
    ) -> Result<MergeOutcome, VelaError> {
        let _guard = self.locks.acquire(integration, &ctx.operation).await;
        let head = self.head_sha(ctx, integration).await?;
        if &head != integration_before {
            // Already merged by an earlier attempt of the same operation?
            if self
                .is_ancestor(ctx, integration, expected_head, "HEAD")
                .await?
            {
                return Ok(MergeOutcome::Merged {
                    head,
                    merged_source: expected_head.clone(),
                });
            }
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "the integration branch moved since it was recorded; the merge was not attempted",
            ));
        }
        self.require_clean(ctx, integration).await?;
        let tip_ref = format!("refs/heads/{}^{{commit}}", source.as_str());
        let tip = sha_of(
            &self
                .run_ok(
                    ctx,
                    integration,
                    Mode::Read,
                    &["rev-parse", "--verify", "--quiet", &tip_ref],
                )
                .await?,
        )?;
        if &tip != expected_head {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "the branch head differs from the reviewed head; it must be reviewed again before merging",
            ));
        }

        let subject = format!(
            "Merge branch '{}' (reviewed head {})",
            source.as_str(),
            expected_head.short(12)
        );
        let mut args = vec![
            "-c".to_owned(),
            "user.name=Vela".to_owned(),
            "-c".to_owned(),
            "user.email=vela@localhost".to_owned(),
            "merge".to_owned(),
            "--no-ff".to_owned(),
            "--no-edit".to_owned(),
            "--no-gpg-sign".to_owned(),
            "-m".to_owned(),
            subject,
        ];
        if !trailers.is_empty() {
            args.push("-m".to_owned());
            args.push(
                trailers
                    .iter()
                    .map(|t| format!("{}: {}", t.key(), t.value()))
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        args.push(expected_head.as_str().to_owned());
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = self.run_raw(ctx, integration, Mode::Write, &refs).await?;

        if out.success() {
            let new_head = self.head_sha(ctx, integration).await?;
            if !self
                .is_ancestor(ctx, integration, expected_head, "HEAD")
                .await?
            {
                return Err(VelaError::internal(
                    "the merge commit does not contain the reviewed head",
                ));
            }
            return Ok(MergeOutcome::Merged {
                head: new_head,
                merged_source: expected_head.clone(),
            });
        }
        let unmerged = self
            .run_ok(
                ctx,
                integration,
                Mode::Read,
                &[
                    "diff",
                    "--name-only",
                    "--diff-filter=U",
                    "-z",
                    "--no-ext-diff",
                    "--no-textconv",
                ],
            )
            .await?;
        let paths: Vec<String> = unmerged
            .split('\0')
            .filter(|p| !p.is_empty())
            .map(str::to_owned)
            .collect();
        if paths.is_empty() {
            return Err(crate::read::classify_failure("merge", &out));
        }
        Ok(MergeOutcome::Conflicted { paths })
    }

    /// Resets a clean, Vela-managed integration worktree to `reset_to` after an unpublished merge. It refuses when the
    /// worktree is dirty, when `HEAD` is not a merge whose first parent is `reset_to`, or when any remote-tracking branch
    /// already contains `HEAD` (the merge is published). This is the only operation in the crate that moves a branch backward.
    pub async fn discard_unpublished_merge_commit(
        &self,
        ctx: &GitContext,
        integration: &WorktreePath,
        reset_to: &CommitSha,
    ) -> Result<(), VelaError> {
        self.require_managed(integration)?;
        let _guard = self.locks.acquire(integration, &ctx.operation).await;
        self.require_clean(ctx, integration).await?;
        let head = self.head_sha(ctx, integration).await?;
        if &head == reset_to {
            return Ok(());
        }
        let parent = sha_of(
            &self
                .run_ok(
                    ctx,
                    integration,
                    Mode::Read,
                    &["rev-parse", "--verify", "HEAD^1"],
                )
                .await
                .map_err(|_| {
                    VelaError::of_class(
                        ErrorClass::ValidationFailure,
                        "HEAD has no parent; there is no merge to discard",
                    )
                })?,
        )?;
        if &parent != reset_to {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "the recorded commit is not the first parent of HEAD; refusing to discard anything",
            ));
        }
        let published = self
            .run_ok(
                ctx,
                integration,
                Mode::Read,
                &[
                    "for-each-ref",
                    "--contains",
                    head.as_str(),
                    "--format=%(refname)",
                    "refs/remotes",
                ],
            )
            .await?;
        if !published.trim().is_empty() {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "the merge is already on the remote; published history is never rewritten",
            ));
        }
        self.run_ok(
            ctx,
            integration,
            Mode::Write,
            &["reset", "--hard", reset_to.as_str()],
        )
        .await
        .map(|_| ())
    }
}

impl GitMerges for GitCli {
    fn merge<'a>(
        &'a self,
        ctx: &'a GitContext,
        integration: &'a WorktreePath,
        integration_before: &'a CommitSha,
        source: &'a BranchName,
        expected_head: &'a CommitSha,
    ) -> PortFuture<'a, MergeOutcome> {
        Box::pin(async move {
            self.merge_branch(
                ctx,
                integration,
                integration_before,
                source,
                expected_head,
                &[],
            )
            .await
        })
    }

    fn abort_merge<'a>(
        &'a self,
        ctx: &'a GitContext,
        integration: &'a WorktreePath,
    ) -> PortFuture<'a, ()> {
        Box::pin(async move {
            let _guard = self.locks.acquire(integration, &ctx.operation).await;
            let in_progress = self
                .run_raw(
                    ctx,
                    integration,
                    Mode::Read,
                    &["rev-parse", "--verify", "--quiet", "MERGE_HEAD"],
                )
                .await?;
            if !in_progress.success() {
                return Ok(());
            }
            self.run_ok(ctx, integration, Mode::Write, &["merge", "--abort"])
                .await
                .map(|_| ())
        })
    }

    fn discard_unpublished_merge<'a>(
        &'a self,
        ctx: &'a GitContext,
        integration: &'a WorktreePath,
        reset_to: &'a CommitSha,
    ) -> PortFuture<'a, ()> {
        Box::pin(async move {
            self.discard_unpublished_merge_commit(ctx, integration, reset_to)
                .await
        })
    }
}
