use std::sync::Mutex;

use vela_domain::ids::{BranchName, CommitSha, WorktreePath};
use vela_domain::state::{IntegrationHealth, RunPhase, WorkerPhase};
use vela_persistence::repositories::k01_approvals::ApprovalRow;

use super::*;
use crate::orchestrator::test_support::{TestEnv, env};

struct Probe(Mutex<SessionAvailability>);

impl Probe {
    fn new() -> Arc<Self> {
        Arc::new(Self(Mutex::new(SessionAvailability::Available)))
    }
    fn set(&self, a: SessionAvailability) {
        *self.0.lock().unwrap() = a;
    }
}

impl SessionProbe for Probe {
    fn interactive_session(&self) -> SessionAvailability {
        *self.0.lock().unwrap()
    }
}

struct Rig {
    e: TestEnv,
    run: RunId,
    ui: WorkerId,
    other: WorkerId,
}

async fn rig(tag: &str) -> Rig {
    let e = env(tag);
    let run = e.create_run().await;
    e.drive_run(&run, RunPhase::Running).await;
    let runs = Runs::new(&e.services);
    let mut ids = Vec::new();
    for n in 1..=2 {
        let w = WorkerId::new(format!("w-{n}")).unwrap();
        runs.create_worker(
            &run,
            w.clone(),
            &vela_domain::ids::TicketId::new(format!("T-{n}")).unwrap(),
            BranchName::new(format!("vela/run-1/t-{n}")).unwrap(),
            WorktreePath::new(format!("C:/work/{n}")).unwrap(),
            CommitSha::new("a".repeat(40)).unwrap(),
            "default",
            IntegrationHealth::Healthy,
        )
        .await
        .unwrap();
        for p in [WorkerPhase::ContextLoading, WorkerPhase::Implementing] {
            runs.advance_worker(&run, &w, p, IntegrationHealth::Healthy)
                .await
                .unwrap();
        }
        ids.push(w);
    }
    let other = ids.pop().unwrap();
    let ui = ids.pop().unwrap();
    Rig { e, run, ui, other }
}

fn pending(r: &Rig, id: &str, worker: &WorkerId) {
    let row = ApprovalRow {
        id: id.into(),
        run_id: r.run.as_str().into(),
        worker_id: worker.as_str().into(),
        source: "uia".into(),
        evidence_tier: "BOUND".into(),
        class: "command".into(),
        executable: "git".into(),
        fingerprint: "fp".into(),
        state: "PENDING".into(),
        decision: None,
        rule_id: None,
        reason: String::new(),
        created_ms: 1,
        decided_ms: None,
    };
    r.e.services
        .store
        .write(move |tx| k01_approvals::insert_request(tx.conn(), &row))
        .unwrap();
}

fn approval_state(r: &Rig, id: &str) -> String {
    let id = id.to_owned();
    r.e.services
        .store
        .read(|c| k01_approvals::get_request(c, &id))
        .unwrap()
        .unwrap()
        .state
}

fn state(r: &Rig, w: &WorkerId) -> WorkerState {
    Runs::new(&r.e.services).worker_status(w).unwrap().state()
}

const IMPLEMENTING: WorkerState = WorkerState::Active(WorkerPhase::Implementing);

#[tokio::test]
async fn losing_the_session_pauses_only_the_ui_automation_workers_and_stales_their_pending_approvals()
 {
    let r = rig("lost").await;
    let probe = Probe::new();
    let g = SessionGuard::new(r.e.services.clone(), probe.clone());
    pending(&r, "a-1", &r.ui);
    pending(&r, "a-2", &r.other);
    assert_eq!(
        g.poll(&r.run, std::slice::from_ref(&r.ui)).await.unwrap(),
        Transition::None
    );

    probe.set(SessionAvailability::Locked);
    let t = g.poll(&r.run, std::slice::from_ref(&r.ui)).await.unwrap();
    assert_eq!(
        t,
        Transition::Lost {
            availability: SessionAvailability::Locked,
            paused: vec![r.ui.clone()],
            stale: 1
        }
    );
    assert_eq!(
        state(&r, &r.ui),
        WorkerState::Paused {
            reason: PausedReason::InteractiveSessionUnavailable,
            resume_state: WorkerPhase::Implementing
        }
    );
    assert_eq!(
        state(&r, &r.other),
        IMPLEMENTING,
        "unaffected workers continue"
    );
    assert_eq!(approval_state(&r, "a-1"), "UNDELIVERABLE");
    assert_eq!(approval_state(&r, "a-2"), "PENDING");
}

