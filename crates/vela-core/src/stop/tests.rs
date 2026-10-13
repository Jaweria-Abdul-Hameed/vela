use std::sync::atomic::{AtomicBool, Ordering};

use vela_domain::ids::{BranchName, TicketId, WorktreePath};
use vela_domain::ports::{PortFuture, ready};
use vela_domain::state::WorkerPhase;
use vela_testkit::fakes::{FakeGit, FakeProcessRunner, ScriptedAgent};

use super::*;
use crate::operations::OperationLog;
use crate::orchestrator::test_support::{TestEnv, env};

struct Lane(AtomicBool);

impl LaneProbe for Lane {
    fn busy(&self, _run: &RunId) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// A session that ignores cancel and keeps reporting that it runs.
struct Stubborn;

impl AgentSession for Stubborn {
    fn run_turn<'a>(
        &'a self,
        _r: &'a vela_domain::ports::TurnRequest,
        _s: &'a dyn vela_domain::ports::AgentEventSink,
    ) -> PortFuture<'a, vela_domain::ports::TurnResult> {
        ready(Err(VelaError::internal("not used")))
    }
    fn cancel<'a>(&'a self, _s: &'a SessionRef) -> PortFuture<'a, ()> {
        ready(Ok(()))
    }
    fn status<'a>(&'a self, _s: &'a SessionRef) -> PortFuture<'a, SessionStatus> {
        ready(Ok(SessionStatus::Running))
    }
}

struct Rig {
    e: TestEnv,
    run: RunId,
    git: Arc<FakeGit>,
    agent: Arc<ScriptedAgent>,
    runner: Arc<FakeProcessRunner>,
    lane: Arc<Lane>,
    stop: StopService,
}

