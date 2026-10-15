use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use vela_domain::ports::{OutputChunk, OutputStream};
use vela_domain::state::{Cause, PausedReason, RunPhase, RunState};
use vela_persistence::repositories::s10_test_runs;
use vela_testkit::fakes::{FakeGit, FakeProcessRunner, Scenario, Scripted, ScriptedAgent};

use super::*;
use crate::orchestrator::test_support::{TestEnv, env};

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
            "FAILED" => CompletionStatus::Failed,
            _ => return Err(failure("no completion block")),
        };
        Ok(Completion {
            status,
            warnings: vec![],
            discovered_dependencies: vec![],
        })
    }
}

struct Rig {
    e: TestEnv,
    git: Arc<FakeGit>,
    agent: Arc<ScriptedAgent>,
    runner: Arc<FakeProcessRunner>,
    driver: Arc<WorkerDriver>,
    task: WorkerTask,
    dir: PathBuf,
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

async fn rig(tag: &str, trusted: bool, response: &str) -> Rig {
    static N: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "vela-worker-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let wt = dir.join("wt");
    std::fs::create_dir_all(&wt).unwrap();
    std::fs::create_dir_all(dir.join("repo")).unwrap();
    let wt_text = std::fs::canonicalize(&wt)
        .unwrap()
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('\\', "/");

    let e = env(tag);
    let run = e.create_run().await;
    e.drive_run(&run, RunPhase::Running).await;
    let worker = WorkerId::new("w-1").unwrap();
    let ticket = TicketId::new("T-1").unwrap();
    let branch = BranchName::new("vela/run-1/t-1").unwrap();
    let base = CommitSha::new("a".repeat(40)).unwrap();
    let worktree = WorktreePath::new(wt_text).unwrap();
    Runs::new(&e.services)
        .create_worker(
            &run,
            worker.clone(),
            &ticket,
            branch.clone(),
            worktree.clone(),
            base.clone(),
            "default",
            IntegrationHealth::Healthy,
        )
        .await
        .unwrap();
    if trusted {
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
    }
    let git = Arc::new(FakeGit::new());
    git.set_changed(vec!["src/lib.rs".into()]);
    let agent = Arc::new(ScriptedAgent::new());
    agent.set_response(worker.clone(), response);
    let runner = Arc::new(FakeProcessRunner::new());
    let driver = Arc::new(WorkerDriver::new(
        e.services.clone(),
        agent.clone(),
        runner.clone(),
        git.clone(),
        Arc::new(Prompts),
    ));
    let task = WorkerTask {
        run,
        worker,
        ticket,
        title: "Add login".into(),
        body: "body".into(),
        project_id: "p1".into(),
        repo: WorktreePath::new("C:/repo").unwrap(),
        repo_root: dir.join("repo"),
        worktree,
        branch,
        base,
        validation: vec![ProfileCommand {
            text: "cargo test".into(),
            cwd: None,
        }],
        max_fix_attempts: 1,
        spec_pointers: vec![],
    };
    Rig {
        e,
        git,
        agent,
        runner,
        driver,
        task,
        dir,
    }
}

fn state(r: &Rig) -> WorkerState {
    Runs::new(&r.e.services)
        .worker_status(&r.task.worker)
        .unwrap()
        .state()
}

#[tokio::test]
async fn a_ticket_goes_from_provisioning_to_ready_to_merge_with_evidence() {
    let r = rig("happy", true, "DONE").await;
    let out = r.driver.run(&r.task).await.unwrap();
    let WorkerOutcome::ReadyToMerge { head } = out else {
        panic!("expected ready to merge")
    };
    assert_eq!(state(&r), WorkerState::Active(WorkerPhase::ReadyToMerge));

    let i = inspect(&r.e.services, &r.task.run, &r.task.worker, &r.task.ticket).unwrap();
    assert_eq!(i.tests.len(), 1);
    assert!(i.tests[0].passed && i.tests[0].command == "cargo test");
    assert_eq!(i.conversation.as_deref(), Some("conv-1"));
    let kinds: Vec<_> = i.timeline.iter().map(|t| t.kind.as_str()).collect();
    for k in [
        "CommandStarted",
        "CommandFinished",
        "TestGateFinished",
        "CheckpointCommitted",
    ] {
        assert!(kinds.contains(&k), "{k} missing from {kinds:?}");
    }
    assert!(
        r.git
            .log()
            .calls()
            .iter()
            .any(|c| c.starts_with("checkpoint:") && c.contains("checkpoint: T-1"))
    );
    // The checkpoint is a recorded operation that was closed after the commit existed.
    let op =
        r.e.services
            .store
            .read(vela_persistence::repositories::r01_operations::list_intended)
            .unwrap();
    assert!(op.is_empty(), "no operation is left half-done");
    assert_eq!(head.as_str().len(), 40);
    // The timeline survives a restart.
    let reopened = r.e.restart();
    assert!(
        !timeline(&reopened, &r.task.run, &r.task.ticket)
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn failing_validation_asks_the_agent_to_fix_then_passes() {
    let r = rig("fix", true, "DONE").await;
    r.runner.push(Scripted::exit(1).with_output(OutputChunk {
        stream: OutputStream::Stderr,
        text: "assertion failed: x\n".into(),
    }));
    let out = r.driver.run(&r.task).await.unwrap();
    assert!(matches!(out, WorkerOutcome::ReadyToMerge { .. }));
    let tests =
        r.e.services
            .store
            .read(|c| s10_test_runs::list_for_worker(c, "w-1"))
            .unwrap();
    assert_eq!(
        tests.iter().map(|t| t.passed).collect::<Vec<_>>(),
        [false, true]
    );
    assert!(tests[0].output_tail.contains("assertion failed"));
    assert_eq!(
        r.agent
            .log()
            .calls()
            .iter()
            .filter(|c| c.starts_with("turn:"))
            .count(),
        2,
        "one implement turn, one fix turn"
    );
}

#[tokio::test]
async fn validation_that_keeps_failing_fails_the_worker_with_the_output() {
    let r = rig("stuck", true, "DONE").await;
    for _ in 0..3 {
        r.runner.push(Scripted::exit(1).with_output(OutputChunk {
            stream: OutputStream::Stderr,
            text: "still broken\n".into(),
        }));
    }
    let err = r.driver.run(&r.task).await.unwrap_err();
    assert_eq!(err.class(), ErrorClass::ValidationFailure);
    assert!(
        err.what_failed().contains("still broken") && err.what_failed().contains("1 fix attempts")
    );
    assert!(
        !r.git
            .log()
            .calls()
            .iter()
            .any(|c| c.starts_with("checkpoint:")),
        "nothing is checkpointed on a failing gate"
    );
}

#[tokio::test]
async fn a_blocked_worker_raises_ambiguity_and_keeps_its_work() {
    let r = rig("blocked", true, "BLOCKED").await;
    let out = r.driver.run(&r.task).await.unwrap();
    assert!(matches!(out, WorkerOutcome::NeedsHuman { .. }));
    assert!(matches!(
        state(&r),
        WorkerState::NeedsHuman {
            kind: InterventionKind::Ambiguity,
            resume_state: WorkerPhase::Implementing
        }
    ));
}

#[tokio::test]
async fn a_provider_policy_block_is_a_held_human_decision_not_a_failure() {
    let r = rig("policy-block", true, "DONE").await;
    r.agent
        .script(r.task.worker.clone(), Scenario::ProviderPolicyBlock);
    let out = r.driver.run(&r.task).await.unwrap();
    assert!(matches!(out, WorkerOutcome::NeedsHuman { .. }));
    assert!(matches!(
        state(&r),
        WorkerState::NeedsHuman {
            kind: InterventionKind::ProviderPolicyBlock,
            ..
        }
    ));
}

#[tokio::test]
async fn capacity_failure_and_an_invalid_completion_fail_the_worker() {
    let r = rig("capacity", true, "DONE").await;
    r.agent.script(r.task.worker.clone(), Scenario::Capacity);
    // The provider ran out of capacity: this worker pauses where it was and the profile cools down; nothing fails.
    let WorkerOutcome::Paused { until_ms } = r.driver.run(&r.task).await.unwrap() else {
        panic!("expected a capacity pause")
    };
    assert!(until_ms > 0);
    assert!(matches!(
        state(&r),
        WorkerState::Paused {
            reason: PausedReason::Capacity,
            ..
        }
    ));
    let profile = crate::capacity::CapacityService::new(r.e.services.clone())
        .profile(crate::scheduler::DEFAULT_PROFILE)
        .unwrap();
    assert_eq!(profile.state, "COOLDOWN");

    let r = rig("garbage", true, "I think I am done").await;
    let err = r.driver.run(&r.task).await.unwrap_err();
    assert_eq!(err.class(), ErrorClass::AgentFailure);
    assert!(
        err.what_failed()
            .contains("stayed invalid after one correction")
    );
    assert_eq!(
        r.agent
            .log()
            .calls()
            .iter()
            .filter(|c| c.starts_with("turn:"))
            .count(),
        2,
        "exactly one correction turn"
    );
}

#[tokio::test]
async fn a_worker_that_changed_nothing_is_not_checkpointed() {
    let r = rig("clean", true, "DONE").await;
    r.git.set_changed(vec![]);
    let err = r.driver.run(&r.task).await.unwrap_err();
    assert!(err.what_failed().contains("changed nothing"));
}

#[tokio::test]
async fn an_untrusted_repository_never_gets_a_worker_started() {
    let r = rig("untrusted", false, "DONE").await;
    let err = r.driver.run(&r.task).await.unwrap_err();
    assert_eq!(err.class(), ErrorClass::PolicyBlock);
    assert!(!r.agent.log().calls().iter().any(|c| c.starts_with("turn:")));
}

#[tokio::test]
async fn a_validation_command_outside_the_confirmed_profile_is_refused() {
    let mut r = rig("outside-profile", true, "DONE").await;
    r.task.validation = vec![ProfileCommand {
        text: "cargo publish".into(),
        cwd: None,
    }];
    let err = r.driver.run(&r.task).await.unwrap_err();
    assert_eq!(err.class(), ErrorClass::PolicyBlock);
    assert!(
        r.runner.log().calls().is_empty()
            || !r.runner.log().calls().iter().any(|c| c.contains("publish"))
    );
}

#[tokio::test]
async fn the_supervisor_fails_a_worker_whose_run_returns_an_error() {
    let r = rig("supervised", true, "FAILED").await;
    let task = r.task.clone();
    let d = r.driver.clone();
    crate::orchestrator::supervisor::supervise(
        r.e.services.clone(),
        task.run.clone(),
        task.worker.clone(),
        async move { d.run(&task).await.map(|_| ()) },
    )
    .await
    .unwrap();
    assert_eq!(
        Runs::new(&r.e.services)
            .worker_status(&r.task.worker)
            .unwrap()
            .state(),
        WorkerState::Failed
    );
    let _ = (Cause::System, RunState::Cancelled);
}

async fn advance_to(r: &Rig, phases: &[WorkerPhase]) {
    for p in phases {
        Runs::new(&r.e.services)
            .advance_worker(&r.task.run, &r.task.worker, *p, IntegrationHealth::Healthy)
            .await
            .unwrap();
    }
}

fn checkpoint() -> CommitSha {
    CommitSha::new("c".repeat(40)).unwrap()
}

fn turns(r: &Rig) -> usize {
    r.agent
        .log()
        .calls()
        .iter()
        .filter(|c| c.starts_with("turn:"))
        .count()
}

#[tokio::test]
async fn recovery_resume_starts_a_new_session_and_finishes_from_an_early_phase() {
    let r = rig("resume-early", true, "DONE").await;
    advance_to(&r, &[WorkerPhase::ContextLoading]).await;
    let out = r
        .driver
        .resume(&r.task, None, &["context loaded".to_owned()])
        .await
        .unwrap();
    assert!(matches!(out, WorkerOutcome::ReadyToMerge { .. }));
    assert_eq!(turns(&r), 1, "one fresh session");
    assert_eq!(state(&r), WorkerState::Active(WorkerPhase::ReadyToMerge));
    assert!(
        r.git
            .log()
            .calls()
            .iter()
            .any(|c| c.starts_with("add_worktree:")),
        "a missing worktree is recreated"
    );
}

#[tokio::test]
async fn recovery_resume_after_validation_only_checkpoints_and_never_asks_the_agent_again() {
    let r = rig("resume-checkpoint", true, "DONE").await;
    advance_to(
        &r,
        &[
            WorkerPhase::ContextLoading,
            WorkerPhase::Implementing,
            WorkerPhase::FocusedValidation,
        ],
    )
    .await;
    let out = r
        .driver
        .resume(&r.task, Some(&checkpoint()), &[])
        .await
        .unwrap();
    assert!(matches!(out, WorkerOutcome::ReadyToMerge { .. }));
    assert_eq!(turns(&r), 0);
    assert!(
        r.git
            .log()
            .calls()
            .iter()
            .any(|c| c.starts_with("checkpoint:"))
    );
}

#[tokio::test]
async fn recovery_resume_of_a_reviewed_worker_just_reports_its_checkpoint() {
    let r = rig("resume-ready", true, "DONE").await;
    advance_to(
        &r,
        &[
            WorkerPhase::ContextLoading,
            WorkerPhase::Implementing,
            WorkerPhase::FocusedValidation,
            WorkerPhase::Checkpointing,
            WorkerPhase::Reviewing,
        ],
    )
    .await;
    let out = r
        .driver
        .resume(&r.task, Some(&checkpoint()), &[])
        .await
        .unwrap();
    assert_eq!(out, WorkerOutcome::ReadyToMerge { head: checkpoint() });
    assert_eq!(state(&r), WorkerState::Active(WorkerPhase::ReadyToMerge));
    // Without a verified checkpoint there is nothing to continue from.
    let r2 = rig("resume-no-checkpoint", true, "DONE").await;
    advance_to(
        &r2,
        &[
            WorkerPhase::ContextLoading,
            WorkerPhase::Implementing,
            WorkerPhase::FocusedValidation,
            WorkerPhase::Checkpointing,
            WorkerPhase::Reviewing,
            WorkerPhase::ReadyToMerge,
        ],
    )
    .await;
    assert!(r2.driver.resume(&r2.task, None, &[]).await.is_err());
}

#[tokio::test]
async fn a_worker_in_the_merge_lane_is_not_resumed_by_a_new_session() {
    let r = rig("resume-lane", true, "DONE").await;
    advance_to(
        &r,
        &[
            WorkerPhase::ContextLoading,
            WorkerPhase::Implementing,
            WorkerPhase::FocusedValidation,
            WorkerPhase::Checkpointing,
            WorkerPhase::Reviewing,
            WorkerPhase::ReadyToMerge,
            WorkerPhase::Merging,
        ],
    )
    .await;
    let err = r
        .driver
        .resume(&r.task, Some(&checkpoint()), &[])
        .await
        .unwrap_err();
    assert!(err.what_failed().contains("merge lane"));
    assert_eq!(turns(&r), 0);
}

struct Blocked;

impl crate::scheduler::ProfileAvailability for Blocked {
    fn is_available(&self, _p: &str) -> bool {
        false
    }
}

#[tokio::test]
async fn recovery_resume_never_starts_a_session_on_a_blocked_profile() {
    let r = rig("resume-blocked", true, "DONE").await;
    r.e.services
        .register(Arc::new(crate::scheduler::ProfileAvailabilityService(
            Arc::new(Blocked),
        )));
    advance_to(&r, &[WorkerPhase::ContextLoading]).await;
    assert!(r.driver.resume(&r.task, None, &[]).await.is_err());
    assert_eq!(turns(&r), 0);
    assert_eq!(
        state(&r),
        WorkerState::Active(WorkerPhase::ContextLoading),
        "nothing moved"
    );
}

struct FakeAsked {
    asked: Mutex<Vec<String>>,
}

impl AskedCommands for FakeAsked {
    fn take_asked(&self, _worker: &WorkerId) -> Vec<String> {
        std::mem::take(&mut *self.asked.lock().unwrap())
    }
    fn grant_once(&self, _w: &WorkerId, _e: &str, _now: i64) -> Result<(), VelaError> {
        Ok(())
    }
}

fn with_asked(r: &Rig, executables: &[&str]) -> Arc<FakeAsked> {
    let fake = Arc::new(FakeAsked {
        asked: Mutex::new(executables.iter().map(|e| (*e).to_owned()).collect()),
    });
    r.e.services
        .register(Arc::new(AskedCommandsService(fake.clone())));
    fake
}

#[tokio::test]
async fn a_worker_that_cannot_go_on_because_a_command_was_asked_about_raises_an_approval_ask() {
    let r = rig("ask", true, "BLOCKED").await;
    with_asked(&r, &["npm", "npm", "cargo"]);
    let out = r.driver.run(&r.task).await.unwrap();
    assert!(matches!(out, WorkerOutcome::NeedsHuman { .. }));
    assert!(matches!(
        state(&r),
        WorkerState::NeedsHuman {
            kind: InterventionKind::ApprovalAsk,
            resume_state: WorkerPhase::Implementing
        }
    ));
    let open = crate::intervention::Interventions::new(&r.e.services)
        .open_for_run(&r.task.run)
        .unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].kind, "APPROVAL_ASK");
    let evidence: Vec<String> = serde_json::from_str(&open[0].evidence).unwrap();
    assert_eq!(
        evidence,
        ["cargo", "npm"],
        "each executable once, from the hook's record"
    );
}

#[tokio::test]
async fn a_command_that_was_asked_about_but_did_not_stop_the_worker_is_not_a_question() {
    let r = rig("ask-ignored", true, "DONE").await;
    let fake = with_asked(&r, &["npm"]);
    let out = r.driver.run(&r.task).await.unwrap();
    assert!(matches!(out, WorkerOutcome::ReadyToMerge { .. }));
    assert!(
        fake.asked.lock().unwrap().is_empty(),
        "the stale ask was dropped"
    );
}

#[tokio::test]
async fn a_resumed_session_consumes_what_the_person_approved() {
    let r = rig("ask-resume", true, "DONE").await;
    advance_to(
        &r,
        &[WorkerPhase::ContextLoading, WorkerPhase::Implementing],
    )
    .await;
    let notes = Arc::new(ApprovedNotes::new());
    r.e.services.register(notes.clone());
    notes.add(&r.task.worker, "npm");
    let out = r.driver.resume(&r.task, None, &[]).await.unwrap();
    assert!(matches!(out, WorkerOutcome::ReadyToMerge { .. }));
    assert!(
        notes.take(&r.task.worker).is_empty(),
        "the note was told to the new session"
    );
}