#[tokio::test]
async fn on_return_workers_resume_but_no_old_click_is_ever_replayed() {
    let r = rig("return").await;
    let probe = Probe::new();
    let g = SessionGuard::new(r.e.services.clone(), probe.clone());
    pending(&r, "a-1", &r.ui);
    probe.set(SessionAvailability::Disconnected);
    g.poll(&r.run, std::slice::from_ref(&r.ui)).await.unwrap();
    // A request that arrived while the session was gone is also stale by the time it returns.
    pending(&r, "a-late", &r.ui);
    probe.set(SessionAvailability::Available);
    let t = g.poll(&r.run, std::slice::from_ref(&r.ui)).await.unwrap();
    assert_eq!(
        t,
        Transition::Returned {
            resumed: vec![r.ui.clone()],
            stale: 1
        }
    );
    assert_eq!(state(&r, &r.ui), IMPLEMENTING);
    // Neither is decided or delivered: only a fresh observation, re-correlated and re-classified, can answer the prompt.
    assert_eq!(approval_state(&r, "a-1"), "UNDELIVERABLE");
    assert_eq!(approval_state(&r, "a-late"), "UNDELIVERABLE");
}

#[tokio::test]
async fn an_unchanged_session_causes_no_transition_even_for_a_long_time() {
    let r = rig("steady").await;
    let probe = Probe::new();
    let g = SessionGuard::new(r.e.services.clone(), probe.clone());
    // A display that is switched off is still an available session: the probe reports Available and nothing happens.
    for _ in 0..5 {
        assert_eq!(
            g.poll(&r.run, std::slice::from_ref(&r.ui)).await.unwrap(),
            Transition::None
        );
    }
    assert_eq!(state(&r, &r.ui), IMPLEMENTING);
    // Staying unavailable is one transition, not one per poll.
    probe.set(SessionAvailability::SecureDesktop);
    assert!(matches!(
        g.poll(&r.run, std::slice::from_ref(&r.ui)).await.unwrap(),
        Transition::Lost { .. }
    ));
    for _ in 0..3 {
        assert_eq!(
            g.poll(&r.run, std::slice::from_ref(&r.ui)).await.unwrap(),
            Transition::None
        );
    }
    // Moving between unavailable kinds changes nothing either.
    probe.set(SessionAvailability::Locked);
    assert_eq!(
        g.poll(&r.run, std::slice::from_ref(&r.ui)).await.unwrap(),
        Transition::None
    );
}

#[tokio::test]
async fn an_unknown_session_is_treated_as_unavailable() {
    let r = rig("unknown").await;
    let probe = Probe::new();
    let g = SessionGuard::new(r.e.services.clone(), probe.clone());
    probe.set(SessionAvailability::Unknown);
    assert!(matches!(
        g.poll(&r.run, std::slice::from_ref(&r.ui)).await.unwrap(),
        Transition::Lost { .. }
    ));
}

#[tokio::test]
async fn a_worker_paused_for_another_reason_is_never_overridden_or_resumed_by_the_session() {
    let r = rig("other-pause").await;
    let probe = Probe::new();
    let g = SessionGuard::new(r.e.services.clone(), probe.clone());
    Runs::new(&r.e.services)
        .transition_worker(
            &r.run,
            &r.ui,
            WorkerState::Paused {
                reason: PausedReason::Capacity,
                resume_state: WorkerPhase::Implementing,
            },
            vela_domain::state::Cause::System,
            IntegrationHealth::Healthy,
            Vec::new(),
        )
        .await
        .unwrap();
    probe.set(SessionAvailability::Locked);
    assert!(matches!(
        g.poll(&r.run, std::slice::from_ref(&r.ui)).await.unwrap(),
        Transition::Lost { paused, .. } if paused.is_empty()
    ));
    probe.set(SessionAvailability::Available);
    assert!(matches!(
        g.poll(&r.run, std::slice::from_ref(&r.ui)).await.unwrap(),
        Transition::Returned { resumed, .. } if resumed.is_empty()
    ));
    assert!(matches!(
        state(&r, &r.ui),
        WorkerState::Paused {
            reason: PausedReason::Capacity,
            ..
        }
    ));
}
