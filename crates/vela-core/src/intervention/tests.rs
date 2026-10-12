use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use vela_domain::ids::{BranchName, CommitSha, TicketId, WorkerId, WorktreePath};
use vela_domain::state::{
    Cause, IntegrationHealth, PausedReason, RunPhase, RunState, WorkerPhase, WorkerState,
};

use super::*;
use crate::orchestrator::runs::Runs;
use crate::orchestrator::test_support::env;

fn details(summary: &str) -> InterventionDetails {
    InterventionDetails {
        summary: summary.to_owned(),
        evidence: vec!["the graph has a cycle A -> B -> A".to_owned()],
        preserved_work: "nothing was started".to_owned(),
        last_safe_commit: Some(CommitSha::new("a".repeat(40)).unwrap()),
        actions: vec!["edit the issues".to_owned(), "cancel the run".to_owned()],
        consequences: "the run waits".to_owned(),
    }
}

async fn run_in_analysis(e: &crate::orchestrator::test_support::TestEnv) -> RunId {
    let run = e.create_run().await;
    let runs = Runs::new(&e.services);
    for phase in [RunPhase::Preflight, RunPhase::Analyzing] {
        runs.transition_run(
            &run,
            RunState::Active(phase),
            Cause::System,
            IntegrationHealth::Healthy,
            Vec::new(),
        )
        .await
        .unwrap();
    }
    run
}

#[tokio::test]
async fn a_run_level_intervention_is_raised_persisted_journaled_and_resolved_to_the_recorded_state()
{
    let e = env("run-level");
    let run = run_in_analysis(&e).await;
    let iv = Interventions::new(&e.services);
    let id = iv
        .raise_for_run(&run, InterventionKind::GraphInvalid, details("cycle"))
        .await
        .unwrap();

    let state = Runs::new(&e.services).run_state(&run).unwrap();
    assert_eq!(
        state,
        RunState::NeedsHuman {
            kind: InterventionKind::GraphInvalid,
            resume_state: RunPhase::Analyzing
        }
    );
    let record = iv.get(&id).unwrap().unwrap();
    assert_eq!(record.state, "OPEN");
    assert_eq!(record.kind, "GRAPH_INVALID");
    assert_eq!(
        record.last_safe_commit.as_deref(),
        Some("a".repeat(40).as_str())
    );
    assert_eq!(iv.open_for_run(&run).unwrap().len(), 1);
    assert!(
        e.raw_journal()
            .iter()
            .any(|(k, p)| k == "HumanActionRequired" && p.contains("GRAPH_INVALID"))
    );

    iv.resolve(&id, "edited the issues").await.unwrap();
    assert_eq!(
        Runs::new(&e.services).run_state(&run).unwrap(),
        RunState::Active(RunPhase::Analyzing),
        "back to the recorded state"
    );
    let record = iv.get(&id).unwrap().unwrap();
    assert_eq!(
        (record.state.as_str(), record.resolution.as_deref()),
        ("RESOLVED", Some("edited the issues"))
    );
    assert!(iv.open_for_run(&run).unwrap().is_empty());
    assert!(e.raw_journal().iter().any(|(k, _)| k == "RunResumed"));
    // Resolving twice is refused.
    assert!(iv.resolve(&id, "again").await.is_err());
}

async fn worker_in_implementing(
    e: &crate::orchestrator::test_support::TestEnv,
) -> (RunId, WorkerId) {
    let run = e.create_run().await;
    let runs = Runs::new(&e.services);
    let w = WorkerId::new("w-1").unwrap();
    runs.create_worker(
        &run,
        w.clone(),
        &TicketId::new("T-1").unwrap(),
        BranchName::new("vela/run-1/t1").unwrap(),
        WorktreePath::new("C:/wt/t1").unwrap(),
        CommitSha::new("a".repeat(40)).unwrap(),
        "default",
        IntegrationHealth::Healthy,
    )
    .await
    .unwrap();
    for phase in [WorkerPhase::ContextLoading, WorkerPhase::Implementing] {
        runs.advance_worker(&run, &w, phase, IntegrationHealth::Healthy)
            .await
            .unwrap();
    }
    (run, w)
}

