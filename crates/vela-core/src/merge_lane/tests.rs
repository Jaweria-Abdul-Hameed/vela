use vela_domain::ports::{OutputChunk, OutputStream};
use vela_domain::state::RunPhase;
use vela_testkit::fakes::{FakeGit, FakeProcessRunner, Scripted};

use super::*;
use crate::orchestrator::test_support::{TestEnv, env};

struct Rig {
    e: TestEnv,
    git: Arc<FakeGit>,
    runner: Arc<FakeProcessRunner>,
    lane: Arc<MergeLane>,
    req: LaneRequest,
}

async fn rig(tag: &str, with_remote: bool) -> Rig {
    let e = env(tag);
    let run = e.create_run().await;
    e.drive_run(&run, RunPhase::Running).await;
    let worker = WorkerId::new("w-1").unwrap();
    let ticket = TicketId::new("T-1").unwrap();
    let branch = BranchName::new("vela/run-1/t-1").unwrap();
    let head = CommitSha::new("c".repeat(40)).unwrap();
    let runs = Runs::new(&e.services);
    runs.create_worker(
        &run,
        worker.clone(),
        &ticket,
        branch.clone(),
        WorktreePath::new("C:/wt/t1").unwrap(),
        CommitSha::new("a".repeat(40)).unwrap(),
        "default",
        IntegrationHealth::Healthy,
    )
    .await
    .unwrap();
    for p in [
        WorkerPhase::ContextLoading,
        WorkerPhase::Implementing,
        WorkerPhase::FocusedValidation,
        WorkerPhase::Checkpointing,
        WorkerPhase::Reviewing,
        WorkerPhase::ReadyToMerge,
    ] {
        runs.advance_worker(&run, &worker, p, IntegrationHealth::Healthy)
            .await
            .unwrap();
    }
    TrustService::new(&e.services)
        .trust(
            "p1",
            "id",
            &[ProfileCommand {
                text: "cargo test".into(),
                cwd: None,
            }],
            "v1",
        )
        .unwrap();
    let git = Arc::new(FakeGit::new());
    let runner = Arc::new(FakeProcessRunner::new());
    let lane = Arc::new(MergeLane::new(
        e.services.clone(),
        git.clone(),
        runner.clone(),
    ));
    let req = LaneRequest {
        run,
        project_id: "p1".into(),
        worker,
        ticket,
        branch,
        reviewed_head: head,
        integration: WorktreePath::new("C:/wt/integration").unwrap(),
        integration_branch: BranchName::new("vela/run-1/integration").unwrap(),
        validation: vec![ProfileCommand {
            text: "cargo test".into(),
            cwd: None,
        }],
        remote: with_remote.then(|| RemoteName::new("origin").unwrap()),
    };
    Rig {
        e,
        git,
        runner,
        lane,
        req,
    }
}

fn worker_state(r: &Rig) -> WorkerState {
    Runs::new(&r.e.services)
        .worker_status(&r.req.worker)
        .unwrap()
        .state()
}

fn kinds(r: &Rig) -> Vec<String> {
    r.e.raw_journal().into_iter().map(|(k, _)| k).collect()
}

#[tokio::test]
async fn a_reviewed_branch_merges_validates_completes_and_pushes() {
    let r = rig("merge", true).await;
    let out = r.lane.merge(&r.req).await.unwrap();
    let LaneOutcome::Merged { pushed, .. } = out else {
        panic!("expected merged, got {out:?}")
    };
    assert!(pushed);
    assert_eq!(worker_state(&r), WorkerState::Complete);
    assert_eq!(
        r.lane.health(&r.req.run).unwrap(),
        IntegrationHealth::Healthy
    );
    let k = kinds(&r);
    let pos = |name: &str| {
        k.iter()
            .position(|x| x == name)
            .unwrap_or_else(|| panic!("{name} missing in {k:?}"))
    };
    assert!(
        pos("MergeStarted") < pos("TestGateFinished")
            && pos("TestGateFinished") < pos("MergeCompleted")
            && pos("MergeCompleted") < pos("PushCompleted")
    );
    let calls = r.git.log().calls();
    assert!(calls.iter().any(|c| c.starts_with("merge:vela/run-1/t-1")));
    assert!(
        calls
            .iter()
            .any(|c| c == "push:origin:vela/run-1/integration")
    );
    // No operation is left half-done.
    let pending =
        r.e.services
            .store
            .read(vela_persistence::repositories::r01_operations::list_intended)
            .unwrap();
    assert!(pending.is_empty());
}

#[tokio::test]
async fn no_remote_means_merged_but_not_pushed() {
    let r = rig("no-remote", false).await;
    let LaneOutcome::Merged { pushed, .. } = r.lane.merge(&r.req).await.unwrap() else {
        panic!()
    };
    assert!(!pushed);
    assert!(!r.git.log().calls().iter().any(|c| c.starts_with("push:")));
}

