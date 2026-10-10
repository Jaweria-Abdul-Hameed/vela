//! Port contract tests: dyn-compatibility, capability composition, single-capability test doubles.

use super::*;
use crate::errors::{ErrorClass, ErrorCode};
use crate::ids::{BranchName, CommitSha, OperationId, TicketId, WorkerId, WorktreePath};
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

/// Polls a future that completes without waiting (every future in these tests is already ready).
fn block_on<T>(mut future: PortFuture<'_, T>) -> Result<T, VelaError> {
    let mut cx = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut cx) {
        Poll::Ready(result) => result,
        Poll::Pending => panic!("test future was not ready"),
    }
}

fn sha() -> CommitSha {
    CommitSha::new("3e5aafe745ef11887380e0ec9a5dad76fdb3d41d").unwrap()
}

fn path() -> WorktreePath {
    WorktreePath::new("/repo").unwrap()
}

fn ctx() -> GitContext {
    GitContext {
        trust: RepoTrust::Untrusted,
        operation: OperationId::new("op-1").unwrap(),
    }
}

static ONLY_READ: OnlyRead = OnlyRead;
static ONLY_SESSION: OnlySession = OnlySession;

/// A test double that implements ONLY the read capability. It compiles because the capability traits are separate.
struct OnlyRead;

impl GitRead for OnlyRead {
    fn status<'a>(
        &'a self,
        _ctx: &'a GitContext,
        _repo: &'a WorktreePath,
    ) -> PortFuture<'a, RepoStatus> {
        ready(Ok(RepoStatus {
            head: Some(sha()),
            branch: BranchName::new("main").ok(),
            changed_paths: Vec::new(),
        }))
    }
    fn resolve_ref<'a>(
        &'a self,
        _ctx: &'a GitContext,
        _repo: &'a WorktreePath,
        _revision: &'a RevisionSpec,
    ) -> PortFuture<'a, CommitSha> {
        ready(Ok(sha()))
    }
    fn merge_base<'a>(
        &'a self,
        _ctx: &'a GitContext,
        _repo: &'a WorktreePath,
        a: &'a CommitSha,
        _b: &'a CommitSha,
    ) -> PortFuture<'a, CommitSha> {
        ready(Ok(a.clone()))
    }
    fn diff_summary<'a>(
        &'a self,
        _ctx: &'a GitContext,
        _repo: &'a WorktreePath,
        _from: &'a CommitSha,
        _to: &'a CommitSha,
    ) -> PortFuture<'a, DiffSummary> {
        ready(Ok(DiffSummary {
            changed_paths: vec!["a.rs".into()],
        }))
    }
}

/// A double that implements every Git capability, so it is a `GitAdapter` through the blanket implementation.
struct EveryGitCapability;

