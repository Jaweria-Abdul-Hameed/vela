//! `GitAdapter`, composed from capability traits (`ADAPTERS.md` "GitAdapter", component C07).
//!
//! The capabilities are separate traits so that the tickets that implement each one edit separate impl blocks and files
//! (`SHARED_SURFACE_PROTOCOL.md` section 1). [`GitAdapter`] is the original port name: a supertrait of all capabilities
//! with a blanket implementation, so any type that implements every capability is a `GitAdapter`, and consumers that
//! need one capability can ask for just that trait.
//!
//! C07 lists the adapter's inputs as "repo path, refs/SHAs, branch/worktree names, operation id, trust state". Every
//! method therefore takes a [`GitContext`] (the repository's trust state, which decides whether repository-controlled
//! code execution must be neutralized, ADR-013, and the operation id that binds the call to its persisted operation
//! record and per-worktree lock, ADR-015) and takes only validated types for anything that becomes a Git argument
//! ([`RemoteName`], [`RevisionSpec`], `BranchName`, `CommitSha`), so an adapter never has to treat a repository-derived
//! string as an option.
//!
//! By design there is no force-push, no rebase of pushed branches and no history rewrite on any capability (ADR-011);
//! the permitted discards are [`GitMerges::abort_merge`] (leave a merge that stopped on conflicts) and
//! [`GitMerges::discard_unpublished_merge`] (reset a clean, Vela-managed integration worktree to a recorded commit).

use std::fmt;

use crate::ids::{BranchName, CommitSha, IdError, WorktreePath};

use super::PortFuture;

/// Whether the repository is trusted to run its own configured code (ADR-013).
///
/// On [`RepoTrust::Untrusted`] repositories, and for read operations generally, an adapter neutralizes
/// repository-controlled execution (hooks path, fsmonitor, external diff, textconv and filter drivers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RepoTrust {
    /// The repository is trusted.
    Trusted,
    /// The repository is untrusted (the default until the user trusts it).
    Untrusted,
}

/// Per-call context: the repository's trust state and the operation this call belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitContext {
    /// Trust state of the repository the call operates on.
    pub trust: RepoTrust,
    /// Identifier of the operation (the persisted operation record and per-worktree lock this call belongs to).
    pub operation: crate::ids::OperationId,
}

/// A configured remote's name (`origin`). Validated so it can never be read as an option.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RemoteName(String);

impl RemoteName {
    /// Accepts `[A-Za-z0-9._-]`, starting with a letter or digit, at most 64 bytes, without `..` or a trailing dot.
    pub fn new(value: impl Into<String>) -> Result<Self, IdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(IdError::Empty);
        }
        if value.len() > 64 {
            return Err(IdError::TooLong { max: 64 });
        }
        if let Some(found) = value
            .chars()
            .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')))
        {
            return Err(IdError::InvalidCharacter { found });
        }
        if !value.starts_with(|c: char| c.is_ascii_alphanumeric()) {
            return Err(IdError::Malformed(
                "a remote name must start with a letter or digit",
            ));
        }
        if value.ends_with('.') || value.contains("..") {
            return Err(IdError::Malformed(
                "a remote name cannot end with `.` or contain `..`",
            ));
        }
        Ok(Self(value))
    }

    /// The name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RemoteName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A revision expression to resolve (`HEAD`, `origin/main`, `HEAD~2`, `v1^{commit}`). Validated so it can never be read
/// as an option or carry whitespace, control characters, or path/pathspec syntax.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RevisionSpec(String);

impl RevisionSpec {
    /// Accepts `[A-Za-z0-9._/@^~{}#-]`, not starting with `-`, at most 256 bytes.
    pub fn new(value: impl Into<String>) -> Result<Self, IdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(IdError::Empty);
        }
        if value.len() > 256 {
            return Err(IdError::TooLong { max: 256 });
        }
        if let Some(found) = value.chars().find(|c| {
            !(c.is_ascii_alphanumeric()
                || matches!(c, '.' | '_' | '/' | '@' | '^' | '~' | '{' | '}' | '#' | '-'))
        }) {
            return Err(IdError::InvalidCharacter { found });
        }
        if value.starts_with('-') {
            return Err(IdError::Malformed("a revision cannot start with `-`"));
        }
        Ok(Self(value))
    }

    /// The expression as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RevisionSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A `Key: value` commit trailer. The key is a token and the value is a single line, so a trailer can neither forge
