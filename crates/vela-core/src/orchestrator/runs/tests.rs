use vela_domain::events::{Event, RunPausedPayload};
use vela_domain::ids::{BranchName, CommitSha, RunId, TicketId, WorkerId, WorktreePath};
use vela_domain::state::{
    Cause, IntegrationHealth, PausedReason, RunPhase, RunState, WorkerPhase, WorkerState,
};

use super::super::test_support::env;
use super::*;

fn healthy() -> IntegrationHealth {
    IntegrationHealth::Healthy
}

#[tokio::test]
async fn a_legal_transition_commits_state_and_journal_together() {
    let e = env("legal");
    let run = e.create_run().await;
    let runs = Runs::new(&e.services);
    assert_eq!(
        runs.run_state(&run).unwrap(),
        RunState::Active(RunPhase::Created)
    );
    let next = runs
        .transition_run(
            &run,
            RunState::Active(RunPhase::Preflight),
            Cause::System,
            healthy(),
            Vec::new(),
        )
        .await
        .unwrap();
    assert_eq!(next, RunState::Active(RunPhase::Preflight));
    assert_eq!(runs.run_state(&run).unwrap(), next);
    // The journal's tip and the run row agree.
    let (tip, row_seq): (u64, i64) = e
        .services
        .store
        .read(|c| {
            Ok((
                vela_persistence::journal::tip(c)?,
                c.query_row(
                    "SELECT last_event_seq FROM build_runs WHERE id = 'run-1'",
                    [],
                    |r| r.get(0),
                )
                .unwrap(),
            ))
        })
        .unwrap();
    assert_eq!(i64::try_from(tip).unwrap(), row_seq);
}

#[tokio::test]
async fn an_illegal_transition_is_rejected_journaled_and_changes_nothing() {
    let e = env("illegal");
    let run = e.create_run().await;
    let runs = Runs::new(&e.services);
    let before = e.raw_journal().len();
    let err = runs
        .transition_run(
            &run,
            RunState::Active(RunPhase::Running),
            Cause::System,
            healthy(),
            Vec::new(),
        )
        .await
        .unwrap_err();
    assert!(
        err.what_failed().contains("cannot do that now"),
        "{}",
        err.what_failed()
    );
    assert_eq!(
        runs.run_state(&run).unwrap(),
        RunState::Active(RunPhase::Created),
        "state unchanged"
    );
    let journal = e.raw_journal();
    assert_eq!(journal.len(), before + 1, "exactly one discrepancy entry");
    assert_eq!(journal.last().unwrap().0, "DiscrepancyDetected");
    assert!(journal.last().unwrap().1.contains("illegal run transition"));
}

#[tokio::test]
async fn events_ride_with_their_transition_and_a_terminal_run_stays_terminal() {
    let e = env("events");
    let run = e.create_run().await;
    let runs = Runs::new(&e.services);
    e.drive_run(&run, RunPhase::Running).await;
    runs.transition_run(
        &run,
        RunState::Paused {
            reason: PausedReason::Manual,
            resume_state: RunPhase::Running,
        },
        Cause::UserDecision,
        healthy(),
        vec![Event::RunPaused(RunPausedPayload {
            reason: "manual".to_owned(),
        })],
    )
    .await
    .unwrap();
    assert!(e.raw_journal().iter().any(|(k, _)| k == "RunPaused"));
    runs.transition_run(
        &run,
        RunState::Cancelled,
        Cause::UserDecision,
        healthy(),
        Vec::new(),
    )
    .await
    .unwrap();
    assert!(
        runs.transition_run(
            &run,
            RunState::Active(RunPhase::Preflight),
            Cause::UserDecision,
            healthy(),
            Vec::new()
        )
        .await
        .is_err()
    );
    assert_eq!(runs.run_state(&run).unwrap(), RunState::Cancelled);
}

#[tokio::test]
async fn workers_are_created_provisioned_and_advanced_through_the_machine() {
    let e = env("worker");
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
        healthy(),
    )
    .await
    .unwrap();
    assert_eq!(
        runs.worker_status(&w).unwrap().state(),
        WorkerState::Active(WorkerPhase::Provisioning)
    );
    runs.advance_worker(&run, &w, WorkerPhase::ContextLoading, healthy())
        .await
        .unwrap();
    // Skipping ahead is illegal and journaled.
    assert!(
        runs.advance_worker(&run, &w, WorkerPhase::Merging, healthy())
            .await
            .is_err()
    );
    assert_eq!(
        runs.worker_status(&w).unwrap().state(),
        WorkerState::Active(WorkerPhase::ContextLoading)
    );
    assert!(
        e.raw_journal()
            .iter()
            .any(|(k, p)| k == "DiscrepancyDetected" && p.contains("illegal worker transition"))
    );
    // Unhealthy integration blocks a new worker.
    let blocked = runs
        .create_worker(
            &run,
            WorkerId::new("w-2").unwrap(),
            &TicketId::new("T-2").unwrap(),
            BranchName::new("vela/run-1/t2").unwrap(),
            WorktreePath::new("C:/wt/t2").unwrap(),
            CommitSha::new("a".repeat(40)).unwrap(),
            "default",
            IntegrationHealth::Unhealthy,
        )
        .await;
    assert!(blocked.is_err());
}

#[tokio::test]
async fn a_missing_run_or_worker_is_a_typed_error() {
    let e = env("missing");
    let runs = Runs::new(&e.services);
    assert!(runs.run_state(&RunId::new("nope").unwrap()).is_err());
    assert!(runs.worker_status(&WorkerId::new("nope").unwrap()).is_err());
}