#[tokio::test]
async fn a_worker_level_intervention_returns_the_worker_to_its_resume_state() {
    let e = env("worker-level");
    let (run, w) = worker_in_implementing(&e).await;
    let iv = Interventions::new(&e.services);
    let id = iv
        .raise_for_worker(
            &run,
            &w,
            InterventionKind::Ambiguity,
            details("the ticket contradicts itself"),
        )
        .await
        .unwrap();
    assert_eq!(
        Runs::new(&e.services).worker_status(&w).unwrap().state(),
        WorkerState::NeedsHuman {
            kind: InterventionKind::Ambiguity,
            resume_state: WorkerPhase::Implementing
        }
    );
    iv.resolve(&id, "clarified").await.unwrap();
    assert_eq!(
        Runs::new(&e.services).worker_status(&w).unwrap().state(),
        WorkerState::Active(WorkerPhase::Implementing)
    );
    assert!(e.raw_journal().iter().any(|(k, _)| k == "WorkerResumed"));
}

#[tokio::test]
async fn kinds_are_checked_against_their_level_and_the_state() {
    let e = env("levels");
    let (run, w) = worker_in_implementing(&e).await;
    let iv = Interventions::new(&e.services);
    // A run-level kind on a worker is refused.
    assert!(
        iv.raise_for_worker(&run, &w, InterventionKind::GraphInvalid, details("x"))
            .await
            .is_err()
    );
    // A run that is not active cannot get one.
    let runs = Runs::new(&e.services);
    e.drive_run(&run, RunPhase::Running).await;
    runs.transition_run(
        &run,
        RunState::Paused {
            reason: PausedReason::Manual,
            resume_state: RunPhase::Running,
        },
        Cause::UserDecision,
        IntegrationHealth::Healthy,
        Vec::new(),
    )
    .await
    .unwrap();
    assert!(
        iv.raise_for_run(&run, InterventionKind::GraphInvalid, details("x"))
            .await
            .is_err()
    );
}

struct Flag(Arc<AtomicBool>);

impl ResolutionPrecondition for Flag {
    fn check(&self, _i: &Intervention) -> Result<(), String> {
        if self.0.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err("the provider has not cleared the block".to_owned())
        }
    }
}

#[tokio::test]
async fn a_kind_with_a_registered_precondition_cannot_be_resolved_until_it_holds() {
    let e = env("precondition");
    let (run, w) = worker_in_implementing(&e).await;
    let iv = Interventions::new(&e.services);
    let cleared = Arc::new(AtomicBool::new(false));
    iv.registry.register(
        InterventionKind::ProviderPolicyBlock,
        Arc::new(Flag(cleared.clone())),
    );
    let id = iv
        .raise_for_worker(
            &run,
            &w,
            InterventionKind::ProviderPolicyBlock,
            details("account blocked"),
        )
        .await
        .unwrap();
    let err = iv.resolve(&id, "try").await.unwrap_err();
    assert!(
        err.what_failed().contains("has not cleared the block"),
        "{}",
        err.what_failed()
    );
    assert!(matches!(
        Runs::new(&e.services).worker_status(&w).unwrap().state(),
        WorkerState::NeedsHuman { .. }
    ));
    cleared.store(true, Ordering::SeqCst);
    iv.resolve(&id, "cleared by the provider").await.unwrap();
    assert_eq!(
        Runs::new(&e.services).worker_status(&w).unwrap().state(),
        WorkerState::Active(WorkerPhase::Implementing)
    );
}

#[tokio::test]
async fn resolving_an_unknown_intervention_is_an_error() {
    let e = env("unknown");
    e.create_run().await;
    assert!(
        Interventions::new(&e.services)
            .resolve("iv-nope", "x")
            .await
            .is_err()
    );
}
