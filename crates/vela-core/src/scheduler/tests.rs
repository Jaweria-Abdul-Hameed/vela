use std::collections::BTreeMap;

use vela_domain::parallel::PredictedWriteSet;
use vela_domain::ports::{PortFuture, TrackerTicket};
use vela_domain::state::WorkerPhase;
use vela_persistence::repositories::o01_workers;
use vela_testkit::fakes::{FakeGit, FakeTracker};

use super::*;
use crate::analysis::{AnalystHook, AnalystOutput, GraphService};
use crate::orchestrator::test_support::{TestEnv, env};
use crate::run::{BuildService, StartOptions};
use crate::settings::Source;
use crate::trust::TrustService;
use vela_domain::graph::DependencyGraph;

fn tid(n: &str) -> TicketId {
    TicketId::new(n).unwrap()
}

fn ticket(id: &str, blockers: &[&str]) -> TrackerTicket {
    TrackerTicket {
        id: tid(id),
        title: id.into(),
        body: String::new(),
        blockers: blockers.iter().map(|b| tid(b)).collect(),
        labels: vec![],
    }
}

/// Disjoint predicted writes for every ticket: all same-layer pairs are parallel-safe.
struct Disjoint;

impl AnalystHook for Disjoint {
    fn analyze<'a>(
        &'a self,
        tickets: &'a [TrackerTicket],
        _g: &'a DependencyGraph,
    ) -> PortFuture<'a, AnalystOutput> {
        let mut out = AnalystOutput::default();
        for t in tickets {
            let mut ws = PredictedWriteSet::default();
            ws.files
                .insert(format!("src/{}/main.rs", t.id.as_str().to_lowercase()));
            ws.directories
                .insert(format!("src/{}", t.id.as_str().to_lowercase()));
            out.write_sets.insert(t.id.clone(), ws);
        }
        Box::pin(async move { Ok(out) })
    }
}

async fn running(
    e: &TestEnv,
    tickets: Vec<TrackerTicket>,
    analyst: bool,
    max_workers: &str,
) -> RunId {
    let run = e.create_run().await;
    let runs = Runs::new(&e.services);
    for p in [RunPhase::Preflight, RunPhase::Analyzing] {
        runs.transition_run(
            &run,
            RunState::Active(p),
            vela_domain::state::Cause::System,
            IntegrationHealth::Healthy,
            vec![],
        )
        .await
        .unwrap();
    }
    let tracker = Arc::new(FakeTracker::new());
    for t in tickets {
        tracker.add(t);
    }
    let hook: Option<Arc<dyn AnalystHook>> = if analyst {
        Some(Arc::new(Disjoint))
    } else {
        None
    };
    GraphService::new(e.services.clone(), tracker, hook)
        .analyze(&run)
        .await
        .unwrap();
    SettingsService::new(&e.services)
        .set_user(Source::User, "max_parallel_workers", max_workers)
        .unwrap();
    SettingsService::new(&e.services)
        .capture_run_snapshot(&run, Some("p1"))
        .unwrap();
    TrustService::new(&e.services)
        .trust("p1", "id", &[], "v1")
        .unwrap();
    let build = BuildService::new(e.services.clone(), Arc::new(FakeGit::new()));
    build.approve_graph(&run).await.unwrap();
    build
        .start(
            &run,
            &StartOptions {
                project_id: "p1".into(),
                repo: WorktreePath::new("C:/repo").unwrap(),
                base_branch: BranchName::new("main").unwrap(),
                expected_base: None,
                acknowledge_divergence: false,
                integration_worktree: WorktreePath::new("C:/vela/wt/run-1/integration").unwrap(),
            },
        )
        .await
        .unwrap();
    run
}

fn input(run: &RunId, health: IntegrationHealth) -> CycleInput {
    CycleInput {
        run: run.clone(),
        worktree_root: "C:/vela/wt".into(),
        integration_head: CommitSha::new("a".repeat(40)).unwrap(),
        health,
    }
}

fn set_worker_state(e: &TestEnv, ticket: &str, state: WorkerState) {
    let rows = e
        .services
        .store
        .read(|c| o01_workers::list_for_run(c, "run-1"))
        .unwrap();
    let id = rows
        .iter()
        .find(|r| r.ticket_id == ticket)
        .unwrap()
        .id
        .clone();
    let json = serde_json::to_string(&state).unwrap();
    e.services
        .store
        .write(move |tx| o01_workers::set_state(tx.conn(), &id, &json, false, 1))
        .unwrap();
}

fn diamond() -> Vec<TrackerTicket> {
    vec![
        ticket("T-1", &[]),
        ticket("T-2", &["T-1"]),
        ticket("T-3", &["T-1"]),
        ticket("T-4", &["T-2", "T-3"]),
    ]
}

