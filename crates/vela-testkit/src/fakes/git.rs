//! An in-memory `GitAdapter`: refs, worktrees and merge outcomes held in a mutex, no repository on disk.

use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;

use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::ids::{BranchName, CommitSha, WorktreePath};
use vela_domain::ports::{
    CheckpointMessage, DiffSummary, FetchOutcome, GitCheckpoints, GitContext, GitMerges, GitRead,
    GitRemote, GitWorktrees, MergeOutcome, PortFuture, PushOutcome, RemoteName, RepoStatus,
    RevisionSpec, WorktreeInfo, WorktreeRequest, ready,
};

use super::simple::Recorder;

#[derive(Debug)]
struct State {
    counter: u64,
    refs: BTreeMap<String, CommitSha>,
    head: Option<CommitSha>,
    changed: Vec<String>,
    diff: Vec<String>,
    worktrees: Vec<WorktreeInfo>,
    merges: VecDeque<MergeOutcome>,
    pushed: BTreeMap<String, CommitSha>,
    keep_changed: bool,
}

/// In-memory Git. Every operation is recorded; merge outcomes are scripted (a clean merge when the script is empty).
#[derive(Debug)]
pub struct FakeGit {
    state: Mutex<State>,
    log: Recorder,
}

impl Default for FakeGit {
    fn default() -> Self {
        Self::new()
    }
}

fn sha(n: u64) -> CommitSha {
    CommitSha::new(format!("{n:040x}")).expect("valid sha")
}

impl FakeGit {
    /// A repository with `HEAD` and `main` at the first commit.
    pub fn new() -> Self {
        let first = sha(1);
        let mut refs = BTreeMap::new();
        refs.insert("HEAD".to_owned(), first.clone());
        refs.insert("main".to_owned(), first.clone());
        Self {
            state: Mutex::new(State {
                counter: 1,
                refs,
                head: Some(first),
                changed: Vec::new(),
                diff: Vec::new(),
                worktrees: Vec::new(),
                merges: VecDeque::new(),
                pushed: BTreeMap::new(),
                keep_changed: false,
            }),
            log: Recorder::new(),
        }
    }

    /// The recorded calls.
    pub fn log(&self) -> &Recorder {
        &self.log
    }

    /// Queues the outcome of the next `merge`.
    pub fn push_merge(&self, outcome: MergeOutcome) {
        self.state
            .lock()
            .expect("git lock")
            .merges
            .push_back(outcome);
    }

    /// Sets the paths reported as changed by `status`.
    pub fn set_changed(&self, paths: Vec<String>) {
        self.state.lock().expect("git lock").changed = paths;
    }

    /// When set, a checkpoint leaves the working tree reported as changed (every worker then has something to commit).
    pub fn set_keep_changed(&self, keep: bool) {
        self.state.lock().expect("git lock").keep_changed = keep;
    }

    /// Sets the paths reported by `diff_summary`.
    pub fn set_diff(&self, paths: Vec<String>) {
        self.state.lock().expect("git lock").diff = paths;
    }

    /// Points `name` at `commit`.
    pub fn set_ref(&self, name: &str, commit: CommitSha) {
        self.state
            .lock()
            .expect("git lock")
            .refs
            .insert(name.to_owned(), commit);
    }

    fn next_sha(state: &mut State) -> CommitSha {
        state.counter += 1;
        sha(state.counter)
    }
}

impl GitRead for FakeGit {
    fn status<'a>(
        &'a self,
        _ctx: &'a GitContext,
        repo: &'a WorktreePath,
    ) -> PortFuture<'a, RepoStatus> {
        self.log.record(format!("status:{repo}"));
        let s = self.state.lock().expect("git lock");
        ready(Ok(RepoStatus {
            head: s.head.clone(),
            branch: BranchName::new("main").ok(),
            changed_paths: s.changed.clone(),
        }))
    }

    fn resolve_ref<'a>(
        &'a self,
        _ctx: &'a GitContext,
        _repo: &'a WorktreePath,
        revision: &'a RevisionSpec,
    ) -> PortFuture<'a, CommitSha> {
        self.log.record(format!("resolve:{revision}"));
        let found = self
            .state
            .lock()
            .expect("git lock")
            .refs
            .get(revision.as_str())
            .cloned();
        ready(
            found.ok_or_else(|| {
                VelaError::of_class(ErrorClass::ValidationFailure, "unknown revision")
            }),
        )
    }

    fn merge_base<'a>(
        &'a self,
        _ctx: &'a GitContext,
        _repo: &'a WorktreePath,
        a: &'a CommitSha,
        _b: &'a CommitSha,
    ) -> PortFuture<'a, CommitSha> {
        self.log.record("merge_base");
        ready(Ok(a.clone()))
    }

    fn diff_summary<'a>(
        &'a self,
        _ctx: &'a GitContext,
        _repo: &'a WorktreePath,
        _from: &'a CommitSha,
        _to: &'a CommitSha,
    ) -> PortFuture<'a, DiffSummary> {
        self.log.record("diff_summary");
        let paths = self.state.lock().expect("git lock").diff.clone();
        ready(Ok(DiffSummary {
            changed_paths: paths,
        }))
    }
}