async fn rig(tag: &str, agent_override: Option<Arc<dyn AgentSession>>) -> Rig {
    let e = env(tag);
    let run = e.create_run().await;
    e.drive_run(&run, RunPhase::Running).await;
    let runs = Runs::new(&e.services);
    for n in 0..2 {
        let w = WorkerId::new(format!("w-{n}")).unwrap();
        runs.create_worker(
            &run,
            w.clone(),
            &TicketId::new(format!("T-{n}")).unwrap(),
            BranchName::new(format!("vela/run-1/t-{n}")).unwrap(),
            WorktreePath::new(format!("C:/wt/{n}")).unwrap(),
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
    }
    let git = Arc::new(FakeGit::new());
    let agent = Arc::new(ScriptedAgent::new());
    let runner = Arc::new(FakeProcessRunner::new());
    let lane = Arc::new(Lane(AtomicBool::new(false)));
    let used: Arc<dyn AgentSession> = agent_override.unwrap_or_else(|| agent.clone());
    let stop = StopService::new(
        e.services.clone(),
        used,
        runner.clone(),
        git.clone(),
        lane.clone(),
        Duration::from_millis(40),
    );
    Rig {
        e,
        run,
        git,
        agent,
        runner,
        lane,
        stop,
    }
}

fn integration() -> WorktreePath {
    WorktreePath::new("C:/wt/integration").unwrap()
}

fn worker_state(r: &Rig, n: usize) -> WorkerState {
    Runs::new(&r.e.services)
        .worker_status(&WorkerId::new(format!("w-{n}")).unwrap())
        .unwrap()
        .state()
}

fn clean_report(_r: &Rig) -> RecoveryReport {
    RecoveryReport {
        operations: vec![],
        discrepancies: vec![],
        resumable: vec![WorkerId::new("w-0").unwrap(), WorkerId::new("w-1").unwrap()],
        held: vec![],
        orphans_cancelled: vec![],
    }
}

#[tokio::test]
async fn stop_all_cancels_sessions_pauses_workers_and_preserves_everything() {
    let r = rig("stop", None).await;
    let report = r.stop.stop_all(&r.run, "p1", &integration()).await.unwrap();
    assert_eq!(report.paused.len(), 2);
    assert!(report.forced.is_empty());
    assert_eq!(report.lane, LaneSafePoint::Idle);
    for n in 0..2 {
        assert_eq!(
            worker_state(&r, n),
            WorkerState::Paused {
                reason: PausedReason::StopAll,
                resume_state: WorkerPhase::Implementing
            }
        );
    }
    assert!(matches!(
        Runs::new(&r.e.services).run_state(&r.run).unwrap(),
        RunState::Paused {
            reason: PausedReason::StopAll,
            resume_state: RunPhase::Running
        }
    ));
    let calls = r.agent.log().calls();
    assert!(calls.iter().any(|c| c == "cancel:w-0") && calls.iter().any(|c| c == "cancel:w-1"));
    assert!(
        r.runner.killed().is_empty(),
        "graceful first: nothing killed"
    );
    let k: Vec<String> = r.e.raw_journal().into_iter().map(|(k, _)| k).collect();
    for needle in ["StopRequested", "RunStopping", "WorkerPaused"] {
        assert!(k.iter().any(|x| x == needle), "{needle}");
    }
    assert!(
        !r.git
            .log()
            .calls()
            .iter()
            .any(|c| c.starts_with("remove_worktree")),
        "no work is deleted"
    );
}

#[tokio::test]
async fn a_session_that_will_not_stop_has_its_own_process_tree_killed_and_only_that() {
    let r = rig("forced", Some(Arc::new(Stubborn))).await;
    let report = r.stop.stop_all(&r.run, "p1", &integration()).await.unwrap();
    assert_eq!(report.forced.len(), 2);
    assert_eq!(r.runner.killed(), ["w-0", "w-1"]);
}

#[tokio::test]
async fn an_unfinished_merge_is_discarded_to_the_recorded_commit_and_a_push_is_abandoned() {
    let r = rig("lane-discard", None).await;
    let ops = OperationLog::new(&r.e.services);
    ops.begin(
        &vela_domain::ids::OperationId::new("op-m").unwrap(),
        Some(&r.run),
        "merge",
        &serde_json::json!({"before": "b".repeat(40), "integration": "C:/wt/integration"}),
    )
    .unwrap();
    ops.begin(
        &vela_domain::ids::OperationId::new("op-p").unwrap(),
        Some(&r.run),
        "push",
        &serde_json::json!({}),
    )
    .unwrap();
    let report = r.stop.stop_all(&r.run, "p1", &integration()).await.unwrap();
    assert_eq!(report.lane, LaneSafePoint::Discarded);
    assert!(
        r.git
            .log()
            .calls()
            .iter()
            .any(|c| c == &format!("discard_merge:{}", "b".repeat(40)))
    );
    assert_eq!(
        ops.get(&vela_domain::ids::OperationId::new("op-m").unwrap())
            .unwrap()
            .unwrap()
            .state,
        "FAILED"
    );
    assert_eq!(
        ops.get(&vela_domain::ids::OperationId::new("op-p").unwrap())
            .unwrap()
            .unwrap()
            .state,
        "FAILED"
    );
    assert_eq!(r.stop.health(&r.run).unwrap(), IntegrationHealth::Healthy);
}

#[tokio::test]
async fn a_merge_that_will_not_settle_in_time_marks_integration_unknown() {
    let r = rig("lane-unknown", None).await;
    r.lane.0.store(true, Ordering::SeqCst);
    let report = r.stop.stop_all(&r.run, "p1", &integration()).await.unwrap();
    assert_eq!(report.lane, LaneSafePoint::Unknown);
    assert_eq!(r.stop.health(&r.run).unwrap(), IntegrationHealth::Unknown);
    // A merge that cannot be discarded (no recorded commit) is unknown too.
    let r = rig("lane-nodiscard", None).await;
    OperationLog::new(&r.e.services)
        .begin(
            &vela_domain::ids::OperationId::new("op-m").unwrap(),
            Some(&r.run),
            "merge",
            &serde_json::json!({}),
        )
        .unwrap();
    assert_eq!(
        r.stop
            .stop_all(&r.run, "p1", &integration())
            .await
            .unwrap()
            .lane,
        LaneSafePoint::Unknown
    );
}

#[tokio::test]
async fn a_lane_that_finishes_inside_the_bounded_wait_is_left_alone() {
    let r = rig("lane-finishes", None).await;
    r.lane.0.store(true, Ordering::SeqCst);
    let lane = r.lane.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        lane.0.store(false, Ordering::SeqCst);
    });
    assert_eq!(
        r.stop
            .stop_all(&r.run, "p1", &integration())
            .await
            .unwrap()
            .lane,
        LaneSafePoint::Finished
    );
    assert_eq!(r.stop.health(&r.run).unwrap(), IntegrationHealth::Healthy);
}

#[tokio::test]
async fn resume_needs_a_clean_reconciliation_and_healthy_integration() {
    let r = rig("resume", None).await;
    r.stop.stop_all(&r.run, "p1", &integration()).await.unwrap();

    let mut dirty = clean_report(&r);
    dirty.discrepancies.push(crate::recovery::Discrepancy {
        class: crate::recovery::DiscrepancyClass::RefMoved,
        subject: "w-0".into(),
        expected: "a".into(),
        observed: "b".into(),
    });
    assert_eq!(
        r.stop.resume(&r.run, &dirty).await.unwrap_err().class(),
        ErrorClass::UserActionRequired
    );

    r.e.services
        .store
        .write(|tx| s12_lane::set_health(tx.conn(), "run-1", IntegrationHealth::Unknown))
        .unwrap();
    assert!(r.stop.resume(&r.run, &clean_report(&r)).await.is_err());
    r.e.services
        .store
        .write(|tx| s12_lane::set_health(tx.conn(), "run-1", IntegrationHealth::Healthy))
        .unwrap();

    let resumed = r.stop.resume(&r.run, &clean_report(&r)).await.unwrap();
    assert_eq!(resumed.len(), 2);
    assert_eq!(
        Runs::new(&r.e.services).run_state(&r.run).unwrap(),
        RunState::Active(RunPhase::Running)
    );
    assert_eq!(
        worker_state(&r, 0),
        WorkerState::Active(WorkerPhase::Implementing)
    );
}

#[test]
fn the_stop_fault_points_are_registered() {
    for n in ["stop.bounded_wait_expired", "stop.discard_decision"] {
        assert!(FaultPoint::from_name(n).is_some(), "{n}");
    }
}