#[tokio::test]
async fn the_frontier_advances_as_blockers_finish_and_independent_tickets_run_together() {
    let e = env("diamond");
    let run = running(&e, diamond(), true, "2").await;
    let s = Scheduler::new(e.services.clone());

    let first = s
        .cycle(&input(&run, IntegrationHealth::Healthy))
        .await
        .unwrap();
    assert_eq!(first.started, [tid("T-1")]);
    // Nothing new while T-1 runs: its dependents are not ready.
    let again = s
        .cycle(&input(&run, IntegrationHealth::Healthy))
        .await
        .unwrap();
    assert!(again.started.is_empty());

    set_worker_state(&e, "T-1", WorkerState::Complete);
    let second = s
        .cycle(&input(&run, IntegrationHealth::Healthy))
        .await
        .unwrap();
    assert_eq!(
        second.started,
        [tid("T-2"), tid("T-3")],
        "disjoint writes run together"
    );

    set_worker_state(&e, "T-2", WorkerState::Complete);
    assert!(
        s.cycle(&input(&run, IntegrationHealth::Healthy))
            .await
            .unwrap()
            .started
            .is_empty(),
        "T-3 still blocks T-4"
    );
    set_worker_state(&e, "T-3", WorkerState::Complete);
    assert_eq!(
        s.cycle(&input(&run, IntegrationHealth::Healthy))
            .await
            .unwrap()
            .started,
        [tid("T-4")]
    );
    set_worker_state(&e, "T-4", WorkerState::Complete);
    assert!(
        s.cycle(&input(&run, IntegrationHealth::Healthy))
            .await
            .unwrap()
            .all_done
    );
}

#[tokio::test]
async fn without_analyst_evidence_same_layer_tickets_are_serialized_with_the_reason_recorded() {
    let e = env("sequential");
    let run = running(&e, diamond(), false, "4").await;
    let s = Scheduler::new(e.services.clone());
    s.cycle(&input(&run, IntegrationHealth::Healthy))
        .await
        .unwrap();
    set_worker_state(&e, "T-1", WorkerState::Complete);
    let out = s
        .cycle(&input(&run, IntegrationHealth::Healthy))
        .await
        .unwrap();
    assert_eq!(out.started, [tid("T-2")]);
    assert!(
        matches!(&out.held[0], (t, Serialized::Unsafe { with, .. }) if *t == tid("T-3") && *with == tid("T-2"))
    );
    assert_eq!(
        s.last_cycle(&run).unwrap().unwrap(),
        out,
        "the reason is recorded, not just returned"
    );
}

#[tokio::test]
async fn the_concurrency_cap_holds_back_the_rest() {
    let e = env("cap");
    let run = running(
        &e,
        vec![ticket("A-1", &[]), ticket("A-2", &[]), ticket("A-3", &[])],
        true,
        "2",
    )
    .await;
    let out = Scheduler::new(e.services.clone())
        .cycle(&input(&run, IntegrationHealth::Healthy))
        .await
        .unwrap();
    assert_eq!(out.started.len(), 2);
    assert_eq!(out.held, vec![(tid("A-3"), Serialized::Capacity)]);
}

#[tokio::test]
async fn nothing_starts_while_integration_is_not_healthy() {
    let e = env("health");
    let run = running(&e, diamond(), true, "2").await;
    for h in [IntegrationHealth::Unknown, IntegrationHealth::Unhealthy] {
        let out = Scheduler::new(e.services.clone())
            .cycle(&input(&run, h))
            .await
            .unwrap();
        assert!(out.integration_gate && out.started.is_empty());
    }
    let rows = e
        .services
        .store
        .read(|c| o01_workers::list_for_run(c, "run-1"))
        .unwrap();
    assert!(rows.is_empty());
}

struct Blocked;

impl ProfileAvailability for Blocked {
    fn is_available(&self, _p: &str) -> bool {
        false
    }
}

#[tokio::test]
async fn a_blocked_profile_starts_nothing_and_is_the_recorded_reason() {
    let e = env("profile");
    let run = running(&e, diamond(), true, "2").await;
    e.services
        .register(Arc::new(ProfileAvailabilityService(Arc::new(Blocked))));
    let out = Scheduler::new(e.services.clone())
        .cycle(&input(&run, IntegrationHealth::Healthy))
        .await
        .unwrap();
    assert!(out.started.is_empty());
    assert_eq!(out.held, vec![(tid("T-1"), Serialized::ProfileBlocked)]);
}

#[tokio::test]
async fn a_failed_worker_is_reported_stuck_and_never_restarted_or_unblocking() {
    let e = env("failed");
    let run = running(&e, diamond(), true, "2").await;
    let s = Scheduler::new(e.services.clone());
    s.cycle(&input(&run, IntegrationHealth::Healthy))
        .await
        .unwrap();
    set_worker_state(&e, "T-1", WorkerState::Failed);
    let out = s
        .cycle(&input(&run, IntegrationHealth::Healthy))
        .await
        .unwrap();
    assert!(out.started.is_empty());
    assert_eq!(out.stuck, [tid("T-1")]);
    assert!(!out.all_done);
}

#[tokio::test]
async fn workers_are_created_transactionally_with_branch_and_worktree() {
    let e = env("records");
    let run = running(&e, vec![ticket("T-7", &[])], false, "1").await;
    Scheduler::new(e.services.clone())
        .cycle(&input(&run, IntegrationHealth::Healthy))
        .await
        .unwrap();
    let rows = e
        .services
        .store
        .read(|c| o01_workers::list_for_run(c, "run-1"))
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].branch.as_deref(), Some("vela/run-1/t-7"));
    assert_eq!(
        rows[0].worktree_path.as_deref(),
        Some("C:/vela/wt/run-1/t-7")
    );
    assert_eq!(rows[0].base_sha.as_deref(), Some("a".repeat(40).as_str()));
    assert!(e.raw_journal().iter().any(|(k, _)| k == "WorkerStarted"));
    let _ = (WorkerPhase::Provisioning, BTreeMap::<u8, u8>::new());
}

#[tokio::test]
async fn scheduling_needs_a_running_run() {
    let e = env("not-running");
    let run = e.create_run().await;
    assert!(
        Scheduler::new(e.services.clone())
            .cycle(&input(&run, IntegrationHealth::Healthy))
            .await
            .is_err()
    );
}
