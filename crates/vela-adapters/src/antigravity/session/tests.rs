use std::num::NonZeroU64;
use std::sync::{Arc, Mutex};

use tokio::sync::Notify;
use vela_domain::errors::ErrorCode;
use vela_domain::ids::{WorkerId, WorktreePath};
use vela_domain::ports::{
    AgentEvent, AgentEventSink, AgentSession, CommandSpec, ConversationId, OutputSink, PortFuture,
    ProcessOutcome, ProcessRunner, SessionRef, SessionStatus, TurnRequest,
};
use vela_testkit::fake_agy::{FakeAgy, Scenario};

use super::*;

#[derive(Default)]
struct Events(Mutex<Vec<AgentEvent>>);

impl AgentEventSink for Events {
    fn on_event(&self, event: AgentEvent) {
        self.0.lock().unwrap().push(event);
    }
}

impl Events {
    fn all(&self) -> Vec<AgentEvent> {
        self.0.lock().unwrap().clone()
    }
}

fn worker() -> WorkerId {
    WorkerId::new("w-1").unwrap()
}

fn request(prompt: &str, conversation: Option<&str>, timeout_ms: u64) -> TurnRequest {
    TurnRequest {
        worker: worker(),
        worktree: WorktreePath::new("C:/work/wt-1").unwrap(),
        prompt: prompt.to_owned(),
        conversation: conversation.map(|c| ConversationId::new(c).unwrap()),
        timeout_ms: NonZeroU64::new(timeout_ms).unwrap(),
    }
}

fn driver(fake: &Arc<FakeAgy>) -> AgyDriver {
    AgyDriver::new(fake.clone(), vec![("PATH".into(), "x".into())])
}

#[test]
fn a_new_session_never_passes_conversation_and_a_resume_passes_exactly_the_id() {
    let fresh = turn_args(&request("do it", None, 90_000)).unwrap();
    assert!(!fresh.iter().any(|a| a == "--conversation"));
    assert_eq!(fresh[..2], ["-p".to_owned(), "do it".to_owned()]);
    assert!(
        fresh
            .windows(2)
            .any(|w| w == ["--output-format", "stream-json"])
    );
    let resumed = turn_args(&request(
        "more",
        Some("4f7475be-2459-4b6b-9b30-bbf76791427e"),
        90_000,
    ))
    .unwrap();
    let at = resumed.iter().position(|a| a == "--conversation").unwrap();
    assert_eq!(resumed[at + 1], "4f7475be-2459-4b6b-9b30-bbf76791427e");
    assert_eq!(resumed.iter().filter(|a| *a == "--conversation").count(), 1);
}

#[test]
fn an_explicit_print_timeout_is_always_passed_and_rounded_up() {
    for (ms, expected) in [
        (90_000, "90s"),
        (1, "1s"),
        (1_500, "2s"),
        (2 * 60 * 60 * 1000, "7200s"),
    ] {
        let args = turn_args(&request("x", None, ms)).unwrap();
        let at = args
            .iter()
            .position(|a| a == "--print-timeout")
            .expect("always passed");
        assert_eq!(args[at + 1], expected);
    }
}

#[test]
fn empty_and_oversized_prompts_are_refused() {
    assert!(turn_args(&request("   ", None, 1000)).is_err());
    assert!(turn_args(&request(&"x".repeat(MAX_PROMPT_BYTES + 1), None, 1000)).is_err());
    assert!(turn_args(&request(&"x".repeat(MAX_PROMPT_BYTES), None, 1000)).is_ok());
}

#[tokio::test]
async fn a_turn_runs_in_the_worktree_and_returns_events_a_result_and_the_conversation_id() {
    let fake = Arc::new(FakeAgy::new());
    fake.then_replay("success-pong");
    let events = Events::default();
    let result = driver(&fake)
        .run_turn(&request("Reply with PONG", None, 90_000), &events)
        .await
        .unwrap();
    assert!(result.finished);
    assert_eq!(
        result.conversation.unwrap().as_str(),
        "4f7475be-2459-4b6b-9b30-bbf76791427e"
    );
    let seen = events.all();
    assert!(matches!(
        seen.first(),
        Some(AgentEvent::SessionStarted {
            conversation: Some(_)
        })
    ));
    assert!(seen.iter().any(
        |e| matches!(e, AgentEvent::StepObserved { summary } if summary.starts_with("view_file"))
    ));
    assert_eq!(seen.last(), Some(&AgentEvent::TurnFinished));
    let call = &fake.calls()[0];
    assert_eq!(call.cwd, "C:/work/wt-1");
    assert!(call.args.iter().any(|a| a == "--print-timeout"));
}

#[tokio::test]
async fn a_soft_denial_is_reported_as_a_denial_not_as_work_done() {
    let fake = Arc::new(FakeAgy::new());
    fake.then_replay("soft-denial");
    let events = Events::default();
    let d = driver(&fake);
    let result = d
        .run_turn(&request("run git status", None, 90_000), &events)
        .await
        .unwrap();
    assert!(result.finished);
    let denied = events.all().into_iter().find_map(|e| match e {
        AgentEvent::ToolDenied { tool, reason } => Some((tool, reason)),
        _ => None,
    });
    let (tool, reason) = denied.expect("a ToolDenied event");
    assert_eq!(tool, "command");
    assert!(reason.contains("headless mode cannot prompt"));
    let outcome = d.last_outcome(&worker()).unwrap();
    assert_eq!(outcome.class, TurnClass::SoftDenied);
    assert_eq!(
        outcome
            .steps
            .iter()
            .find(|s| s.tool == "run_command")
            .unwrap()
            .executed,
        crate::antigravity::events::Executed::No
    );
}

