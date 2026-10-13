use vela_domain::ids::{BranchName, TicketId};
use vela_domain::ports::{GitContext, GitWorktrees, RepoTrust, WorktreeRequest};
use vela_domain::state::WorkerPhase;
use vela_persistence::repositories::s08_build::{self, RunBase};
use vela_testkit::fakes::{FakeGit, ScriptedAgent};

use super::*;
use crate::orchestrator::test_support::{TestEnv, env};
use crate::settings::Source;

struct Rig {
    e: TestEnv,
    run: RunId,
    git: Arc<FakeGit>,
    agent: Arc<ScriptedAgent>,
    recovery: Recovery,
    input: RecoveryInput,
}

fn base() -> CommitSha {
    CommitSha::new(format!("{:040x}", 1)).unwrap()
}

async fn rig(tag: &str, workers: &[(&str, WorkerPhase)]) -> Rig {
    let e = env(tag);
    let run = e.create_run().await;
    e.drive_run(&run, RunPhase::Running).await;
    let git = Arc::new(FakeGit::new());
    let agent = Arc::new(ScriptedAgent::new());
    let runs = Runs::new(&e.services);
    let ctx = GitContext {
        trust: RepoTrust::Trusted,
        operation: e.services.ids.new_operation_id(),
    };
    for (id, phase) in workers {
        let w = WorkerId::new(*id).unwrap();
        let branch = BranchName::new(format!("vela/run-1/{id}")).unwrap();
        let path = WorktreePath::new(format!("C:/wt/{id}")).unwrap();
        runs.create_worker(
            &run,
            w.clone(),
            &TicketId::new(format!("T-{id}")).unwrap(),
            branch.clone(),
            path.clone(),
            base(),
            "default",
            IntegrationHealth::Healthy,
        )
        .await
        .unwrap();
        for p in [WorkerPhase::ContextLoading, WorkerPhase::Implementing] {
            if *phase != WorkerPhase::Provisioning {
                runs.advance_worker(&run, &w, p, IntegrationHealth::Healthy)
                    .await
                    .unwrap();
            }
        }
        git.add_worktree(
            &ctx,
            &WorktreeRequest {
                repo: WorktreePath::new("C:/repo").unwrap(),
                path,
                branch: branch.clone(),
                base: base(),
            },
        )
        .await
        .unwrap();
        git.set_ref(branch.as_str(), base());
    }
    let recovery = Recovery::new(e.services.clone(), git.clone(), agent.clone());
    let input = RecoveryInput {
        run: run.clone(),
        project_id: "p1".into(),
        repo: WorktreePath::new("C:/repo").unwrap(),
        reason: "startup".into(),
    };
    Rig {
        e,
        run,
        git,
        agent,
        recovery,
        input,
    }
}

fn kinds(r: &Rig) -> Vec<String> {
    r.e.raw_journal().into_iter().map(|(k, _)| k).collect()
}

#[tokio::test]
async fn a_clean_run_resumes_what_is_safe_and_asks_before_continuing_by_default() {
    let r = rig(
        "clean",
        &[
            ("a", WorkerPhase::Implementing),
            ("b", WorkerPhase::Implementing),
        ],
    )
    .await;
    let report = r.recovery.reconcile(&r.input).await.unwrap();
    assert!(report.is_clean());
    assert_eq!(report.resumable.len(), 2);
    let k = kinds(&r);
    assert!(
        k.contains(&"ReconciliationStarted".to_owned())
            && k.contains(&"ReconciliationCompleted".to_owned())
    );
    // Default continuation is "ask": the run waits for confirmation even though nothing was wrong.
    assert!(matches!(
        Runs::new(&r.e.services).run_state(&r.run).unwrap(),
        RunState::Paused {
            reason: PausedReason::AwaitingRecoveryConfirmation,
            resume_state: RunPhase::Running
        }
    ));
}

