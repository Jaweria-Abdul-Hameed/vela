//! `GitAdapter`, composed from capability traits (`ADAPTERS.md` "GitAdapter", component C07).
//!
//! The capabilities are separate traits so that the tickets that implement each one edit separate impl blocks and files
//! (`SHARED_SURFACE_PROTOCOL.md` section 1). [`GitAdapter`] is the original port name: a supertrait of all capabilities
//! with a blanket implementation, so any type that implements every capability is a `GitAdapter`, and consumers that
//! need one capability can ask for just that trait.
//!
//! By design there is no force-push, no rebase of pushed branches and no history rewrite on any capability (ADR-011);
//! the single permitted discard is [`GitMerges::discard_unpublished_merge`].

use crate::ids::{BranchName, CommitSha, WorktreePath};

use super::PortFuture;

/// Repository status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoStatus {
    /// The checked-out commit, when there is one.
    pub head: Option<CommitSha>,
    /// The checked-out branch; `None` when detached.
    pub branch: Option<BranchName>,
    /// Paths with uncommitted changes (including untracked files).
    pub changed_paths: Vec<String>,
}

impl RepoStatus {
    /// True when the working tree has no uncommitted changes.
    pub fn is_clean(&self) -> bool {
        self.changed_paths.is_empty()
    }
}

/// A summary of the difference between two commits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffSummary {
    /// Paths that differ.
    pub changed_paths: Vec<String>,
}

impl DiffSummary {
    /// True when the two commits differ (a review requires a non-empty diff).
    pub fn is_non_empty(&self) -> bool {
        !self.changed_paths.is_empty()
    }
}

/// Request to create a worktree on a new branch from a base commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeRequest {
    /// The primary checkout (never modified).
    pub repo: WorktreePath,
    /// Where the new worktree goes.
    pub path: WorktreePath,
    /// The new branch.
    pub branch: BranchName,
    /// The commit to start from.
    pub base: CommitSha,
}

/// A worktree known to the repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeInfo {
    /// Its path.
    pub path: WorktreePath,
    /// Its branch; `None` when detached.
    pub branch: Option<BranchName>,
    /// Its head.
    pub head: CommitSha,
}

/// A checkpoint commit message: a subject plus run and ticket trailers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointMessage {
    /// Commit subject.
    pub subject: String,
    /// `Key: value` trailers (run, ticket, worker).
    pub trailers: Vec<(String, String)>,
}

/// Result of a merge attempt in the integration worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    /// The merge completed cleanly; the new integration head.
    Merged {
        /// The integration head after the merge.
        head: CommitSha,
    },
    /// The merge stopped on textual conflicts.
    Conflicted {
        /// The conflicting paths.
        paths: Vec<String>,
    },
}

/// Read-only repository inspection.
pub trait GitRead: Send + Sync {
    /// Status of the checkout at `repo`.
    fn status<'a>(&'a self, repo: &'a WorktreePath) -> PortFuture<'a, RepoStatus>;
    /// Resolves a ref or revision expression to a full commit id.
    fn resolve_ref<'a>(
        &'a self,
        repo: &'a WorktreePath,
        reference: &'a str,
    ) -> PortFuture<'a, CommitSha>;
    /// The merge base of two commits.
    fn merge_base<'a>(
        &'a self,
        repo: &'a WorktreePath,
        a: &'a CommitSha,
        b: &'a CommitSha,
    ) -> PortFuture<'a, CommitSha>;
    /// Summary of `from...to` (three-dot), the diff a review sees.
    fn diff_summary<'a>(
        &'a self,
        repo: &'a WorktreePath,
        from: &'a CommitSha,
        to: &'a CommitSha,
    ) -> PortFuture<'a, DiffSummary>;
}

/// Worktree management.
pub trait GitWorktrees: Send + Sync {
    /// Creates a worktree on a new branch.
    fn add_worktree<'a>(&'a self, request: &'a WorktreeRequest) -> PortFuture<'a, ()>;
    /// Removes a Vela-managed worktree.
    fn remove_worktree<'a>(
        &'a self,
        repo: &'a WorktreePath,
        path: &'a WorktreePath,
    ) -> PortFuture<'a, ()>;
    /// Lists the worktrees of `repo`.
    fn list_worktrees<'a>(&'a self, repo: &'a WorktreePath) -> PortFuture<'a, Vec<WorktreeInfo>>;
}

/// Checkpoint commits.
pub trait GitCheckpoints: Send + Sync {
    /// Commits all changes in `worktree` as a checkpoint; returns the new commit. Only called while the worker session
    /// is quiescent and under the per-worktree Git operation lock (ADR-015).
    fn commit_checkpoint<'a>(
        &'a self,
        worktree: &'a WorktreePath,
        message: &'a CheckpointMessage,
    ) -> PortFuture<'a, CommitSha>;
}

/// Merge-lane operations in the integration worktree.
pub trait GitMerges: Send + Sync {
    /// Merges `source` into the integration worktree with a non-fast-forward merge commit.
    fn merge<'a>(
        &'a self,
        integration: &'a WorktreePath,
        source: &'a BranchName,
    ) -> PortFuture<'a, MergeOutcome>;
    /// Discards an unpublished merge by resetting a clean, Vela-managed integration worktree to a recorded commit.
    fn discard_unpublished_merge<'a>(
        &'a self,
        integration: &'a WorktreePath,
        reset_to: &'a CommitSha,
    ) -> PortFuture<'a, ()>;
}

/// Remote operations. Deliberately has no force option (ADR-011).
pub trait GitRemote: Send + Sync {
    /// Fetches from `remote`.
    fn fetch<'a>(&'a self, repo: &'a WorktreePath, remote: &'a str) -> PortFuture<'a, ()>;
    /// Pushes `branch` to `remote` without force.
    fn push<'a>(
        &'a self,
        repo: &'a WorktreePath,
        remote: &'a str,
        branch: &'a BranchName,
    ) -> PortFuture<'a, ()>;
}

/// The original `GitAdapter` port: every capability.
pub trait GitAdapter: GitRead + GitWorktrees + GitCheckpoints + GitMerges + GitRemote {}

impl<T> GitAdapter for T where
    T: ?Sized + GitRead + GitWorktrees + GitCheckpoints + GitMerges + GitRemote
{
}
