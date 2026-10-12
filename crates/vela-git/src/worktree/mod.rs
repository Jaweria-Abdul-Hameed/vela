//! The worktree manager (G02, ADR-011): isolated worktrees under one Vela-managed root, created at a recorded commit,
//! removed only when clean, with deterministic collision-safe names. Vela never touches the primary checkout.

pub mod naming;
pub mod policy;

use std::path::Path;
use std::time::Duration;

use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::ids::{BranchName, CommitSha, WorktreePath};
use vela_domain::ports::{GitContext, GitWorktrees, PortFuture, WorktreeInfo, WorktreeRequest};

use crate::read::GitCli;
use crate::read::hardening::Mode;

/// How many times a locked folder is retried when removing a worktree.
pub const REMOVE_ATTEMPTS: u32 = 5;
/// First backoff; doubles each attempt.
pub const REMOVE_BACKOFF_MS: u64 = 200;

/// Parses `git worktree list --porcelain -z` into worktrees (entries without a commit, such as a bare repository, are
/// skipped).
pub fn parse_worktree_list(text: &str) -> Vec<WorktreeInfo> {
    let mut out = Vec::new();
    let mut path: Option<String> = None;
    let mut head: Option<String> = None;
    let mut branch: Option<String> = None;
    let mut flush =
        |path: &mut Option<String>, head: &mut Option<String>, branch: &mut Option<String>| {
            if let (Some(p), Some(h)) = (path.take(), head.take())
                && let (Ok(p), Ok(h)) = (WorktreePath::new(p), CommitSha::new(h))
            {
                let b = branch.take().and_then(|b| BranchName::new(b).ok());
                out.push(WorktreeInfo {
                    path: p,
                    branch: b,
                    head: h,
                });
            }
            *branch = None;
        };
    for field in text.split('\0') {
        if let Some(p) = field.strip_prefix("worktree ") {
            flush(&mut path, &mut head, &mut branch);
            path = Some(p.to_owned());
        } else if let Some(h) = field.strip_prefix("HEAD ") {
            head = Some(h.to_owned());
        } else if let Some(b) = field.strip_prefix("branch refs/heads/") {
            branch = Some(b.to_owned());
        }
    }
    flush(&mut path, &mut head, &mut branch);
    out
}

fn transient_removal_failure(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    [
        "permission denied",
        "being used by another process",
        "directory not empty",
        "unable to delete",
        "failed to remove",
    ]
    .iter()
    .any(|m| lower.contains(m))
}