#[tokio::test]
async fn a_failing_gate_discards_the_merge_and_sends_the_worker_back_to_fixing() {
    let r = rig("gate-fails", true).await;
    r.runner.push(Scripted::exit(1).with_output(OutputChunk {
        stream: OutputStream::Stderr,
        text: "integration test broke\n".into(),
    }));
    let out = r.lane.merge(&r.req).await.unwrap();
    assert_eq!(
        out,
        LaneOutcome::SentBackToFix {
            gate: "cargo test".into(),
            attempts: 1
        }
    );
    assert_eq!(worker_state(&r), WorkerState::Active(WorkerPhase::Fixing));
    assert_eq!(
        r.lane.health(&r.req.run).unwrap(),
        IntegrationHealth::Healthy,
        "discarded cleanly, so integration is trusted again"
    );
    let k = kinds(&r);
    assert!(
        k.contains(&"IntegrationGateFailed".to_owned()) && k.contains(&"MergeDiscarded".to_owned())
    );
    assert!(!k.contains(&"MergeCompleted".to_owned()));
    let calls = r.git.log().calls();
    assert!(calls.iter().any(|c| c.starts_with("discard_merge:")));
    assert!(
        !calls.iter().any(|c| c.starts_with("push:")),
        "an invalid merge is never pushed"
    );
}

#[tokio::test]
async fn repeated_integration_failures_past_the_cap_need_a_human() {
    let r = rig("cap", true).await;
    SettingsService::new(&r.e.services)
        .set_user(
            crate::settings::Source::User,
            "integration_fix_attempts",
            "0",
        )
        .unwrap();
    SettingsService::new(&r.e.services)
        .capture_run_snapshot(&r.req.run, Some("p1"))
        .unwrap();
    r.runner.push(Scripted::exit(1));
    let out = r.lane.merge(&r.req).await.unwrap();
    assert!(matches!(out, LaneOutcome::FixCapReached { .. }));
    assert!(matches!(
        worker_state(&r),
        WorkerState::NeedsHuman {
            kind: InterventionKind::IntegrationRegression,
            ..
        }
    ));
}

#[tokio::test]
async fn a_conflict_aborts_the_merge_and_escalates_without_touching_integration() {
    let r = rig("conflict", true).await;
    r.git.push_merge(MergeOutcome::Conflicted {
        paths: vec!["src/a.rs".into()],
    });
    let out = r.lane.merge(&r.req).await.unwrap();
    let LaneOutcome::Conflict { paths, .. } = out else {
        panic!()
    };
    assert_eq!(paths, ["src/a.rs"]);
    assert!(
        r.git
            .log()
            .calls()
            .iter()
            .any(|c| c.starts_with("abort_merge:"))
    );
    assert!(!r.git.log().calls().iter().any(|c| c.starts_with("push:")));
    assert!(matches!(
        worker_state(&r),
        WorkerState::NeedsHuman {
            kind: InterventionKind::MergeConflict,
            ..
        }
    ));
    assert_eq!(
        r.lane.health(&r.req.run).unwrap(),
        IntegrationHealth::Healthy
    );
    assert!(kinds(&r).contains(&"ConflictResolutionStarted".to_owned()));
}

#[tokio::test]
async fn a_gate_outside_the_confirmed_profile_is_refused_and_the_merge_discarded() {
    let mut r = rig("policy", true).await;
    r.req.validation = vec![ProfileCommand {
        text: "cargo publish".into(),
        cwd: None,
    }];
    let err = r.lane.merge(&r.req).await.unwrap_err();
    assert_eq!(err.class(), ErrorClass::PolicyBlock);
    assert!(
        r.git
            .log()
            .calls()
            .iter()
            .any(|c| c.starts_with("discard_merge:")),
        "the unvalidated merge does not stay"
    );
    assert!(!r.runner.log().calls().iter().any(|c| c.contains("publish")));
    assert_eq!(
        r.lane.health(&r.req.run).unwrap(),
        IntegrationHealth::Healthy
    );
}

#[tokio::test]
async fn merges_of_one_run_never_overlap() {
    let r = rig("serial", false).await;
    let lock = r.lane.lock_for(&r.req.run);
    let guard = lock.lock().await;
    let lane = r.lane.clone();
    let req = r.req.clone();
    let task = tokio::spawn(async move { lane.merge(&req).await });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(!task.is_finished(), "the second merge waits for the lane");
    assert!(r.git.log().calls().iter().all(|c| !c.starts_with("merge:")));
    drop(guard);
    assert!(matches!(
        task.await.unwrap().unwrap(),
        LaneOutcome::Merged { .. }
    ));
}

#[tokio::test]
async fn interrupted_merges_need_a_human_and_interrupted_pushes_are_repeated() {
    let r = rig("recipes", true).await;
    register_recipes(&r.e.services);
    let ops = OperationLog::new(&r.e.services);
    ops.begin(
        &vela_domain::ids::OperationId::new("op-m").unwrap(),
        Some(&r.req.run),
        "merge",
        &serde_json::json!({}),
    )
    .unwrap();
    ops.begin(
        &vela_domain::ids::OperationId::new("op-p").unwrap(),
        Some(&r.req.run),
        "push",
        &serde_json::json!({}),
    )
    .unwrap();
    let reports = ops.reconcile_all().await.unwrap();
    let by = |id: &str| reports.iter().find(|x| x.id == id).unwrap().outcome.clone();
    assert!(matches!(by("op-m"), Reconciled::NeedsHuman(_)));
    assert_eq!(by("op-p"), Reconciled::NotStarted);
}

#[test]
fn the_lane_fault_points_are_registered() {
    for n in [
        "lane.before_merge",
        "lane.after_merge",
        "lane.after_validation",
        "lane.after_discard",
    ] {
        assert!(FaultPoint::from_name(n).is_some(), "{n}");
    }
}