/// another trailer nor smuggle in a body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trailer {
    key: String,
    value: String,
}

impl Trailer {
    /// Validates the key (`[A-Za-z][A-Za-z0-9-]*`) and the value (non-empty, no line breaks or control characters).
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Result<Self, IdError> {
        let (key, value) = (key.into(), value.into());
        if key.is_empty() || value.trim().is_empty() {
            return Err(IdError::Empty);
        }
        if !key.starts_with(|c: char| c.is_ascii_alphabetic()) {
            return Err(IdError::Malformed("a trailer key must start with a letter"));
        }
        if let Some(found) = key
            .chars()
            .find(|c| !(c.is_ascii_alphanumeric() || *c == '-'))
        {
            return Err(IdError::InvalidCharacter { found });
        }
        if let Some(found) = value.chars().find(|c| c.is_control()) {
            return Err(IdError::InvalidCharacter { found });
        }
        Ok(Self { key, value })
    }

    /// The trailer key.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The trailer value.
    pub fn value(&self) -> &str {
        &self.value
    }
}

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

/// A checkpoint commit message: a subject plus run and ticket trailers. Built only through [`CheckpointMessage::new`],
/// so the subject is always a single line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointMessage {
    subject: String,
    trailers: Vec<Trailer>,
}

impl CheckpointMessage {
    /// Builds a message; the subject must be a non-empty single line without control characters.
    pub fn new(subject: impl Into<String>, trailers: Vec<Trailer>) -> Result<Self, IdError> {
        let subject = subject.into();
        if subject.trim().is_empty() {
            return Err(IdError::Empty);
        }
        if let Some(found) = subject.chars().find(|c| c.is_control()) {
            return Err(IdError::InvalidCharacter { found });
        }
        Ok(Self { subject, trailers })
    }

    /// The commit subject: a non-empty single line.
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// The validated trailers (run, ticket, worker).
    pub fn trailers(&self) -> &[Trailer] {
        &self.trailers
    }
}

/// Result of a merge attempt in the integration worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    /// The merge completed cleanly.
    Merged {
        /// The integration head after the merge.
        head: CommitSha,
        /// The source commit that was merged. Always equal to the `expected_head` the caller passed (the merge is
        /// refused otherwise), reported so the journal records exactly what entered integration.
        merged_source: CommitSha,
    },
    /// The merge stopped on textual conflicts and the integration worktree is mid-merge. The lane calls
    /// [`GitMerges::abort_merge`]; the conflict is then resolved in the *worker's* worktree (the integration tip is
    /// merged into the worker branch, ADR-011 section 5), which returns through checkpoint and review to a new
    /// reviewed head, and the lane merges that head. A conflicted merge is never completed inside the integration
    /// worktree, so no operation for that exists on this port.
    Conflicted {
        /// The conflicting paths.
        paths: Vec<String>,
    },
}

/// Result of a fetch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchOutcome {
    /// Remote-tracking refs that moved, as `refs/remotes/...` names.
    pub updated_refs: Vec<String>,
}

/// Result of a push.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushOutcome {
    /// The commit the remote branch points at after the push.
    pub remote_head: CommitSha,
}