impl GitCli {
    pub(crate) fn require_managed(&self, path: &WorktreePath) -> Result<(), VelaError> {
        let Some(root) = self.managed_root() else {
            return Err(VelaError::of_class(
                ErrorClass::InternalBug,
                "no managed worktree root is configured",
            ));
        };
        policy::check_root(root)?;
        if !policy::is_under(root, Path::new(path.as_str())) {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "worktrees can only be created or removed below the Vela-managed root",
            ));
        }
        Ok(())
    }

    /// Creates a worktree at `request.base` on a new `request.branch`, then returns what to record: the path, the branch and
    /// the commit the worktree starts at.
    pub async fn create_worktree(
        &self,
        ctx: &GitContext,
        request: &WorktreeRequest,
    ) -> Result<WorktreeInfo, VelaError> {
        self.require_managed(&request.path)?;
        let existing = self.list(ctx, &request.repo).await?;
        let wanted = vela_domain::ids::case_fold_key(&request.path.as_str().replace('\\', "/"));
        if existing
            .iter()
            .any(|w| vela_domain::ids::case_fold_key(&w.path.as_str().replace('\\', "/")) == wanted)
        {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "a worktree already exists at that path (names that differ only in case collide on Windows)",
            ));
        }
        let _guard = self.locks.acquire(&request.repo, &ctx.operation).await;
        let path = policy::git_path(Path::new(request.path.as_str()));
        self.run_ok(
            ctx,
            &request.repo,
            Mode::Write,
            &[
                "worktree",
                "add",
                "-b",
                request.branch.as_str(),
                &path,
                request.base.as_str(),
            ],
        )
        .await?;
        let head = self
            .run_ok(
                ctx,
                &request.path,
                Mode::Read,
                &["rev-parse", "--verify", "HEAD"],
            )
            .await?;
        let head = CommitSha::new(head.trim())
            .map_err(|e| VelaError::of_class(ErrorClass::InternalBug, e.to_string()))?;
        if head != request.base {
            return Err(VelaError::of_class(
                ErrorClass::InternalBug,
                "the new worktree did not start at the requested commit",
            ));
        }
        Ok(WorktreeInfo {
            path: request.path.clone(),
            branch: Some(request.branch.clone()),
            head,
        })
    }

    async fn list(
        &self,
        ctx: &GitContext,
        repo: &WorktreePath,
    ) -> Result<Vec<WorktreeInfo>, VelaError> {
        let text = self
            .run_ok(
                ctx,
                repo,
                Mode::Read,
                &["worktree", "list", "--porcelain", "-z"],
            )
            .await?;
        Ok(parse_worktree_list(&text))
    }

    /// Removes a clean worktree under the managed root. A dirty worktree is refused. A folder another process holds is
    /// retried with bounded backoff and then reported, never forced.
    pub async fn remove_clean_worktree(
        &self,
        ctx: &GitContext,
        repo: &WorktreePath,
        path: &WorktreePath,
    ) -> Result<(), VelaError> {
        self.require_managed(path)?;
        let _guard = self.locks.acquire(path, &ctx.operation).await;
        let dirty = self
            .run_ok(
                ctx,
                path,
                Mode::Read,
                &["status", "--porcelain", "--untracked-files=all"],
            )
            .await?;
        if !dirty.trim().is_empty() {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "the worktree has uncommitted work; it was not removed",
            ));
        }
        let target = policy::git_path(Path::new(path.as_str()));
        let mut last = String::new();
        for attempt in 0..REMOVE_ATTEMPTS {
            let out = self
                .run_raw(ctx, repo, Mode::Write, &["worktree", "remove", &target])
                .await?;
            if out.success() {
                return Ok(());
            }
            last = out.stderr;
            // A previous attempt may have removed Git's records and the files but not the folder itself (something held
            // it). The worktree was verified clean and lives under the managed root, so only the husk is left to remove.
            if attempt > 0 && last.contains("is not a working tree") {
                match std::fs::remove_dir_all(path.as_str()) {
                    Ok(()) => {
                        let _ = self
                            .run_raw(ctx, repo, Mode::Write, &["worktree", "prune"])
                            .await;
                        return Ok(());
                    }
                    Err(e) => {
                        last = format!("failed to delete the folder: {e}");
                        tokio::time::sleep(Duration::from_millis(REMOVE_BACKOFF_MS << attempt))
                            .await;
                        continue;
                    }
                }
            }
            if !transient_removal_failure(&last) {
                return Err(crate::read::classify_failure(
                    "worktree remove",
                    &crate::read::GitOutput {
                        exit_code: out.exit_code,
                        stdout: out.stdout,
                        stderr: last,
                    },
                ));
            }
            tokio::time::sleep(Duration::from_millis(REMOVE_BACKOFF_MS << attempt)).await;
        }
        let holder = self
            .locks
            .holder_of(path)
            .map(|op| format!(" (held by Vela operation {op})"))
            .unwrap_or_default();
        Err(VelaError::of_class(
            ErrorClass::ExternalTemporary,
            format!(
                "the worktree folder is in use by another process{holder}; close editors, terminals or indexers using it and retry. Vela did not force removal. Git said: {}",
                last.lines().next().unwrap_or("").trim()
            ),
        ))
    }

    /// Drops administrative records of worktrees whose folders no longer exist.
    pub async fn prune_worktrees(
        &self,
        ctx: &GitContext,
        repo: &WorktreePath,
    ) -> Result<(), VelaError> {
        self.run_ok(ctx, repo, Mode::Write, &["worktree", "prune"])
            .await
            .map(|_| ())
    }

    /// Repairs administrative links after a worktree folder was moved.
    pub async fn repair_worktrees(
        &self,
        ctx: &GitContext,
        repo: &WorktreePath,
    ) -> Result<(), VelaError> {
        self.run_ok(ctx, repo, Mode::Write, &["worktree", "repair"])
            .await
            .map(|_| ())
    }

    /// Locks a worktree against pruning, with a reason.
    pub async fn lock_worktree(
        &self,
        ctx: &GitContext,
        repo: &WorktreePath,
        path: &WorktreePath,
        reason: &str,
    ) -> Result<(), VelaError> {
        self.require_managed(path)?;
        let target = policy::git_path(Path::new(path.as_str()));
        let reason = format!("--reason={reason}");
        self.run_ok(
            ctx,
            repo,
            Mode::Write,
            &["worktree", "lock", &reason, &target],
        )
        .await
        .map(|_| ())
    }
}

impl GitWorktrees for GitCli {
    fn add_worktree<'a>(
        &'a self,
        ctx: &'a GitContext,
        request: &'a WorktreeRequest,
    ) -> PortFuture<'a, ()> {
        Box::pin(async move { self.create_worktree(ctx, request).await.map(|_| ()) })
    }

    fn remove_worktree<'a>(
        &'a self,
        ctx: &'a GitContext,
        repo: &'a WorktreePath,
        path: &'a WorktreePath,
    ) -> PortFuture<'a, ()> {
        Box::pin(async move { self.remove_clean_worktree(ctx, repo, path).await })
    }

    fn list_worktrees<'a>(
        &'a self,
        ctx: &'a GitContext,
        repo: &'a WorktreePath,
    ) -> PortFuture<'a, Vec<WorktreeInfo>> {
        Box::pin(async move { self.list(ctx, repo).await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_worktree_lists_are_parsed_with_branches_and_detached_heads() {
        let a = "a".repeat(40);
        let b = "b".repeat(40);
        let text = format!(
            "worktree C:/repo\0HEAD {a}\0branch refs/heads/main\0\0worktree C:/wt/x\0HEAD {b}\0detached\0\0worktree C:/bare\0bare\0\0"
        );
        let list = parse_worktree_list(&text);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].branch.as_ref().map(|b| b.as_str()), Some("main"));
        assert_eq!(list[0].head.as_str(), a);
        assert!(list[1].branch.is_none());
        assert_eq!(list[1].path.as_str(), "C:/wt/x");
    }

    #[test]
    fn only_lock_like_failures_are_retried() {
        assert!(transient_removal_failure(
            "error: failed to delete 'x': Permission denied"
        ));
        assert!(transient_removal_failure(
            "The process cannot access the file because it is being used by another process"
        ));
        assert!(!transient_removal_failure(
            "fatal: 'x' is not a working tree"
        ));
    }
}
