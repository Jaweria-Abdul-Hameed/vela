use std::sync::Mutex;

use vela_domain::ids::WorkerId;
use vela_domain::ports::{PortFuture, PullRequestPort, PullRequestRequest, ready};
use vela_domain::state::{IntegrationHealth, WorkerPhase};
use vela_persistence::repositories::o01_workers;
use vela_persistence::repositories::s08_build;
use vela_testkit::fakes::{FakeGit, FakeProcessRunner, Scripted};

use super::*;
use crate::orchestrator::test_support::{TestEnv, env};
use crate::trust::TrustService;

#[derive(Default)]
struct Prs(Mutex<Vec<String>>);

impl PullRequestPort for Prs {
    fn create_draft<'a>(&'a self, r: &'a PullRequestRequest) -> PortFuture<'a, String> {
        self.0.lock().unwrap().push(format!("draft:{}", r.branch));
        ready(Ok("https://github.com/acme/app/pull/9".to_owned()))
    }
    fn mark_ready<'a>(&'a self, url: &'a str) -> PortFuture<'a, ()> {
        self.0.lock().unwrap().push(format!("ready:{url}"));
        ready(Ok(()))
    }
    fn merge<'a>(&'a self, url: &'a str) -> PortFuture<'a, ()> {
        self.0.lock().unwrap().push(format!("merge:{url}"));
        ready(Ok(()))
    }
}

struct Rig {
    e: TestEnv,
    git: Arc<FakeGit>,
    runner: Arc<FakeProcessRunner>,
    prs: Arc<Prs>,
    fin: Finalizer,
    input: FinalInput,
}

async fn rig(tag: &str, remote: bool, workers_done: usize) -> Rig {
    let e = env(tag);
    let run = e.create_run().await;
    e.drive_run(&run, RunPhase::Running).await;
    let runs = Runs::new(&e.services);
    for n in 0..2usize {
        let w = WorkerId::new(format!("w-{n}")).unwrap();
        let t = vela_domain::ids::TicketId::new(format!("T-{n}")).unwrap();
        let path = format!("C:/wt/{n}");
        runs.create_worker(
            &run,
            w.clone(),
            &t,
            BranchName::new(format!("vela/run-1/t-{n}")).unwrap(),
            WorktreePath::new(path.clone()).unwrap(),
            CommitSha::new("a".repeat(40)).unwrap(),
            "default",
            IntegrationHealth::Healthy,
        )
        .await
        .unwrap();
        let state = if n < workers_done {
            WorkerState::Complete
        } else {
            WorkerState::Active(WorkerPhase::Implementing)
        };
        let json = serde_json::to_string(&state).unwrap();
        let wid = w.as_str().to_owned();
        e.services
            .store
            .write(move |tx| o01_workers::set_state(tx.conn(), &wid, &json, false, 1))
            .unwrap();
        e.services
            .store
            .write(move |tx| {
                s08_build::insert_worktree(
                    tx.conn(),
                    &path,
                    "run-1",
                    "WORKER",
                    "b",
                    &"a".repeat(40),
                    1,
                )
            })
            .unwrap();
    }
    e.services
        .store
        .write(|tx| {
            s08_build::insert_worktree(
                tx.conn(),
                "C:/wt/integration",
                "run-1",
                "INTEGRATION",
                "vela/run-1/integration",
                &"a".repeat(40),
                1,
            )
        })
        .unwrap();
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
    git.set_diff(vec!["src/lib.rs".into()]);
    let runner = Arc::new(FakeProcessRunner::new());
    let prs = Arc::new(Prs::default());
    let promotion = Arc::new(PromotionService::new(
        e.services.clone(),
        git.clone(),
        Some(prs.clone()),
    ));
    let fin = Finalizer::new(e.services.clone(), git.clone(), runner.clone(), promotion);
    let input = FinalInput {
        run,
        project_id: "p1".into(),
        repo: WorktreePath::new("C:/repo").unwrap(),
        integration: WorktreePath::new("C:/wt/integration").unwrap(),
        integration_branch: BranchName::new("vela/run-1/integration").unwrap(),
        base_branch: BranchName::new("main").unwrap(),
        base_head: CommitSha::new("a".repeat(40)).unwrap(),
        validation: vec![ProfileCommand {
            text: "cargo test".into(),
            cwd: None,
        }],
        remote: remote.then(|| RemoteName::new("origin").unwrap()),
        title: "Run 1".into(),
        body: "body".into(),
        tickets_total: 2,
    };
    Rig {
        e,
        git,
        runner,
        prs,
        fin,
        input,
    }
}