impl GitRead for EveryGitCapability {
    fn status<'a>(
        &'a self,
        ctx: &'a GitContext,
        repo: &'a WorktreePath,
    ) -> PortFuture<'a, RepoStatus> {
        ONLY_READ.status(ctx, repo)
    }
    fn resolve_ref<'a>(
        &'a self,
        ctx: &'a GitContext,
        repo: &'a WorktreePath,
        revision: &'a RevisionSpec,
    ) -> PortFuture<'a, CommitSha> {
        ONLY_READ.resolve_ref(ctx, repo, revision)
    }
    fn merge_base<'a>(
        &'a self,
        ctx: &'a GitContext,
        repo: &'a WorktreePath,
        a: &'a CommitSha,
        b: &'a CommitSha,
    ) -> PortFuture<'a, CommitSha> {
        ONLY_READ.merge_base(ctx, repo, a, b)
    }
    fn diff_summary<'a>(
        &'a self,
        ctx: &'a GitContext,
        repo: &'a WorktreePath,
        from: &'a CommitSha,
        to: &'a CommitSha,
    ) -> PortFuture<'a, DiffSummary> {
        ONLY_READ.diff_summary(ctx, repo, from, to)
    }
}
impl GitWorktrees for EveryGitCapability {
    fn add_worktree<'a>(
        &'a self,
        _ctx: &'a GitContext,
        _request: &'a WorktreeRequest,
    ) -> PortFuture<'a, ()> {
        ready(Ok(()))
    }
    fn remove_worktree<'a>(
        &'a self,
        _ctx: &'a GitContext,
        _repo: &'a WorktreePath,
        _path: &'a WorktreePath,
    ) -> PortFuture<'a, ()> {
        ready(Ok(()))
    }
    fn list_worktrees<'a>(
        &'a self,
        _ctx: &'a GitContext,
        _repo: &'a WorktreePath,
    ) -> PortFuture<'a, Vec<WorktreeInfo>> {
        ready(Ok(Vec::new()))
    }
}
impl GitCheckpoints for EveryGitCapability {
    fn commit_checkpoint<'a>(
        &'a self,
        _ctx: &'a GitContext,
        _worktree: &'a WorktreePath,
        _message: &'a CheckpointMessage,
    ) -> PortFuture<'a, CommitSha> {
        ready(Ok(sha()))
    }
}
impl GitMerges for EveryGitCapability {
    fn merge<'a>(
        &'a self,
        _ctx: &'a GitContext,
        _integration: &'a WorktreePath,
        _source: &'a BranchName,
        expected_head: &'a CommitSha,
    ) -> PortFuture<'a, MergeOutcome> {
        ready(Ok(MergeOutcome::Merged {
            head: sha(),
            merged_source: expected_head.clone(),
        }))
    }
    fn abort_merge<'a>(
        &'a self,
        _ctx: &'a GitContext,
        _integration: &'a WorktreePath,
    ) -> PortFuture<'a, ()> {
        ready(Ok(()))
    }
    fn discard_unpublished_merge<'a>(
        &'a self,
        _ctx: &'a GitContext,
        _integration: &'a WorktreePath,
        _reset_to: &'a CommitSha,
    ) -> PortFuture<'a, ()> {
        ready(Ok(()))
    }
}
impl GitRemote for EveryGitCapability {
    fn fetch<'a>(
        &'a self,
        _ctx: &'a GitContext,
        _repo: &'a WorktreePath,
        _remote: &'a RemoteName,
    ) -> PortFuture<'a, FetchOutcome> {
        ready(Ok(FetchOutcome {
            updated_refs: Vec::new(),
        }))
    }
    fn push<'a>(
        &'a self,
        _ctx: &'a GitContext,
        _repo: &'a WorktreePath,
        _remote: &'a RemoteName,
        _branch: &'a BranchName,
    ) -> PortFuture<'a, PushOutcome> {
        ready(Ok(PushOutcome { remote_head: sha() }))
    }
}

fn wants_one_capability(git: &dyn GitRead) -> Result<RepoStatus, VelaError> {
    block_on(git.status(&ctx(), &path()))
}

fn wants_the_original_port(git: &dyn GitAdapter) -> Result<MergeOutcome, VelaError> {
    let branch = BranchName::new("f02/x").unwrap();
    block_on(git.merge(&ctx(), &path(), &branch, &sha()))
}

#[test]
fn a_double_implementing_only_one_capability_compiles_and_works() {
    let status = wants_one_capability(&OnlyRead).unwrap();
    assert!(status.is_clean());
}

#[test]
fn capability_traits_compose_into_the_original_port_name() {
    // `EveryGitCapability` never mentions `GitAdapter`; the blanket implementation makes it one.
    let merged = wants_the_original_port(&EveryGitCapability).unwrap();
    assert_eq!(
        merged,
        MergeOutcome::Merged {
            head: sha(),
            merged_source: sha()
        }
    );
    // A full adapter is usable wherever a single capability is wanted (the supertrait relationship).
    let as_adapter: &dyn GitAdapter = &EveryGitCapability;
    let as_read: &dyn GitRead = &EveryGitCapability;
    assert!(
        block_on(as_adapter.status(&ctx(), &path()))
            .unwrap()
            .is_clean()
    );
    assert!(
        block_on(as_read.diff_summary(&ctx(), &path(), &sha(), &sha()))
            .unwrap()
            .is_non_empty()
    );
    // Ports can be shared across tasks.
    let shared: Arc<dyn GitAdapter> = Arc::new(EveryGitCapability);
    let origin = RemoteName::new("origin").unwrap();
    assert!(block_on(shared.fetch(&ctx(), &path(), &origin)).is_ok());
}

