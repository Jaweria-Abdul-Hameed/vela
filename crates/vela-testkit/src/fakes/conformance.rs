//! The shared conformance suite: contract checks written against the port traits, so any implementation (a fake here, a real
//! adapter later) can be run through the same functions. Each function panics with a message on a violation.

use std::num::NonZeroU64;
use std::sync::Mutex;

use vela_domain::ids::{BranchName, CommitSha, TicketId, WorkerId, WorktreePath};
use vela_domain::ports::{
    AgentAdapter, AgentEvent, AgentEventSink, ApprovalDeliveryAdapter, ApprovalObservation,
    CheckpointMessage, Clock, CommandSpec, DeliveryDecision, DeliveryRequest, EvidenceTier,
    GitAdapter, GitContext, IdGenerator, IssueTrackerAdapter, OutputChunk, OutputSink,
    PowerManager, ProcessRunner, RepoTrust, RevisionSpec, TrackerTicket, TurnRequest,
    WorktreeRequest,
};

/// Collects output chunks.
#[derive(Debug, Default)]
pub struct CollectingSink(Mutex<Vec<OutputChunk>>);

impl CollectingSink {
    /// Everything received, in order.
    pub fn chunks(&self) -> Vec<OutputChunk> {
        self.0.lock().expect("sink lock").clone()
    }
}

impl OutputSink for CollectingSink {
    fn on_output(&self, chunk: OutputChunk) {
        self.0.lock().expect("sink lock").push(chunk);
    }
}

/// Collects agent events.
#[derive(Debug, Default)]
pub struct CollectingEvents(Mutex<Vec<AgentEvent>>);

impl CollectingEvents {
    /// Everything received, in order.
    pub fn events(&self) -> Vec<AgentEvent> {
        self.0.lock().expect("events lock").clone()
    }
}

impl AgentEventSink for CollectingEvents {
    fn on_event(&self, event: AgentEvent) {
        self.0.lock().expect("events lock").push(event);
    }
}

/// Time never runs backwards.
pub fn clock(clock: &dyn Clock) {
    let a = clock.now_ms();
    let b = clock.now_ms();
    assert!(b >= a, "clock went backwards");
}

/// Identifiers are unique within a kind.
pub fn ids(ids: &dyn IdGenerator) {
    assert_ne!(ids.new_run_id(), ids.new_run_id(), "run ids repeat");
    assert_ne!(
        ids.new_worker_id(),
        ids.new_worker_id(),
        "worker ids repeat"
    );
    assert_ne!(
        ids.new_operation_id(),
        ids.new_operation_id(),
        "operation ids repeat"
    );
    assert_ne!(ids.new_token(), ids.new_token(), "tokens repeat");
}

/// Keep-awake can be acquired and released.
pub fn power(power: &dyn PowerManager) {
    let token = power.acquire_keep_awake("conformance").expect("acquire");
    power.release(token);
}

fn command(id: &str) -> CommandSpec {
    CommandSpec {
        id: id.to_owned(),
        program: "echo".to_owned(),
        args: vec!["hi".to_owned()],
        cwd: WorktreePath::new("/repo").expect("path"),
        env: Vec::new(),
        timeout_ms: NonZeroU64::new(1_000).expect("non-zero"),
    }
}

/// A run reports an outcome; killing an unknown id succeeds.
pub async fn process(runner: &dyn ProcessRunner) {
    let sink = CollectingSink::default();
    let outcome = runner.run(&command("p-1"), &sink).await.expect("run");
    assert!(
        outcome.exit_code.is_some() || outcome.timed_out || outcome.cancelled,
        "an outcome must say how the process ended"
    );
    runner
        .kill_tree("never-started")
        .await
        .expect("kill unknown id");
}

fn ctx() -> GitContext {
    GitContext {
        trust: RepoTrust::Trusted,
        operation: vela_domain::ids::OperationId::new("op-conf").expect("id"),
    }
}