impl GitWorktrees for FakeGit {
    fn add_worktree<'a>(
        &'a self,
        _ctx: &'a GitContext,
        request: &'a WorktreeRequest,
    ) -> PortFuture<'a, ()> {
        self.log.record(format!("add_worktree:{}", request.path));
        let mut s = self.state.lock().expect("git lock");
        if s.worktrees.iter().any(|w| w.path == request.path) {
            return ready(Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "worktree already exists",
            )));
        }
        s.worktrees.push(WorktreeInfo {
            path: request.path.clone(),
            branch: Some(request.branch.clone()),
            head: request.base.clone(),
        });
        ready(Ok(()))
    }

    fn remove_worktree<'a>(
        &'a self,
        _ctx: &'a GitContext,
        _repo: &'a WorktreePath,
        path: &'a WorktreePath,
    ) -> PortFuture<'a, ()> {
        self.log.record(format!("remove_worktree:{path}"));
        self.state
            .lock()
            .expect("git lock")
            .worktrees
            .retain(|w| &w.path != path);
        ready(Ok(()))
    }

    fn list_worktrees<'a>(
        &'a self,
        _ctx: &'a GitContext,
        _repo: &'a WorktreePath,
    ) -> PortFuture<'a, Vec<WorktreeInfo>> {
        self.log.record("list_worktrees");
        ready(Ok(self.state.lock().expect("git lock").worktrees.clone()))
    }
}

impl GitCheckpoints for FakeGit {
    fn commit_checkpoint<'a>(
        &'a self,
        _ctx: &'a GitContext,
        worktree: &'a WorktreePath,
        message: &'a CheckpointMessage,
    ) -> PortFuture<'a, CommitSha> {
        self.log
            .record(format!("checkpoint:{worktree}:{}", message.subject()));
        let mut s = self.state.lock().expect("git lock");
        let new = Self::next_sha(&mut s);
        s.head = Some(new.clone());
        s.refs.insert("HEAD".to_owned(), new.clone());
        if !s.keep_changed {
            s.changed.clear();
        }
        ready(Ok(new))
    }
}

impl GitMerges for FakeGit {
    fn merge<'a>(
        &'a self,
        _ctx: &'a GitContext,
        _integration: &'a WorktreePath,
        _integration_before: &'a CommitSha,
        source: &'a BranchName,
        expected_head: &'a CommitSha,
    ) -> PortFuture<'a, MergeOutcome> {
        self.log.record(format!("merge:{source}"));
        let mut s = self.state.lock().expect("git lock");
        let outcome = match s.merges.pop_front() {
            Some(o) => o,
            None => MergeOutcome::Merged {
                head: Self::next_sha(&mut s),
                merged_source: expected_head.clone(),
            },
        };
        if let MergeOutcome::Merged { head, .. } = &outcome {
            s.head = Some(head.clone());
        }
        ready(Ok(outcome))
    }

    fn abort_merge<'a>(
        &'a self,
        _ctx: &'a GitContext,
        integration: &'a WorktreePath,
    ) -> PortFuture<'a, ()> {
        self.log.record(format!("abort_merge:{integration}"));
        ready(Ok(()))
    }

    fn discard_unpublished_merge<'a>(
        &'a self,
        _ctx: &'a GitContext,
        _integration: &'a WorktreePath,
        reset_to: &'a CommitSha,
    ) -> PortFuture<'a, ()> {
        self.log.record(format!("discard_merge:{reset_to}"));
        self.state.lock().expect("git lock").head = Some(reset_to.clone());
        ready(Ok(()))
    }
}

impl GitRemote for FakeGit {
    fn fetch<'a>(
        &'a self,
        _ctx: &'a GitContext,
        _repo: &'a WorktreePath,
        remote: &'a RemoteName,
    ) -> PortFuture<'a, FetchOutcome> {
        self.log.record(format!("fetch:{remote}"));
        ready(Ok(FetchOutcome {
            updated_refs: Vec::new(),
        }))
    }

    fn push<'a>(
        &'a self,
        _ctx: &'a GitContext,
        _repo: &'a WorktreePath,
        remote: &'a RemoteName,
        branch: &'a BranchName,
    ) -> PortFuture<'a, PushOutcome> {
        self.log.record(format!("push:{remote}:{branch}"));
        let mut s = self.state.lock().expect("git lock");
        let head = s.head.clone().unwrap_or_else(|| sha(1));
        let remote_head = s
            .pushed
            .entry(format!("{remote}/{branch}"))
            .or_insert(head)
            .clone();
        ready(Ok(PushOutcome { remote_head }))
    }
}
