//! Real-environment A03 checks: the session driver against the installed `agy` and the user's own account.
//!
//! Gated by `VELA_REAL_ANTIGRAVITY=1`. These tests spend a little of the user's real Antigravity quota and take about a minute,
//! so they are never part of an ordinary run, and they hold the `RK-AGY-REAL-ACCOUNT` resource key: run them one at a time
//! (`--test-threads=1`, and never next to another real-account test).

use std::num::NonZeroU64;
use std::sync::{Arc, Mutex};

use vela_adapters::antigravity::events::TurnClass;
use vela_adapters::antigravity::session::AgyDriver;
use vela_antigravity_compat::{GATE_ENV, gate_enabled};
use vela_domain::ids::{WorkerId, WorktreePath};
use vela_domain::ports::{
    AgentEvent, AgentEventSink, AgentSession, ConversationId, SessionRef, TurnRequest,
};
use vela_process::redact::PatternRedactor;
use vela_process::runner::{TokioProcessRunner, baseline_env};
use vela_process::tree::TreeRegistry;

#[derive(Default)]
struct Events(Mutex<Vec<AgentEvent>>);

impl AgentEventSink for Events {
    fn on_event(&self, event: AgentEvent) {
        self.0.lock().unwrap().push(event);
    }
}

fn enabled() -> bool {
    let on = gate_enabled(std::env::var(GATE_ENV).ok().as_deref());
    if !on {
        eprintln!("skipped: set {GATE_ENV}=1 to run the real-environment suite");
    }
    on
}

fn driver() -> AgyDriver {
    let runner = TokioProcessRunner::new(
        Arc::new(PatternRedactor::new()),
        Arc::new(TreeRegistry::new()),
    );
    AgyDriver::new(Arc::new(runner), baseline_env())
}

fn scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("vela-real-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(args)
            .output()
            .unwrap();
    };
    git(&["init", "-q"]);
    dir
}

fn request(
    dir: &std::path::Path,
    prompt: &str,
    conversation: Option<ConversationId>,
    timeout_s: u64,
) -> TurnRequest {
    TurnRequest {
        worker: WorkerId::new("w-real").unwrap(),
        worktree: WorktreePath::new(dir.to_string_lossy().replace('\\', "/")).unwrap(),
        prompt: prompt.to_owned(),
        conversation,
        timeout_ms: NonZeroU64::new(timeout_s * 1000).unwrap(),
    }
}

/// A tool-free turn runs, returns the conversation id, and a second turn resumes the same conversation with its context.
#[tokio::test]
async fn a_real_turn_and_a_real_resume_keep_the_conversation() {
    if !enabled() {
        return;
    }
    let dir = scratch("resume");
    let d = driver();
    let events = Events::default();
    let first = d
        .run_turn(
            &request(
                &dir,
                "Reply with exactly the text: PONG. Do not use any tools; just answer.",
                None,
                120,
            ),
            &events,
        )
        .await
        .unwrap();
    assert!(first.finished);
    let id = first.conversation.expect("a conversation id");
    let outcome = d.last_outcome(&WorkerId::new("w-real").unwrap()).unwrap();
    assert_eq!(outcome.class, TurnClass::Completed);
    assert!(outcome.response.contains("PONG"), "{}", outcome.response);

    let second = d
        .run_turn(
            &request(&dir, "What single word did I ask you to reply with? Do not use any tools. Answer with just that word.", Some(id.clone()), 120),
            &Events::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        second.conversation.as_ref(),
        Some(&id),
        "the resume keeps the conversation id"
    );
    let outcome = d.last_outcome(&WorkerId::new("w-real").unwrap()).unwrap();
    assert!(
        outcome.response.to_ascii_uppercase().contains("PONG"),
        "{}",
        outcome.response
    );
    assert!(outcome.num_turns >= 2);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A command with no allow rule is soft-denied in headless mode and the driver says so (probe P1).
#[tokio::test]
async fn a_real_command_without_an_allow_rule_is_reported_as_a_denial() {
    if !enabled() {
        return;
    }
    let dir = scratch("denial");
    let d = driver();
    let events = Events::default();
    d.run_turn(
        &request(
            &dir,
            "Run the shell command: git status --short. Then tell me the output.",
            None,
            150,
        ),
        &events,
    )
    .await
    .unwrap();
    let outcome = d.last_outcome(&WorkerId::new("w-real").unwrap()).unwrap();
    assert_eq!(outcome.class, TurnClass::SoftDenied, "{outcome:?}");
    assert!(
        events
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e, AgentEvent::ToolDenied { .. }))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Cancelling a running turn kills the whole tree and the turn reports unfinished (probe P6).
#[tokio::test]
async fn a_real_cancel_terminates_the_process_tree() {
    if !enabled() {
        return;
    }
    let dir = scratch("cancel");
    let d = Arc::new(driver());
    let session = SessionRef::worker(WorkerId::new("w-real").unwrap());
    let turn = {
        let d = d.clone();
        let req = request(
            &dir,
            "Count slowly from 1 to 200, one number per line, thinking carefully before each.",
            None,
            300,
        );
        tokio::spawn(async move { d.run_turn(&req, &Events::default()).await })
    };
    tokio::time::sleep(std::time::Duration::from_secs(8)).await;
    d.cancel(&session).await.unwrap();
    let result = tokio::time::timeout(std::time::Duration::from_secs(60), turn)
        .await
        .expect("the cancelled turn ends promptly")
        .unwrap()
        .unwrap();
    assert!(!result.finished);
    assert_eq!(
        d.last_outcome(&WorkerId::new("w-real").unwrap())
            .unwrap()
            .class,
        TurnClass::Cancelled
    );
    let _ = std::fs::remove_dir_all(&dir);
}