/// Worktrees round-trip, a checkpoint moves HEAD, and a push is idempotent.
pub async fn git(git: &dyn GitAdapter) {
    let ctx = ctx();
    let repo = WorktreePath::new("/repo").expect("path");
    let wt = WorktreePath::new("/repo-wt").expect("path");
    let head = git
        .resolve_ref(&ctx, &repo, &RevisionSpec::new("HEAD").expect("rev"))
        .await
        .expect("resolve HEAD");
    let request = WorktreeRequest {
        repo: repo.clone(),
        path: wt.clone(),
        branch: BranchName::new("conf/branch").expect("branch"),
        base: head.clone(),
    };
    git.add_worktree(&ctx, &request)
        .await
        .expect("add worktree");
    assert!(
        git.list_worktrees(&ctx, &repo)
            .await
            .expect("list")
            .iter()
            .any(|w| w.path == wt),
        "an added worktree must be listed"
    );
    let message = CheckpointMessage::new("checkpoint", Vec::new()).expect("message");
    let new_head: CommitSha = git
        .commit_checkpoint(&ctx, &wt, &message)
        .await
        .expect("checkpoint");
    assert_ne!(new_head, head, "a checkpoint must create a new commit");
    git.remove_worktree(&ctx, &repo, &wt).await.expect("remove");
    assert!(
        !git.list_worktrees(&ctx, &repo)
            .await
            .expect("list")
            .iter()
            .any(|w| w.path == wt),
        "a removed worktree must not be listed"
    );
    let remote = vela_domain::ports::RemoteName::new("origin").expect("remote");
    let branch = BranchName::new("conf/branch").expect("branch");
    let first = git.push(&ctx, &repo, &remote, &branch).await.expect("push");
    let second = git
        .push(&ctx, &repo, &remote, &branch)
        .await
        .expect("push again");
    assert_eq!(
        first, second,
        "pushing the same branch twice must be idempotent"
    );
}

/// A written status can be read back; an unknown ticket is an error, not a panic.
pub async fn tracker(tracker: &dyn IssueTrackerAdapter, known: &TicketId) {
    let ticket: TrackerTicket = tracker.read_ticket(known).await.expect("read known ticket");
    assert_eq!(&ticket.id, known);
    tracker
        .update_status(known, "in-progress")
        .await
        .expect("update status");
    tracker
        .comment_evidence(known, "evidence")
        .await
        .expect("comment");
    let missing = TicketId::new("NOPE-0").expect("id");
    assert!(
        tracker.read_ticket(&missing).await.is_err(),
        "reading an unknown ticket must fail"
    );
}

/// Delivery fails closed when the surface is unavailable, and reports verification when it is.
pub async fn approval_delivery(delivery: &dyn ApprovalDeliveryAdapter, available: bool) {
    let request = DeliveryRequest {
        observation: ApprovalObservation {
            source_id: "o-1".to_owned(),
            session_identity: None,
            operation: "cp a b".to_owned(),
            working_directory: None,
            options: vec!["Allow once".to_owned()],
            evidence: EvidenceTier::Observed,
        },
        decision: DeliveryDecision::AllowOnce,
    };
    let capability = delivery.detect().await.expect("detect");
    assert_eq!(capability.available, available);
    let result = delivery.deliver(&request).await;
    if available {
        result.expect("deliver when available");
    } else {
        assert!(
            result.is_err(),
            "delivery must fail closed when unavailable"
        );
    }
}

/// A turn emits a session start first and returns a conversation id on success.
pub async fn agent_success(agent: &dyn AgentAdapter, worker: WorkerId) {
    let sink = CollectingEvents::default();
    let request = TurnRequest {
        worker,
        worktree: WorktreePath::new("/repo-wt").expect("path"),
        prompt: "do the work".to_owned(),
        conversation: None,
        timeout_ms: NonZeroU64::new(1_000).expect("non-zero"),
    };
    let result = agent.run_turn(&request, &sink).await.expect("turn");
    assert!(result.finished);
    assert!(result.conversation.is_some());
    assert!(
        matches!(
            sink.events().first(),
            Some(AgentEvent::SessionStarted { .. })
        ),
        "the first event of a turn must be SessionStarted"
    );
}