/// Read-only repository inspection.
pub trait GitRead: Send + Sync {
    /// Status of the checkout at `repo`.
    fn status<'a>(
        &'a self,
        ctx: &'a GitContext,
        repo: &'a WorktreePath,
    ) -> PortFuture<'a, RepoStatus>;
    /// Resolves a revision expression to a full commit id.
    fn resolve_ref<'a>(
        &'a self,
        ctx: &'a GitContext,
        repo: &'a WorktreePath,
        revision: &'a RevisionSpec,
    ) -> PortFuture<'a, CommitSha>;
    /// The merge base of two commits.
    fn merge_base<'a>(
        &'a self,
        ctx: &'a GitContext,
        repo: &'a WorktreePath,
        a: &'a CommitSha,
        b: &'a CommitSha,
    ) -> PortFuture<'a, CommitSha>;
    /// Summary of `from...to` (three-dot), the diff a review sees.
    fn diff_summary<'a>(
        &'a self,
        ctx: &'a GitContext,
        repo: &'a WorktreePath,
        from: &'a CommitSha,
        to: &'a CommitSha,
    ) -> PortFuture<'a, DiffSummary>;
}

/// Worktree management.
pub trait GitWorktrees: Send + Sync {
    /// Creates a worktree on a new branch.
    fn add_worktree<'a>(
        &'a self,
        ctx: &'a GitContext,
        request: &'a WorktreeRequest,
    ) -> PortFuture<'a, ()>;
    /// Removes a Vela-managed worktree.
    fn remove_worktree<'a>(
        &'a self,
        ctx: &'a GitContext,
        repo: &'a WorktreePath,
        path: &'a WorktreePath,
    ) -> PortFuture<'a, ()>;
    /// Lists the worktrees of `repo`.
    fn list_worktrees<'a>(
        &'a self,
        ctx: &'a GitContext,
        repo: &'a WorktreePath,
    ) -> PortFuture<'a, Vec<WorktreeInfo>>;
}

/// Checkpoint commits.
pub trait GitCheckpoints: Send + Sync {
    /// Commits all changes in `worktree` as a checkpoint; returns the new commit. Only called while the worker session
    /// is quiescent and under the per-worktree Git operation lock (ADR-015).
    fn commit_checkpoint<'a>(
        &'a self,
        ctx: &'a GitContext,
        worktree: &'a WorktreePath,
        message: &'a CheckpointMessage,
    ) -> PortFuture<'a, CommitSha>;
}

/// Merge-lane operations in the integration worktree.
pub trait GitMerges: Send + Sync {
    /// Merges `source` into the integration worktree with a non-fast-forward merge commit, **bound to the reviewed
    /// commit**: the adapter first resolves the tip of `source` and, if it is not exactly `expected_head` (the
    /// `review_head_sha` that passed review, `REVIEW_PROTOCOL.md`), refuses with a `VALIDATION_FAILURE` error and merges
    /// nothing. A worker branch that moved after review (a late checkpoint) therefore can never enter integration
    /// unreviewed. The merge lane (S12) passes the reviewed head recorded by the review service.
    ///
    /// It is likewise bound to the recorded reset point: the adapter refuses (also `VALIDATION_FAILURE`, nothing
    /// merged) unless the integration worktree is clean and its head is exactly `integration_before`, the value the lane
    /// journaled in `MergeStarted`. `discard_unpublished_merge` can then always return to a state the journal knows.
    fn merge<'a>(
        &'a self,
        ctx: &'a GitContext,
        integration: &'a WorktreePath,
        integration_before: &'a CommitSha,
        source: &'a BranchName,
        expected_head: &'a CommitSha,
    ) -> PortFuture<'a, MergeOutcome>;
    /// Leaves a merge that stopped on conflicts, restoring the integration worktree to its pre-merge state. Calling it
    /// when no merge is in progress succeeds.
    fn abort_merge<'a>(
        &'a self,
        ctx: &'a GitContext,
        integration: &'a WorktreePath,
    ) -> PortFuture<'a, ()>;
    /// Discards an unpublished merge by resetting a clean, Vela-managed integration worktree to a recorded commit.
    /// Requires a clean worktree: after a conflicted merge call [`GitMerges::abort_merge`] first.
    fn discard_unpublished_merge<'a>(
        &'a self,
        ctx: &'a GitContext,
        integration: &'a WorktreePath,
        reset_to: &'a CommitSha,
    ) -> PortFuture<'a, ()>;
}