#[tokio::test]
async fn a_finished_run_validates_pushes_opens_a_ready_pr_cleans_up_and_completes() {
    let r = rig("complete", true, 2).await;
    let FinalOutcome::Completed(summary) = r.fin.finish(&r.input).await.unwrap() else {
        panic!("expected completion")
    };
    assert_eq!((summary.tickets_done, summary.tickets_total), (2, 2));
    assert_eq!(summary.promotion.state, "READY");
    assert_eq!(
        summary.promotion.pr_url.as_deref(),
        Some("https://github.com/acme/app/pull/9")
    );
    assert_eq!(
        Runs::new(&r.e.services).run_state(&r.input.run).unwrap(),
        RunState::Completed
    );
    // Worker and integration worktrees are gone because the branch is safely on the remote.
    assert_eq!(summary.worktrees_removed.len(), 3);
    let live =
        r.e.services
            .store
            .read(|c| s08_build::list_worktrees(c, "run-1"))
            .unwrap();
    assert!(live.is_empty());
    let calls = r.git.log().calls();
    assert!(
        calls
            .iter()
            .any(|c| c == "push:origin:vela/run-1/integration")
    );
    assert!(
        !r.prs
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.starts_with("merge")),
        "never a silent merge"
    );
    let k: Vec<String> = r.e.raw_journal().into_iter().map(|(k, _)| k).collect();
    for needle in [
        "TestGateFinished",
        "PushCompleted",
        "PromotionCompleted",
        "RunCompleted",
    ] {
        assert!(k.iter().any(|x| x == needle), "{needle} missing in {k:?}");
    }
}

#[tokio::test]
async fn without_a_remote_the_run_completes_local_ready_and_keeps_the_integration_worktree() {
    let r = rig("local", false, 2).await;
    let FinalOutcome::Completed(summary) = r.fin.finish(&r.input).await.unwrap() else {
        panic!()
    };
    assert_eq!(summary.promotion.state, "LOCAL_READY");
    assert_eq!(
        summary.worktrees_removed.len(),
        2,
        "only the worker worktrees go"
    );
    let live =
        r.e.services
            .store
            .read(|c| s08_build::list_worktrees(c, "run-1"))
            .unwrap();
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].1, "INTEGRATION");
    assert!(!r.git.log().calls().iter().any(|c| c.starts_with("push:")));
}

#[tokio::test]
async fn a_failed_final_gate_is_never_success_and_publishes_nothing() {
    let r = rig("gate", true, 2).await;
    r.runner.push(Scripted::exit(1));
    let FinalOutcome::NeedsHuman { .. } = r.fin.finish(&r.input).await.unwrap() else {
        panic!("expected a human decision")
    };
    assert!(matches!(
        Runs::new(&r.e.services).run_state(&r.input.run).unwrap(),
        RunState::NeedsHuman {
            resume_state: RunPhase::Running,
            ..
        }
    ));
    assert!(!r.git.log().calls().iter().any(|c| c.starts_with("push:")));
    assert!(r.prs.0.lock().unwrap().is_empty());
    let live =
        r.e.services
            .store
            .read(|c| s08_build::list_worktrees(c, "run-1"))
            .unwrap();
    assert_eq!(live.len(), 3, "nothing is cleaned up while a human decides");
}

#[tokio::test]
async fn the_run_cannot_finish_while_tickets_are_not_done_or_integration_is_unhealthy() {
    let r = rig("early", true, 1).await;
    let err = r.fin.finish(&r.input).await.unwrap_err();
    assert!(err.what_failed().contains("1 of 2 tickets are done"));
    assert_eq!(
        Runs::new(&r.e.services).run_state(&r.input.run).unwrap(),
        RunState::Active(RunPhase::Running)
    );

    let r = rig("unhealthy", true, 2).await;
    r.e.services
        .store
        .write(|tx| s12_lane::set_health(tx.conn(), "run-1", IntegrationHealth::Unhealthy))
        .unwrap();
    assert!(
        r.fin.finish(&r.input).await.is_err(),
        "FINAL_REVIEW needs healthy integration"
    );
    assert_eq!(
        Runs::new(&r.e.services).run_state(&r.input.run).unwrap(),
        RunState::Active(RunPhase::Running)
    );
}

#[test]
fn the_final_fault_points_are_registered() {
    for n in [
        "final.before_push",
        "final.after_push",
        "final.after_promotion",
        "final.during_cleanup",
    ] {
        assert!(FaultPoint::from_name(n).is_some(), "{n}");
    }
}