#[tokio::test]
async fn auto_safe_continues_only_when_nothing_was_found() {
    let r = rig("auto", &[("a", WorkerPhase::Implementing)]).await;
    SettingsService::new(&r.e.services)
        .set_user(Source::User, "recovery_continuation", "auto_safe")
        .unwrap();
    let report = r.recovery.reconcile(&r.input).await.unwrap();
    assert!(report.is_clean());
    assert_eq!(
        Runs::new(&r.e.services).run_state(&r.run).unwrap(),
        RunState::Active(RunPhase::Running)
    );

    let r = rig("auto-dirty", &[("a", WorkerPhase::Implementing)]).await;
    SettingsService::new(&r.e.services)
        .set_user(Source::User, "recovery_continuation", "auto_safe")
        .unwrap();
    r.git
        .set_ref("vela/run-1/a", CommitSha::new("f".repeat(40)).unwrap());
    r.recovery.reconcile(&r.input).await.unwrap();
    assert!(matches!(
        Runs::new(&r.e.services).run_state(&r.run).unwrap(),
        RunState::Paused {
            reason: PausedReason::RecoveredAwaitingResume,
            ..
        }
    ));
}

#[tokio::test]
async fn a_branch_that_moved_is_a_divergence_never_a_guess() {
    let r = rig(
        "moved",
        &[
            ("a", WorkerPhase::Implementing),
            ("b", WorkerPhase::Implementing),
        ],
    )
    .await;
    r.git
        .set_ref("vela/run-1/a", CommitSha::new("f".repeat(40)).unwrap());
    let report = r.recovery.reconcile(&r.input).await.unwrap();
    assert_eq!(report.discrepancies.len(), 1);
    assert_eq!(report.discrepancies[0].class, DiscrepancyClass::RefMoved);
    assert_eq!(report.held.len(), 1);
    assert_eq!(
        report.resumable,
        [WorkerId::new("b").unwrap()],
        "the other worker is unaffected"
    );
    let a = WorkerId::new("a").unwrap();
    assert!(matches!(
        Runs::new(&r.e.services).worker_status(&a).unwrap().state(),
        WorkerState::NeedsHuman {
            kind: InterventionKind::ExternalDivergence,
            ..
        }
    ));
    assert!(kinds(&r).contains(&"DiscrepancyDetected".to_owned()));
}

#[tokio::test]
async fn a_missing_worktree_is_a_divergence() {
    let r = rig("no-worktree", &[("a", WorkerPhase::Implementing)]).await;
    let ctx = GitContext {
        trust: RepoTrust::Trusted,
        operation: r.e.services.ids.new_operation_id(),
    };
    r.git
        .remove_worktree(
            &ctx,
            &WorktreePath::new("C:/repo").unwrap(),
            &WorktreePath::new("C:/wt/a").unwrap(),
        )
        .await
        .unwrap();
    let report = r.recovery.reconcile(&r.input).await.unwrap();
    assert_eq!(
        report.discrepancies[0].class,
        DiscrepancyClass::WorktreeMissing
    );
    assert_eq!(report.held.len(), 1);
}

#[tokio::test]
async fn an_orphan_session_vela_created_is_cancelled() {
    let r = rig("orphan", &[("a", WorkerPhase::Implementing)]).await;
    r.agent
        .set_status(WorkerId::new("a").unwrap(), SessionStatus::Running);
    let report = r.recovery.reconcile(&r.input).await.unwrap();
    assert_eq!(report.orphans_cancelled, [WorkerId::new("a").unwrap()]);
    assert!(r.agent.log().calls().iter().any(|c| c == "cancel:a"));
    assert_eq!(
        report.resumable.len(),
        1,
        "otherwise safe, so it resumes after the cancel"
    );
    assert_eq!(
        report.discrepancies[0].class,
        DiscrepancyClass::OrphanSession
    );
}

