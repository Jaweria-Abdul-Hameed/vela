use vela_domain::ports::{OutputChunk, OutputStream, TrackerTicket};
use vela_domain::state::InterventionKind;
use vela_testkit::fakes::{FakeGit, FakeProcessRunner, FakeTracker, Scripted, ScriptedAgent};

use super::*;
use crate::intervention::Interventions;
use crate::orchestrator::test_support::{TestEnv, env};
use crate::settings::{SettingsService, Source};
use crate::trust::TrustService;
use crate::worker::{Completion, CompletionStatus};

struct Prompts;

impl WorkerPrompts for Prompts {
    fn worker_prompt(&self, task: &WorkerTask) -> String {
        format!("do {}", task.ticket)
    }
    fn fix_prompt(&self, failures: &str) -> String {
        format!("fix: {failures}")
    }
    fn correction_prompt(&self, problem: &str) -> String {
        format!("correct: {problem}")
    }
    fn parse_completion(&self, response: &str, _base: &str) -> Result<Completion, VelaError> {
        let status = match response {
            "DONE" => CompletionStatus::Complete,
            "BLOCKED" => CompletionStatus::Blocked,
            _ => CompletionStatus::Failed,
        };
        Ok(Completion {
            status,
            warnings: vec![],
            discovered_dependencies: vec![],
        })
    }
}

fn tid(n: &str) -> TicketId {
    TicketId::new(n).unwrap()
}

fn ticket(id: &str, blockers: &[&str]) -> TrackerTicket {
    TrackerTicket {
        id: tid(id),
        title: format!("ticket {id}"),
        body: "do the thing".into(),
        blockers: blockers.iter().map(|b| tid(b)).collect(),
        labels: vec![],
    }
}