#[test]
fn git_calls_carry_the_trust_state_and_operation_identity() {
    let context = ctx();
    assert_eq!(context.trust, RepoTrust::Untrusted);
    assert_eq!(context.operation.as_str(), "op-1");
}

struct OnlySession;

impl AgentSession for OnlySession {
    fn run_turn<'a>(
        &'a self,
        _request: &'a TurnRequest,
        sink: &'a dyn AgentEventSink,
    ) -> PortFuture<'a, TurnResult> {
        sink.on_event(AgentEvent::TurnFinished);
        ready(Ok(TurnResult {
            conversation: Some(ConversationId::new("c-1").unwrap()),
            finished: true,
        }))
    }
    fn cancel<'a>(&'a self, _session: &'a SessionRef) -> PortFuture<'a, ()> {
        ready(Ok(()))
    }
    fn status<'a>(&'a self, session: &'a SessionRef) -> PortFuture<'a, SessionStatus> {
        ready(Ok(match session.role {
            SessionRole::Worker => SessionStatus::Idle,
            SessionRole::Reviewer => SessionStatus::Running,
        }))
    }
}

struct Collect(std::sync::Mutex<Vec<AgentEvent>>);

impl AgentEventSink for Collect {
    fn on_event(&self, event: AgentEvent) {
        self.0.lock().unwrap().push(event);
    }
}

#[test]
fn an_agent_double_implementing_only_the_session_capability_compiles_and_streams_events() {
    let sink = Collect(Default::default());
    let request = TurnRequest {
        worker: WorkerId::new("w-1").unwrap(),
        worktree: path(),
        prompt: "do it".into(),
        conversation: None,
        timeout_ms: 1_000,
    };
    let session: &dyn AgentSession = &OnlySession;
    let result = block_on(session.run_turn(&request, &sink)).unwrap();
    assert!(result.finished);
    assert_eq!(*sink.0.lock().unwrap(), [AgentEvent::TurnFinished]);
}

#[test]
fn a_reviewer_session_is_addressable_separately_from_its_worker_session() {
    let worker = WorkerId::new("w-1").unwrap();
    let session: &dyn AgentSession = &ONLY_SESSION;
    assert_eq!(
        block_on(session.status(&SessionRef::worker(worker.clone()))).unwrap(),
        SessionStatus::Idle
    );
    // Stop All can name, and therefore cancel, the running review.
    let reviewer = SessionRef::reviewer(worker.clone());
    assert_eq!(
        block_on(session.status(&reviewer)).unwrap(),
        SessionStatus::Running
    );
    assert!(block_on(session.cancel(&reviewer)).is_ok());
    assert_ne!(SessionRef::worker(worker.clone()), reviewer);
    let request = ReviewRequest {
        worker,
        worktree: path(),
        fixed_point: sha(),
        reviewed_head: sha(),
        prompt: String::new(),
        timeout_ms: 1,
    };
    assert_eq!(request.worker.as_str(), "w-1");
}

/// A double that implements every agent capability.
struct EveryAgentCapability;

impl AgentSession for EveryAgentCapability {
    fn run_turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        sink: &'a dyn AgentEventSink,
    ) -> PortFuture<'a, TurnResult> {
        ONLY_SESSION.run_turn(request, sink)
    }
    fn cancel<'a>(&'a self, session: &'a SessionRef) -> PortFuture<'a, ()> {
        ONLY_SESSION.cancel(session)
    }
    fn status<'a>(&'a self, session: &'a SessionRef) -> PortFuture<'a, SessionStatus> {
        ONLY_SESSION.status(session)
    }
}
impl AgentProfile for EveryAgentCapability {
    fn capabilities(&self) -> PortFuture<'_, AgentCapabilities> {
        ready(Ok(AgentCapabilities {
            version: "0".into(),
            supported: vec![AgentCapability::Discovery, AgentCapability::Cancel],
            max_concurrent_sessions: None,
        }))
    }
    fn permission_posture(&self) -> PortFuture<'_, PermissionPosture> {
        ready(Ok(PermissionPosture::Unknown))
    }
    fn provision_profile<'a>(&'a self, _session: &'a SessionRef) -> PortFuture<'a, ()> {
        ready(Ok(()))
    }
}
impl AgentReviewer for EveryAgentCapability {
    fn review<'a>(
        &'a self,
        _request: &'a ReviewRequest,
        _sink: &'a dyn AgentEventSink,
    ) -> PortFuture<'a, ReviewReport> {
        ready(Ok(ReviewReport {
            output: String::new(),
            conversation: None,
        }))
    }
}