#[tokio::test]
async fn human_gates_and_provider_blocks_are_restored_as_recorded_and_never_cleared() {
    let r = rig("held", &[("a", WorkerPhase::Implementing)]).await;
    let a = WorkerId::new("a").unwrap();
    crate::intervention::Interventions::new(&r.e.services)
        .raise_for_worker(
            &r.run,
            &a,
            InterventionKind::ProviderPolicyBlock,
            InterventionDetails {
                summary: "blocked".into(),
                evidence: vec![],
                preserved_work: "x".into(),
                last_safe_commit: None,
                actions: vec![],
                consequences: "x".into(),
            },
        )
        .await
        .unwrap();
    let before = Runs::new(&r.e.services).worker_status(&a).unwrap().state();
    let report = r.recovery.reconcile(&r.input).await.unwrap();
    assert_eq!(report.held.len(), 1);
    assert!(report.resumable.is_empty());
    assert_eq!(
        Runs::new(&r.e.services).worker_status(&a).unwrap().state(),
        before
    );
    assert_eq!(
        r.agent
            .log()
            .calls()
            .iter()
            .filter(|c| c.starts_with("turn:"))
            .count(),
        0,
        "no agent work starts"
    );
}

#[tokio::test]
async fn an_interrupted_merge_makes_integration_unknown_and_holds_the_run() {
    let r = rig("merge", &[("a", WorkerPhase::Implementing)]).await;
    let run = r.run.as_str().to_owned();
    r.e.services
        .store
        .write(move |tx| {
            s08_build::set_run_base(
                tx.conn(),
                &run,
                &RunBase {
                    base_sha: format!("{:040x}", 1),
                    base_branch: "main".into(),
                    integration_branch: "vela/run-1/integration".into(),
                    integration_worktree: "C:/wt/integration".into(),
                },
            )
        })
        .unwrap();
    OperationLog::new(&r.e.services)
        .begin(
            &vela_domain::ids::OperationId::new("op-m").unwrap(),
            Some(&r.run),
            "merge",
            &serde_json::json!({}),
        )
        .unwrap();
    let report = r.recovery.reconcile(&r.input).await.unwrap();
    assert!(
        report
            .discrepancies
            .iter()
            .any(|d| d.class == DiscrepancyClass::IntegrationUnverified)
    );
    let health =
        r.e.services
            .store
            .read(|c| s12_lane::get_health(c, r.run.as_str()))
            .unwrap();
    assert_eq!(health, IntegrationHealth::Unknown);
    assert!(matches!(
        Runs::new(&r.e.services).run_state(&r.run).unwrap(),
        RunState::NeedsHuman {
            kind: InterventionKind::IntegrationUnknown,
            ..
        }
    ));
    // Not retried or closed: it stays intended for a person.
    assert_eq!(
        OperationLog::new(&r.e.services)
            .get(&vela_domain::ids::OperationId::new("op-m").unwrap())
            .unwrap()
            .unwrap()
            .state,
        "INTENDED"
    );
}

#[tokio::test]
async fn paused_for_stop_all_is_resumable_and_capacity_pauses_are_left_alone() {
    let r = rig(
        "paused",
        &[
            ("a", WorkerPhase::Implementing),
            ("b", WorkerPhase::Implementing),
        ],
    )
    .await;
    let runs = Runs::new(&r.e.services);
    for (id, reason) in [("a", PausedReason::StopAll), ("b", PausedReason::Capacity)] {
        runs.transition_worker(
            &r.run,
            &WorkerId::new(id).unwrap(),
            WorkerState::Paused {
                reason,
                resume_state: WorkerPhase::Implementing,
            },
            Cause::System,
            IntegrationHealth::Healthy,
            vec![],
        )
        .await
        .unwrap();
    }
    let report = r.recovery.reconcile(&r.input).await.unwrap();
    assert_eq!(report.resumable, [WorkerId::new("a").unwrap()]);
    assert_eq!(report.held[0].worker, WorkerId::new("b").unwrap());
}
