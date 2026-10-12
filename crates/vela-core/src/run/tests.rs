use vela_domain::ids::TicketId;
use vela_domain::ports::TrackerTicket;
use vela_testkit::fakes::{FakeGit, FakeTracker};

use super::*;
use crate::analysis::GraphService;
use crate::orchestrator::test_support::env;
use crate::trust::TrustService;

async fn graph_ready(e: &crate::orchestrator::test_support::TestEnv) -> RunId {
    let run = e.create_run().await;
    let runs = Runs::new(&e.services);
    for p in [RunPhase::Preflight, RunPhase::Analyzing] {
        runs.transition_run(
            &run,
            RunState::Active(p),
            Cause::System,
            IntegrationHealth::Healthy,
            vec![],
        )
        .await
        .unwrap();
    }
    let t = Arc::new(FakeTracker::new());
    t.add(TrackerTicket {
        id: TicketId::new("T-1").unwrap(),
        title: "one".into(),
        body: String::new(),
        blockers: vec![],
        labels: vec![],
    });
    GraphService::new(e.services.clone(), t, None)
        .analyze(&run)
        .await
        .unwrap();
    run
}

fn options(expected: Option<CommitSha>, ack: bool) -> StartOptions {
    StartOptions {
        project_id: "p1".into(),
        repo: WorktreePath::new("C:/repo").unwrap(),
        base_branch: BranchName::new("main").unwrap(),
        expected_base: expected,
        acknowledge_divergence: ack,
        integration_worktree: WorktreePath::new("C:/vela/wt/run-1/integration").unwrap(),
    }
}

fn svc(e: &crate::orchestrator::test_support::TestEnv) -> (BuildService, Arc<FakeGit>) {
    let git = Arc::new(FakeGit::new());
    (BuildService::new(e.services.clone(), git.clone()), git)
}

#[tokio::test]
async fn approving_then_building_records_the_base_creates_the_integration_worktree_and_runs() {
    let e = env("build");
    let run = graph_ready(&e).await;
    let (b, git) = svc(&e);
    TrustService::new(&e.services)
        .trust("p1", "id", &[], "v1")
        .unwrap();
    b.approve_graph(&run).await.unwrap();
    assert_eq!(
        Runs::new(&e.services).run_state(&run).unwrap(),
        RunState::Active(RunPhase::AwaitingStart)
    );
    assert!(
        crate::analysis::GraphService::new(e.services.clone(), Arc::new(FakeTracker::new()), None)
            .stored(&run)
            .unwrap()
            .unwrap()
            .approved_ms
            .is_some()
    );

    let base = b.start(&run, &options(None, false)).await.unwrap();
    assert_eq!(base.integration_branch, "vela/run-1/integration");
    assert_eq!(base.base_sha.len(), 40);
    assert_eq!(
        Runs::new(&e.services).run_state(&run).unwrap(),
        RunState::Active(RunPhase::Running)
    );
    assert_eq!(b.base_of(&run).unwrap().unwrap(), base);
    assert!(
        git.log()
            .calls()
            .iter()
            .any(|c| c.starts_with("add_worktree:"))
    );
    assert!(
        e.raw_journal()
            .iter()
            .any(|(k, _)| k == "WorktreeProvisioned")
    );
    let worktrees = e
        .services
        .store
        .read(|c| vela_persistence::repositories::s08_build::list_worktrees(c, "run-1"))
        .unwrap();
    assert_eq!(worktrees.len(), 1);
    assert_eq!(worktrees[0].1, "INTEGRATION");
}

#[tokio::test]
async fn an_untrusted_repository_cannot_be_built_and_nothing_is_created() {
    let e = env("untrusted-build");
    let run = graph_ready(&e).await;
    let (b, git) = svc(&e);
    b.approve_graph(&run).await.unwrap();
    let err = b.start(&run, &options(None, false)).await.unwrap_err();
    assert_eq!(err.class(), ErrorClass::PolicyBlock);
    assert_eq!(
        Runs::new(&e.services).run_state(&run).unwrap(),
        RunState::Active(RunPhase::AwaitingStart)
    );
    assert!(
        !git.log()
            .calls()
            .iter()
            .any(|c| c.starts_with("add_worktree"))
    );
}

#[tokio::test]
async fn a_moved_base_needs_acknowledgement_before_anything_is_created() {
    let e = env("diverged");
    let run = graph_ready(&e).await;
    let (b, git) = svc(&e);
    TrustService::new(&e.services)
        .trust("p1", "id", &[], "v1")
        .unwrap();
    b.approve_graph(&run).await.unwrap();
    let stale = CommitSha::new("f".repeat(40)).unwrap();
    let err = b
        .start(&run, &options(Some(stale.clone()), false))
        .await
        .unwrap_err();
    assert_eq!(err.class(), ErrorClass::UserActionRequired);
    assert_eq!(
        Runs::new(&e.services).run_state(&run).unwrap(),
        RunState::Active(RunPhase::AwaitingStart)
    );
    assert!(
        !git.log()
            .calls()
            .iter()
            .any(|c| c.starts_with("add_worktree"))
    );
    // Acknowledged: it builds on the current base, not the stale one.
    let base = b.start(&run, &options(Some(stale), true)).await.unwrap();
    assert_ne!(base.base_sha, "f".repeat(40));
}

#[tokio::test]
async fn build_needs_an_approved_graph_and_only_starts_once() {
    let e = env("states");
    let run = graph_ready(&e).await;
    let (b, _git) = svc(&e);
    TrustService::new(&e.services)
        .trust("p1", "id", &[], "v1")
        .unwrap();
    assert!(
        b.start(&run, &options(None, false)).await.is_err(),
        "not approved yet"
    );
    b.approve_graph(&run).await.unwrap();
    assert!(b.approve_graph(&run).await.is_err(), "approval is once");
    b.start(&run, &options(None, false)).await.unwrap();
    assert!(
        b.start(&run, &options(None, false)).await.is_err(),
        "the build starts once"
    );
}

#[test]
fn the_build_fault_points_are_registered() {
    for name in [
        "build.before_branch",
        "build.after_worktree",
        "build.before_running",
    ] {
        assert!(FaultPoint::from_name(name).is_some(), "{name}");
    }
}