#[test]
fn agent_capability_traits_compose_into_the_original_port_name() {
    let adapter: &dyn AgentAdapter = &EveryAgentCapability;
    let caps = block_on(adapter.capabilities()).unwrap();
    assert!(caps.supports(AgentCapability::Cancel));
    assert!(!caps.supports(AgentCapability::Resume));
    assert_eq!(
        block_on(adapter.permission_posture()).unwrap(),
        PermissionPosture::Unknown
    );
    let as_session: &dyn AgentSession = &EveryAgentCapability;
    let worker = SessionRef::worker(WorkerId::new("w").unwrap());
    assert_eq!(
        block_on(as_session.status(&worker)).unwrap(),
        SessionStatus::Idle
    );
    assert!(
        block_on(adapter.provision_profile(&SessionRef::reviewer(WorkerId::new("w").unwrap())))
            .is_ok()
    );
}

#[test]
fn the_neutral_provider_policy_block_event_and_typed_error_are_provider_free() {
    let event = AgentEvent::ProviderPolicyBlock {
        evidence: "service disabled for terms reasons".into(),
    };
    assert!(matches!(event, AgentEvent::ProviderPolicyBlock { .. }));
    let error = VelaError::from_code(ErrorCode::ProviderPolicyBlock, "blocked");
    assert_eq!(error.class, ErrorClass::PolicyBlock);
}

struct NoPullRequests;

impl IssueTrackerAdapter for NoPullRequests {
    fn list_tickets(&self) -> PortFuture<'_, Vec<TrackerTicket>> {
        ready(Ok(Vec::new()))
    }
    fn read_ticket<'a>(&'a self, id: &'a TicketId) -> PortFuture<'a, TrackerTicket> {
        ready(Ok(TrackerTicket {
            id: id.clone(),
            title: String::new(),
            body: String::new(),
            blockers: Vec::new(),
            labels: Vec::new(),
        }))
    }
    fn update_status<'a>(&'a self, _id: &'a TicketId, _status: &'a str) -> PortFuture<'a, ()> {
        ready(Ok(()))
    }
    fn comment_evidence<'a>(&'a self, _id: &'a TicketId, _body: &'a str) -> PortFuture<'a, ()> {
        ready(Ok(()))
    }
    fn close_ticket<'a>(&'a self, _id: &'a TicketId) -> PortFuture<'a, ()> {
        ready(Ok(()))
    }
}

#[test]
fn a_tracker_without_pull_requests_reports_the_capability_as_missing() {
    let tracker: &dyn IssueTrackerAdapter = &NoPullRequests;
    let id = TicketId::new("F02").unwrap();
    let err = block_on(tracker.link_pull_request(&id, "https://example.invalid/pr/1")).unwrap_err();
    assert_eq!(err.code, Some(ErrorCode::CapabilityMissing));
}

#[test]
fn the_session_probe_only_permits_ui_automation_when_explicitly_available() {
    assert!(SessionAvailability::Available.permits_ui_automation());
    for state in [
        SessionAvailability::Locked,
        SessionAvailability::Disconnected,
        SessionAvailability::SecureDesktop,
        SessionAvailability::Unknown,
    ] {
        assert!(!state.permits_ui_automation(), "{state:?}");
    }
}

/// Compile-time check that every port is dyn-compatible: naming `dyn Trait` fails to compile otherwise.
#[test]
fn every_port_is_dyn_compatible() {
    fn check<T: ?Sized>() {}
    check::<dyn AgentAdapter>();
    check::<dyn GitAdapter>();
    check::<dyn IssueTrackerAdapter>();
    check::<dyn ProcessRunner>();
    check::<dyn Notifier>();
    check::<dyn ApprovalSource>();
    check::<dyn ApprovalDeliveryAdapter>();
    check::<dyn PowerManager>();
    check::<dyn SessionProbe>();
    check::<dyn Clock>();
    check::<dyn IdGenerator>();
    check::<dyn OutputSink>();
    check::<dyn AgentEventSink>();
}
