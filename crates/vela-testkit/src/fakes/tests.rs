use vela_domain::ids::{TicketId, WorkerId};
use vela_domain::ports::{
    AgentEvent, DeliveryTier, SessionAvailability, TrackerTicket, TurnRequest,
};

use super::conformance::{self, CollectingEvents};
use super::*;

fn worker(s: &str) -> WorkerId {
    WorkerId::new(s).unwrap()
}

#[test]
fn simple_fakes_conform() {
    let clock = FakeClock::new(1_000);
    conformance::clock(&clock);
    clock.advance(50);
    assert_eq!(vela_domain::ports::Clock::now_ms(&clock), 1_050);
    conformance::ids(&FakeIds::new());
    let power = FakePower::new();
    conformance::power(&power);
    assert_eq!(power.log().calls().len(), 2);
    power.fail_next_acquire();
    assert!(vela_domain::ports::PowerManager::acquire_keep_awake(&power, "x").is_err());
    let probe = FakeSessionProbe::new(SessionAvailability::Locked);
    assert!(!vela_domain::ports::SessionProbe::interactive_session(&probe).permits_ui_automation());
}

#[tokio::test]
async fn process_fake_conforms_and_scripts() {
    let runner = FakeProcessRunner::new();
    conformance::process(&runner).await;
    runner.push(Scripted::timeout());
    let sink = conformance::CollectingSink::default();
    let outcome = vela_domain::ports::ProcessRunner::run(&runner, &conformance_command(), &sink)
        .await
        .unwrap();
    assert!(outcome.timed_out);
    assert_eq!(runner.killed(), vec!["never-started".to_owned()]);
}

fn conformance_command() -> vela_domain::ports::CommandSpec {
    vela_domain::ports::CommandSpec {
        id: "p".into(),
        program: "x".into(),
        args: vec![],
        cwd: vela_domain::ids::WorktreePath::new("/r").unwrap(),
        env: vec![("TOKEN".into(), "s3cr3t".into())],
        timeout_ms: std::num::NonZeroU64::new(5).unwrap(),
    }
}

#[tokio::test]
async fn process_fake_never_records_environment_values() {
    let runner = FakeProcessRunner::new();
    let sink = conformance::CollectingSink::default();
    vela_domain::ports::ProcessRunner::run(&runner, &conformance_command(), &sink)
        .await
        .unwrap();
    assert!(runner.log().calls().iter().all(|c| !c.contains("s3cr3t")));
}

#[tokio::test]
async fn git_fake_conforms() {
    conformance::git(&FakeGit::new()).await;
}

#[tokio::test]
async fn tracker_fake_conforms_and_models_an_outage() {
    let tracker = FakeTracker::new();
    let id = TicketId::new("T-1").unwrap();
    tracker.add(TrackerTicket {
        id: id.clone(),
        title: "t".into(),
        body: "b".into(),
        blockers: vec![],
        labels: vec![],
    });
    conformance::tracker(&tracker, &id).await;
    assert_eq!(tracker.status_of(&id).as_deref(), Some("in-progress"));
    tracker.set_outage(true);
    assert!(
        vela_domain::ports::IssueTrackerAdapter::close_ticket(&tracker, &id)
            .await
            .is_err()
    );
    assert!(tracker.closed().is_empty());
}

#[tokio::test]
async fn approval_delivery_fake_fails_closed() {
    let up = FakeApprovalDelivery::new(DeliveryTier::UiAutomation);
    conformance::approval_delivery(&up, true).await;
    let down = FakeApprovalDelivery::new(DeliveryTier::UiAutomation);
    down.set_available(false);
    conformance::approval_delivery(&down, false).await;
}

#[tokio::test]
async fn scripted_agent_conforms_and_reproduces_every_scenario() {
    let agent = ScriptedAgent::new();
    conformance::agent_success(&agent, worker("w-ok")).await;

    for (name, scenario, expect_err) in [
        ("w-fail", Scenario::Failure, true),
        ("w-cap", Scenario::Capacity, true),
        ("w-block", Scenario::ProviderPolicyBlock, true),
        ("w-time", Scenario::Timeout, false),
        ("w-amb", Scenario::Ambiguity, false),
        ("w-test", Scenario::FailingTests, false),
    ] {
        // Two runs of the same scenario must be identical (determinism).
        let mut runs = Vec::new();
        for _ in 0..2 {
            let agent = ScriptedAgent::new();
            agent.script(worker(name), scenario);
            let sink = CollectingEvents::default();
            let request = TurnRequest {
                worker: worker(name),
                worktree: vela_domain::ids::WorktreePath::new("/r").unwrap(),
                prompt: String::new(),
                conversation: None,
                timeout_ms: std::num::NonZeroU64::new(5).unwrap(),
            };
            let result = vela_domain::ports::AgentSession::run_turn(&agent, &request, &sink).await;
            assert_eq!(result.is_err(), expect_err, "{scenario:?}");
            runs.push((format!("{:?}", result.map(|r| r.finished)), sink.events()));
        }
        assert_eq!(runs[0], runs[1], "{scenario:?} must be deterministic");
    }

    let blocked = ScriptedAgent::new();
    blocked.script(worker("w-b"), Scenario::ProviderPolicyBlock);
    let sink = CollectingEvents::default();
    let request = TurnRequest {
        worker: worker("w-b"),
        worktree: vela_domain::ids::WorktreePath::new("/r").unwrap(),
        prompt: String::new(),
        conversation: None,
        timeout_ms: std::num::NonZeroU64::new(5).unwrap(),
    };
    let err = vela_domain::ports::AgentSession::run_turn(&blocked, &request, &sink)
        .await
        .unwrap_err();
    assert_eq!(
        err.code(),
        Some(vela_domain::errors::ErrorCode::ProviderPolicyBlock)
    );
    assert!(
        sink.events()
            .iter()
            .any(|e| matches!(e, AgentEvent::ProviderPolicyBlock { .. }))
    );
}

#[test]
fn fakes_never_touch_the_network_home_or_environment() {
    for (name, source) in [
        ("agent", include_str!("agent.rs")),
        ("approval", include_str!("approval.rs")),
        ("git", include_str!("git.rs")),
        ("process", include_str!("process.rs")),
        ("simple", include_str!("simple.rs")),
        ("tracker", include_str!("tracker.rs")),
    ] {
        for banned in [
            "std::env",
            "std::net",
            "std::fs",
            "TcpStream",
            "UdpSocket",
            "reqwest",
            "home_dir",
            "USERPROFILE",
        ] {
            assert!(!source.contains(banned), "{name}.rs uses {banned}");
        }
    }
}