/// Remote operations. Deliberately has no force option (ADR-011).
pub trait GitRemote: Send + Sync {
    /// Fetches from `remote`.
    fn fetch<'a>(
        &'a self,
        ctx: &'a GitContext,
        repo: &'a WorktreePath,
        remote: &'a RemoteName,
    ) -> PortFuture<'a, FetchOutcome>;
    /// Pushes `branch` to `remote` without force.
    fn push<'a>(
        &'a self,
        ctx: &'a GitContext,
        repo: &'a WorktreePath,
        remote: &'a RemoteName,
        branch: &'a BranchName,
    ) -> PortFuture<'a, PushOutcome>;
}

/// The original `GitAdapter` port: every capability.
pub trait GitAdapter: GitRead + GitWorktrees + GitCheckpoints + GitMerges + GitRemote {}

impl<T> GitAdapter for T where
    T: ?Sized + GitRead + GitWorktrees + GitCheckpoints + GitMerges + GitRemote
{
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_names_cannot_be_read_as_options_or_traverse() {
        assert!(RemoteName::new("origin").is_ok());
        assert!(RemoteName::new("up_stream-2.x").is_ok());
        for bad in [
            "",
            "-origin",
            "--upload-pack=x",
            ".x",
            "a..b",
            "a.",
            "a b",
            "a/b",
            "a:b",
            "a\nb",
            "é",
        ] {
            assert!(RemoteName::new(bad).is_err(), "{bad:?}");
        }
        assert!(RemoteName::new("x".repeat(65)).is_err());
    }

    #[test]
    fn revision_specs_cannot_be_read_as_options_and_carry_no_whitespace() {
        for ok in [
            "HEAD",
            "origin/main",
            "HEAD~2",
            "v1.0^{commit}",
            "HEAD@{1}",
            "a/b#c",
        ] {
            assert!(RevisionSpec::new(ok).is_ok(), "{ok:?}");
        }
        for bad in [
            "",
            "-x",
            "--upload-pack=evil",
            "a b",
            "a\tb",
            "a\nb",
            "a:b",
            "a\\b",
            "a;b",
            "a$b",
            "a`b",
        ] {
            assert!(RevisionSpec::new(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn trailers_cannot_forge_other_trailers_or_a_body() {
        assert!(Trailer::new("Run-Id", "run-1").is_ok());
        assert!(Trailer::new("Ticket", "F02").is_ok());
        for (key, value) in [
            ("", "v"),
            ("k", ""),
            ("k", "   "),
            ("1k", "v"),
            ("k:", "v"),
            ("k k", "v"),
            ("k\nSigned-off-by", "v"),
            ("k", "v\nSigned-off-by: someone"),
            ("k", "v\r"),
        ] {
            assert!(Trailer::new(key, value).is_err(), "{key:?} {value:?}");
        }
    }

    #[test]
    fn checkpoint_subjects_are_single_lines() {
        assert!(CheckpointMessage::new("chore: checkpoint", Vec::new()).is_ok());
        assert!(CheckpointMessage::new("", Vec::new()).is_err());
        assert!(CheckpointMessage::new("a\nb", Vec::new()).is_err());
        let message = CheckpointMessage::new(
            "chore: checkpoint",
            vec![Trailer::new("Ticket", "F02").unwrap()],
        )
        .unwrap();
        assert_eq!(message.subject(), "chore: checkpoint");
        assert_eq!(message.trailers()[0].key(), "Ticket");
    }
}

/// Read access to history that recovery needs when the state store is gone (R05): which `vela/*` branches exist and the
/// messages (with their run and ticket trailers) of the commits at their tips.
pub trait GitHistory: Send + Sync {
    /// Local branches whose names start with `prefix`, each with its tip.
    fn list_branches<'a>(
        &'a self,
        ctx: &'a GitContext,
        repo: &'a WorktreePath,
        prefix: &'a str,
    ) -> PortFuture<'a, Vec<(BranchName, CommitSha)>>;
    /// The full message of a commit.
    fn commit_message<'a>(
        &'a self,
        ctx: &'a GitContext,
        repo: &'a WorktreePath,
        commit: &'a CommitSha,
    ) -> PortFuture<'a, String>;
}