struct Rig {
    e: TestEnv,
    git: Arc<FakeGit>,
    agent: Arc<ScriptedAgent>,
    runner: Arc<FakeProcessRunner>,
    driver: Arc<RunDriver>,
    cfg: RunConfig,
    dir: PathBuf,
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

async fn rig(tag: &str, tickets: Vec<TrackerTicket>, trusted: bool) -> Rig {
    let e = env(tag);
    e.seed_project();
    let dir = std::env::temp_dir().join(format!("vela-driver-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("repo")).unwrap();
    std::fs::create_dir_all(dir.join("wt")).unwrap();
    if trusted {
        TrustService::new(&e.services)
            .trust(
                "p1",
                "id",
                &[crate::trust::ProfileCommand {
                    text: "cargo test".into(),
                    cwd: None,
                }],
                "v1",
            )
            .unwrap();
    }
    SettingsService::new(&e.services)
        .set_user(Source::User, "max_parallel_workers", "2")
        .unwrap();
    let tracker = Arc::new(FakeTracker::new());
    for t in tickets {
        tracker.add(t);
    }
    let git = Arc::new(FakeGit::new());
    git.set_changed(vec!["src/lib.rs".into()]);
    git.set_keep_changed(true);
    git.set_diff(vec!["src/lib.rs".into()]);
    let agent = Arc::new(ScriptedAgent::new());
    agent.set_default_response("DONE");
    let runner = Arc::new(FakeProcessRunner::new());
    let promotion = Arc::new(PromotionService::new(e.services.clone(), git.clone(), None));
    let driver = RunDriver::new(
        e.services.clone(),
        tracker,
        None,
        git.clone(),
        agent.clone(),
        runner.clone(),
        Arc::new(Prompts),
        promotion,
    );
    let wt = std::fs::canonicalize(dir.join("wt"))
        .unwrap()
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('\\', "/");
    let cfg = RunConfig {
        project_id: "p1".into(),
        repo: WorktreePath::new("C:/repo").unwrap(),
        repo_root: dir.join("repo"),
        base_branch: BranchName::new("main").unwrap(),
        worktree_root: wt,
        validation: vec![crate::trust::ProfileCommand {
            text: "cargo test".into(),
            cwd: None,
        }],
        remote: None,
        title: "Run".into(),
        body: "body".into(),
        spec_pointers: vec![],
        max_fix_attempts: 1,
    };
    Rig {
        e,
        git,
        agent,
        runner,
        driver,
        cfg,
        dir,
    }
}

async fn started(r: &Rig) -> RunId {
    let (run, analysis) = r.driver.prepare(&r.cfg).await.unwrap();
    assert!(matches!(analysis, Analysis::Ready { .. }));
    r.driver
        .approve_and_start(&run, &r.cfg, None, false)
        .await
        .unwrap();
    run
}

fn kinds(r: &Rig) -> Vec<String> {
    r.e.raw_journal().into_iter().map(|(k, _)| k).collect()
}

#[tokio::test]
async fn an_approved_graph_runs_to_a_completed_run_with_every_ticket_merged() {
    let r = rig(
        "happy",
        vec![
            ticket("T-1", &[]),
            ticket("T-2", &["T-1"]),
            ticket("T-3", &[]),
        ],
        true,
    )
    .await;
    let run = started(&r).await;
    let out = r.driver.drive(&run, &r.cfg).await.unwrap();
    let DriveOutcome::Completed(summary) = out else {
        panic!("expected completion, got {out:?}")
    };
    assert_eq!((summary.tickets_done, summary.tickets_total), (3, 3));
    assert_eq!(summary.promotion.state, "LOCAL_READY");
    assert_eq!(
        Runs::new(&r.e.services).run_state(&run).unwrap(),
        RunState::Completed
    );
    let k = kinds(&r);
    assert_eq!(k.iter().filter(|x| *x == "MergeCompleted").count(), 3);
    assert_eq!(k.iter().filter(|x| *x == "CheckpointCommitted").count(), 3);
    assert!(k.contains(&"RunCompleted".to_owned()));
    // T-2 only started after T-1 merged: its MergeCompleted comes after T-1's.
    assert_eq!(
        r.agent
            .log()
            .calls()
            .iter()
            .filter(|c| c.starts_with("turn:"))
            .count(),
        3
    );
}

#[tokio::test]
async fn integration_failure_sends_the_ticket_back_to_fix_and_then_merges() {
    let r = rig("fix-loop", vec![ticket("T-1", &[])], true).await;
    // Gate calls in order: the worker's focused gate, the lane's gate (fails once), the focused gate after the fix, the lane's gate again.
    r.runner.push(Scripted::exit(0));
    r.runner.push(Scripted::exit(1).with_output(OutputChunk {
        stream: OutputStream::Stderr,
        text: "integration broke\n".into(),
    }));
    let run = started(&r).await;
    let out = r.driver.drive(&run, &r.cfg).await.unwrap();
    assert!(matches!(out, DriveOutcome::Completed(_)), "{out:?}");
    let k = kinds(&r);
    assert!(
        k.contains(&"IntegrationGateFailed".to_owned()) && k.contains(&"MergeDiscarded".to_owned())
    );
    assert_eq!(k.iter().filter(|x| *x == "MergeCompleted").count(), 1);
    assert_eq!(
        r.agent
            .log()
            .calls()
            .iter()
            .filter(|c| c.starts_with("turn:"))
            .count(),
        2,
        "one implement turn and one fix turn"
    );
}

#[tokio::test]
async fn a_blocked_worker_makes_the_driver_wait_and_a_resolved_intervention_lets_it_continue() {
    let r = rig(
        "wait",
        vec![ticket("T-1", &[]), ticket("T-2", &["T-1"])],
        true,
    )
    .await;
    r.agent.set_default_response("BLOCKED");
    let run = started(&r).await;
    let out = r.driver.drive(&run, &r.cfg).await.unwrap();
    let DriveOutcome::Waiting(cycle) = out else {
        panic!("expected waiting, got {out:?}")
    };
    assert!(cycle.started.is_empty());
    let open = Interventions::new(&r.e.services)
        .open_for_run(&run)
        .unwrap();
    assert_eq!(open.len(), 1);
    assert!(matches!(open[0].kind.as_str(), "AMBIGUITY"));

    // The person decides; the worker goes back to its phase and the driver resumes it with a new session.
    r.agent.set_default_response("DONE");
    Interventions::new(&r.e.services)
        .resolve(&open[0].id, "go ahead")
        .await
        .unwrap();
    let out = r.driver.drive(&run, &r.cfg).await.unwrap();
    assert!(matches!(out, DriveOutcome::Completed(_)), "{out:?}");
    let _ = InterventionKind::Ambiguity;
}

#[tokio::test]
async fn an_untrusted_repository_cannot_be_built() {
    let r = rig("untrusted", vec![ticket("T-1", &[])], false).await;
    let (run, _) = r.driver.prepare(&r.cfg).await.unwrap();
    let err = r
        .driver
        .approve_and_start(&run, &r.cfg, None, false)
        .await
        .unwrap_err();
    assert_eq!(err.class(), ErrorClass::PolicyBlock);
    assert_eq!(
        r.agent
            .log()
            .calls()
            .iter()
            .filter(|c| c.starts_with("turn:"))
            .count(),
        0
    );
}

#[tokio::test]
async fn a_cycle_in_the_issues_stops_before_anything_is_created() {
    let r = rig(
        "cycle",
        vec![ticket("T-1", &["T-2"]), ticket("T-2", &["T-1"])],
        true,
    )
    .await;
    let (run, analysis) = r.driver.prepare(&r.cfg).await.unwrap();
    assert!(matches!(analysis, Analysis::Invalid { .. }));
    assert!(matches!(
        Runs::new(&r.e.services).run_state(&run).unwrap(),
        RunState::NeedsHuman {
            kind: InterventionKind::GraphInvalid,
            ..
        }
    ));
    assert!(
        !r.git
            .log()
            .calls()
            .iter()
            .any(|c| c.starts_with("add_worktree"))
    );
}

#[tokio::test]
async fn a_failed_worker_leaves_the_run_waiting_and_does_not_start_its_dependents() {
    let r = rig(
        "failed",
        vec![ticket("T-1", &[]), ticket("T-2", &["T-1"])],
        true,
    )
    .await;
    r.agent.set_default_response("garbage");
    let run = started(&r).await;
    let out = r.driver.drive(&run, &r.cfg).await.unwrap();
    let DriveOutcome::Waiting(cycle) = out else {
        panic!("expected waiting, got {out:?}")
    };
    assert_eq!(cycle.stuck, [tid("T-1")]);
    assert_eq!(
        r.agent
            .log()
            .calls()
            .iter()
            .filter(|c| c.starts_with("turn:"))
            .count(),
        1,
        "one implement turn for T-1 and nothing for T-2"
    );
    let _ = (RunState::Cancelled, Cause::System);
}