#[tokio::test]
async fn hook_decisions_surface_as_tool_denials() {
    for (transcript, needle) in [
        ("hook-deny", "blocked by vela test policy"),
        ("hook-crash", "policy hook failed"),
    ] {
        let fake = Arc::new(FakeAgy::new());
        fake.then_replay(transcript);
        let events = Events::default();
        driver(&fake)
            .run_turn(&request("x", None, 90_000), &events)
            .await
            .unwrap();
        let reasons: Vec<String> = events
            .all()
            .into_iter()
            .filter_map(|e| match e {
                AgentEvent::ToolDenied { reason, .. } => Some(reason),
                _ => None,
            })
            .collect();
        assert!(
            reasons.iter().any(|r| r.contains(needle)),
            "{transcript}: {reasons:?}"
        );
    }
}

#[tokio::test]
async fn a_timeout_is_reported_as_unfinished_never_as_success() {
    let fake = Arc::new(FakeAgy::new());
    fake.then(Scenario::Hang);
    let events = Events::default();
    let d = driver(&fake);
    let result = d
        .run_turn(&request("long task", None, 5_000), &events)
        .await
        .unwrap();
    assert!(!result.finished);
    assert!(!events.all().contains(&AgentEvent::TurnFinished));
    assert_eq!(
        d.last_outcome(&worker()).unwrap().class,
        TurnClass::TimedOut
    );
    let call = &fake.calls()[0];
    let at = call
        .args
        .iter()
        .position(|a| a == "--print-timeout")
        .unwrap();
    assert_eq!(call.args[at + 1], "5s");
}

#[tokio::test]
async fn capacity_authentication_and_policy_failures_are_typed_errors_with_events() {
    let fake = Arc::new(FakeAgy::new());
    fake.then_replay("quota-text").then_replay("auth-error");
    let d = driver(&fake);
    let events = Events::default();
    let err = d
        .run_turn(&request("x", None, 90_000), &events)
        .await
        .unwrap_err();
    assert_eq!(err.class(), ErrorClass::ExternalTemporary);
    assert!(
        events
            .all()
            .iter()
            .any(|e| matches!(e, AgentEvent::CapacityError { .. }))
    );
    let events = Events::default();
    let err = d
        .run_turn(&request("x", None, 90_000), &events)
        .await
        .unwrap_err();
    assert_eq!(err.code(), Some(ErrorCode::AuthRequired));
    assert!(
        events
            .all()
            .iter()
            .any(|e| matches!(e, AgentEvent::AuthRequired { .. }))
    );
}

#[tokio::test]
async fn a_resume_goes_through_the_conversation_flag_and_keeps_the_id() {
    let fake = Arc::new(FakeAgy::new());
    fake.then_replay("resume-pong");
    let events = Events::default();
    let id = "4f7475be-2459-4b6b-9b30-bbf76791427e";
    let result = driver(&fake)
        .run_turn(&request("what word?", Some(id), 90_000), &events)
        .await
        .unwrap();
    assert_eq!(result.conversation.unwrap().as_str(), id);
    assert!(
        fake.calls()[0]
            .args
            .windows(2)
            .any(|w| w == ["--conversation", id])
    );
}

/// A runner whose process only ends when its tree is killed, to prove cancellation reaches the process-tree facility.
struct BlockingRunner {
    started: Notify,
    killed: Notify,
    kills: Mutex<Vec<String>>,
}

impl ProcessRunner for BlockingRunner {
    fn run<'a>(
        &'a self,
        _spec: &'a CommandSpec,
        _sink: &'a dyn OutputSink,
    ) -> PortFuture<'a, ProcessOutcome> {
        Box::pin(async move {
            self.started.notify_one();
            self.killed.notified().await;
            Ok(ProcessOutcome {
                exit_code: None,
                timed_out: false,
                cancelled: true,
                duration_ms: 10,
            })
        })
    }

    fn kill_tree<'a>(&'a self, id: &'a str) -> PortFuture<'a, ()> {
        Box::pin(async move {
            self.kills.lock().unwrap().push(id.to_owned());
            self.killed.notify_one();
            Ok(())
        })
    }
}

#[tokio::test]
async fn cancellation_kills_the_process_tree_and_the_turn_is_cancelled() {
    let runner = Arc::new(BlockingRunner {
        started: Notify::new(),
        killed: Notify::new(),
        kills: Mutex::new(Vec::new()),
    });
    let d = Arc::new(AgyDriver::new(runner.clone(), vec![]));
    let session = SessionRef::worker(worker());
    let turn = {
        let d = d.clone();
        tokio::spawn(async move {
            let events = Events::default();
            d.run_turn(&request("long", None, 60_000), &events).await
        })
    };
    runner.started.notified().await;
    assert_eq!(d.status(&session).await.unwrap(), SessionStatus::Running);
    d.cancel(&session).await.unwrap();
    let result = turn.await.unwrap().unwrap();
    assert!(!result.finished);
    assert_eq!(runner.kills.lock().unwrap().len(), 1);
    assert!(runner.kills.lock().unwrap()[0].starts_with("agy-w-1-"));
    assert_eq!(
        d.last_outcome(&worker()).unwrap().class,
        TurnClass::Cancelled
    );
    assert_eq!(d.status(&session).await.unwrap(), SessionStatus::Idle);
    // Cancelling an idle worker is harmless.
    d.cancel(&session).await.unwrap();
}
