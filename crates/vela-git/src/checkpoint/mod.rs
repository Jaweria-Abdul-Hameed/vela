//! Checkpoint commits (G03): a worker's progress is recorded as a commit only when the worktree is quiescent and there is
//! something to record. The commit carries run and ticket trailers. Hooks are never bypassed: a failing repository hook is a
//! typed validation failure and no commit exists. A locked Git index is waited out a bounded time and then reported, never
//! deleted.

use std::time::Duration;

use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::ids::{CommitSha, WorktreePath};
use vela_domain::ports::{CheckpointMessage, GitCheckpoints, GitContext, PortFuture};

use crate::read::hardening::Mode;
use crate::read::{GitCli, is_index_locked};

/// How many times a locked index is retried, and the first wait (doubling).
pub const INDEX_LOCK_ATTEMPTS: u32 = 5;
/// First wait for a locked index, in milliseconds.
pub const INDEX_LOCK_BACKOFF_MS: u64 = 100;
/// How long the worktree must show the same changes before it counts as quiescent.
pub const QUIESCENCE_MS: u64 = 150;

/// True when two status listings show the same set of changes (nothing was written in between).
pub fn is_quiescent(before: &str, after: &str) -> bool {
    before == after
}

impl GitCli {
    async fn status_listing(
        &self,
        ctx: &GitContext,
        worktree: &WorktreePath,
    ) -> Result<String, VelaError> {
        self.run_ok(
            ctx,
            worktree,
            Mode::Read,
            &[
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
                "--no-renames",
            ],
        )
        .await
    }

    async fn stage_all(&self, ctx: &GitContext, worktree: &WorktreePath) -> Result<(), VelaError> {
        let mut last = None;
        for attempt in 0..INDEX_LOCK_ATTEMPTS {
            match self
                .run_ok(ctx, worktree, Mode::Write, &["add", "-A"])
                .await
            {
                Ok(_) => return Ok(()),
                Err(e) if is_index_locked(&e) => {
                    last = Some(e);
                    tokio::time::sleep(Duration::from_millis(INDEX_LOCK_BACKOFF_MS << attempt))
                        .await;
                }
                Err(e) => return Err(e),
            }
        }
        Err(last
            .unwrap_or_else(|| VelaError::internal("the index retry loop ended without an error")))
    }

    async fn head_of(
        &self,
        ctx: &GitContext,
        worktree: &WorktreePath,
    ) -> Result<CommitSha, VelaError> {
        let text = self
            .run_ok(
                ctx,
                worktree,
                Mode::Read,
                &["rev-parse", "--verify", "HEAD"],
            )
            .await?;
        CommitSha::new(text.trim())
            .map_err(|e| VelaError::of_class(ErrorClass::InternalBug, e.to_string()))
    }

    /// Records the worktree as a checkpoint commit and returns the new head.
    ///
    /// `fixed_point` is the commit the worker started from. Nothing staged is an error, unless the worker already
    /// committed its own work (head differs from the fixed point), in which case the current head is the checkpoint.
    pub async fn checkpoint(
        &self,
        ctx: &GitContext,
        worktree: &WorktreePath,
        message: &CheckpointMessage,
        fixed_point: Option<&CommitSha>,
    ) -> Result<CommitSha, VelaError> {
        let _guard = self.locks.acquire(worktree, &ctx.operation).await;

        let first = self.status_listing(ctx, worktree).await?;
        tokio::time::sleep(Duration::from_millis(QUIESCENCE_MS)).await;
        let second = self.status_listing(ctx, worktree).await?;
        if !is_quiescent(&first, &second) {
            return Err(VelaError::of_class(
                ErrorClass::ExternalTemporary,
                "the worktree is still changing; a checkpoint waits until the worker is quiet",
            ));
        }

        self.stage_all(ctx, worktree).await?;
        let before = self.head_of(ctx, worktree).await?;
        let staged = self
            .run_raw(
                ctx,
                worktree,
                Mode::Read,
                &[
                    "diff",
                    "--cached",
                    "--quiet",
                    "--no-ext-diff",
                    "--no-textconv",
                ],
            )
            .await?;
        match staged.exit_code {
            Some(0) => {
                // Nothing new to commit.
                if let Some(fixed) = fixed_point
                    && &before != fixed
                {
                    return Ok(before);
                }
                return Err(VelaError::of_class(
                    ErrorClass::ValidationFailure,
                    "there is nothing to checkpoint: the worktree has no changes since the fixed point",
                ));
            }
            Some(1) => {}
            _ => return Err(crate::read::classify_failure("diff", &staged)),
        }

        let identity = self
            .run_raw(
                ctx,
                worktree,
                Mode::Read,
                &["config", "--get", "user.email"],
            )
            .await?;
        let mut args: Vec<String> = Vec::new();
        if !identity.success() || identity.stdout.trim().is_empty() {
            args.extend([
                "-c".to_owned(),
                "user.name=Vela".to_owned(),
                "-c".to_owned(),
                "user.email=vela@localhost".to_owned(),
            ]);
        }
        args.push("commit".to_owned());
        args.push("--no-gpg-sign".to_owned());
        args.push(format!("--message={}", message.subject()));
        for trailer in message.trailers() {
            args.push(format!("--trailer={}: {}", trailer.key(), trailer.value()));
        }
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();

        vela_domain::fault_point!(vela_domain::fault::FaultPoint::GitCheckpointBeforeCommit);
        let out = self.run_raw(ctx, worktree, Mode::Write, &refs).await?;
        if !out.success() {
            let head_now = self.head_of(ctx, worktree).await?;
            if is_index_locked(&crate::read::classify_failure("commit", &out)) {
                return Err(crate::read::classify_failure("commit", &out));
            }
            let detail: String = format!("{}{}", out.stdout, out.stderr)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .take(3)
                .collect::<Vec<_>>()
                .join(" / ");
            debug_assert_eq!(head_now, before, "a failed commit must not move the head");
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                format!(
                    "the commit was rejected, most likely by a repository hook (hooks are never bypassed); no commit was made: {detail}"
                ),
            ));
        }
        vela_domain::fault_point!(vela_domain::fault::FaultPoint::GitCheckpointAfterCommit);
        self.head_of(ctx, worktree).await
    }
}

impl GitCheckpoints for GitCli {
    fn commit_checkpoint<'a>(
        &'a self,
        ctx: &'a GitContext,
        worktree: &'a WorktreePath,
        message: &'a CheckpointMessage,
    ) -> PortFuture<'a, CommitSha> {
        Box::pin(async move { self.checkpoint(ctx, worktree, message, None).await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiescence_means_two_identical_listings() {
        assert!(is_quiescent(" M a.rs\0", " M a.rs\0"));
        assert!(!is_quiescent(" M a.rs\0", " M a.rs\0?? b.rs\0"));
        assert!(is_quiescent("", ""));
    }
}
