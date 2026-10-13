use vela_domain::ids::{BranchName, CommitSha, TicketId, WorktreePath};
use vela_domain::state::{RunPhase, RunState, WorkerPhase};

use super::*;
use crate::orchestrator::test_support::{TestEnv, env};

struct Checker(AccessState);

impl AccessChecker for Checker {
    fn check(&self) -> PortFuture<'_, AccessState> {
        let s = self.0.clone();
        Box::pin(async move { Ok(s) })
    }
}

async fn rig(tag: &str, workers: usize) -> (TestEnv, RunId, Vec<WorkerId>) {
    let e = env(tag);
    let run = e.create_run().await;
    e.drive_run(&run, RunPhase::Running).await;
    register(&e.services);
    let runs = Runs::new(&e.services);
    let mut ids = Vec::new();
    for n in 0..workers {
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
        ids.push(w);
    }
    (e, run, ids)
}

fn state(e: &TestEnv, w: &WorkerId) -> WorkerState {
    Runs::new(&e.services).worker_status(w).unwrap().state()
}

fn available(e: &TestEnv) -> bool {
    e.services
        .get::<ProfileAvailabilityService>()
        .unwrap()
        .0
        .is_available(DEFAULT_PROFILE)
}

#[test]
fn backoff_grows_is_capped_and_honours_the_providers_hint() {
    assert_eq!(backoff_ms(1, None), 30_000);
    assert_eq!(backoff_ms(2, None), 60_000);
    assert_eq!(backoff_ms(3, None), 120_000);
    assert_eq!(backoff_ms(30, None), MAX_BACKOFF_MS);
    assert_eq!(backoff_ms(5, Some(45_000)), 45_000);
    assert_eq!(backoff_ms(1, Some(10_000_000)), MAX_BACKOFF_MS);
    assert_eq!(backoff_ms(1, Some(-5)), 30_000);
}

#[tokio::test]
async fn capacity_pauses_only_the_affected_workers_and_resumes_them_by_themselves() {
    let (e, run, w) = rig("capacity", 3).await;
    let svc = CapacityService::new(e.services.clone());
    assert!(available(&e));
    let until = svc
        .on_capacity(&run, DEFAULT_PROFILE, None, &w[..2])
        .await
        .unwrap();
    assert!(until > e.services.now_ms());
    assert!(matches!(
        state(&e, &w[0]),
        WorkerState::Paused {
            reason: PausedReason::Capacity,
            resume_state: WorkerPhase::Implementing
        }
    ));
    assert!(matches!(state(&e, &w[1]), WorkerState::Paused { .. }));
    assert_eq!(
        state(&e, &w[2]),
        WorkerState::Active(WorkerPhase::Implementing),
        "unrelated workers continue"
    );
    assert!(!available(&e), "the scheduler sees the cooldown");
    assert!(
        svc.tick(&run, DEFAULT_PROFILE).await.unwrap().is_empty(),
        "too early"
    );

    e.clock.advance(until - e.services.now_ms() + 1);
    assert!(available(&e));
    let resumed = svc.tick(&run, DEFAULT_PROFILE).await.unwrap();
    assert_eq!(resumed.len(), 2);
    assert_eq!(
        state(&e, &w[0]),
        WorkerState::Active(WorkerPhase::Implementing)
    );
    assert_eq!(svc.profile(DEFAULT_PROFILE).unwrap().attempts, 0);
    let k: Vec<String> = e.raw_journal().into_iter().map(|(k, _)| k).collect();
    for needle in [
        "ProfileCooldownStarted",
        "WorkerPaused",
        "ProfileAvailable",
        "WorkerResumed",
    ] {
        assert!(k.iter().any(|x| x == needle), "{needle} missing in {k:?}");
    }
}

#[tokio::test]
async fn repeated_interruptions_back_off_further() {
    let (e, run, w) = rig("backoff", 1).await;
    let svc = CapacityService::new(e.services.clone());
    let first = svc
        .on_capacity(&run, DEFAULT_PROFILE, None, &w)
        .await
        .unwrap()
        - e.services.now_ms();
    e.clock.advance(first + 1);
    // No tick in between: the profile stays in cooldown state, the next interruption doubles the delay.
    let second = svc
        .on_capacity(&run, DEFAULT_PROFILE, None, &w)
        .await
        .unwrap()
        - e.services.now_ms();
    assert_eq!((first, second), (30_000, 60_000));
}

#[tokio::test]
async fn a_policy_block_holds_every_worker_and_the_run_and_survives_a_restart() {
    let (e, run, w) = rig("block", 2).await;
    let svc = CapacityService::new(e.services.clone());
    svc.on_provider_policy_block(
        &run,
        DEFAULT_PROFILE,
        "account suspended for terms violation",
        &w,
    )
    .await
    .unwrap();
    for id in &w {
        assert!(matches!(
            state(&e, id),
            WorkerState::NeedsHuman {
                kind: InterventionKind::ProviderPolicyBlock,
                resume_state: WorkerPhase::Implementing
            }
        ));
    }
    assert!(matches!(
        Runs::new(&e.services).run_state(&run).unwrap(),
        RunState::NeedsHuman {
            kind: InterventionKind::ProviderPolicyBlock,
            ..
        }
    ));
    assert!(!available(&e));
    assert!(
        e.raw_journal()
            .iter()
            .any(|(k, p)| k == "ProviderPolicyBlockRecorded" && p.contains("terms violation"))
    );

    // Restart: nothing is cleared by time or by restarting.
    let reopened = e.restart();
    register(&reopened);
    assert!(
        !reopened
            .get::<ProfileAvailabilityService>()
            .unwrap()
            .0
            .is_available(DEFAULT_PROFILE)
    );
    e.clock.advance(24 * 3600 * 1000);
    assert!(
        !reopened
            .get::<ProfileAvailabilityService>()
            .unwrap()
            .0
            .is_available(DEFAULT_PROFILE)
    );
    assert_eq!(
        CapacityService::new(reopened)
            .profile(DEFAULT_PROFILE)
            .unwrap()
            .state,
        "POLICY_BLOCKED"
    );
}

#[tokio::test]
async fn a_capacity_report_cannot_soften_a_policy_block() {
    let (e, run, w) = rig("no-soften", 1).await;
    let svc = CapacityService::new(e.services.clone());
    svc.on_provider_policy_block(&run, DEFAULT_PROFILE, "blocked", &w)
        .await
        .unwrap();
    assert!(
        svc.on_capacity(&run, DEFAULT_PROFILE, None, &w)
            .await
            .is_err()
    );
    assert_eq!(
        svc.profile(DEFAULT_PROFILE).unwrap().state,
        "POLICY_BLOCKED"
    );
}

#[tokio::test]
async fn only_a_user_recheck_then_an_explicit_resolve_clears_a_block() {
    let (e, run, w) = rig("clear", 1).await;
    let svc = CapacityService::new(e.services.clone());
    svc.on_provider_policy_block(&run, DEFAULT_PROFILE, "blocked", &w)
        .await
        .unwrap();
    let ivs = Interventions::new(&e.services);
    let open = ivs.open_for_run(&run).unwrap();
    assert_eq!(open.len(), 2);
    let worker_iv = open
        .iter()
        .find(|i| i.worker_id.is_some())
        .unwrap()
        .id
        .clone();

    // Resolving without a re-check is refused.
    let err = ivs
        .resolve(&worker_iv, "I think it is fine")
        .await
        .unwrap_err();
    assert!(err.what_failed().contains("check access once"));
    // A re-check that still shows a block does not count.
    assert_eq!(
        svc.recheck_access(
            DEFAULT_PROFILE,
            &Checker(AccessState::Blocked("still".into()))
        )
        .await
        .unwrap(),
        AccessState::Blocked("still".into())
    );
    assert!(ivs.resolve(&worker_iv, "x").await.is_err());
    assert_eq!(
        svc.recheck_access(DEFAULT_PROFILE, &Checker(AccessState::AuthRequired))
            .await
            .unwrap(),
        AccessState::AuthRequired
    );
    assert!(ivs.resolve(&worker_iv, "x").await.is_err());

    // One successful user re-check, then the explicit decision.
    assert_eq!(
        svc.recheck_access(DEFAULT_PROFILE, &Checker(AccessState::Ok))
            .await
            .unwrap(),
        AccessState::Ok
    );
    ivs.resolve(&worker_iv, "access restored").await.unwrap();
    svc.clear_block(&run, DEFAULT_PROFILE).unwrap();
    assert_eq!(
        state(&e, &w[0]),
        WorkerState::Active(WorkerPhase::Implementing),
        "back to resume_state"
    );
    assert!(available(&e));
    assert!(
        e.raw_journal()
            .iter()
            .any(|(k, p)| k == "ProviderPolicyBlockCleared" && p.contains("user"))
    );
    // Clearing again is a no-op.
    svc.clear_block(&run, DEFAULT_PROFILE).unwrap();
}

#[tokio::test]
async fn a_successful_recheck_from_before_a_new_block_does_not_count() {
    let (e, run, w) = rig("stale-recheck", 1).await;
    let svc = CapacityService::new(e.services.clone());
    svc.recheck_access(DEFAULT_PROFILE, &Checker(AccessState::Ok))
        .await
        .unwrap();
    e.clock.advance(10);
    svc.on_provider_policy_block(&run, DEFAULT_PROFILE, "blocked later", &w)
        .await
        .unwrap();
    let iv = Interventions::new(&e.services).open_for_run(&run).unwrap()[0]
        .id
        .clone();
    assert!(
        Interventions::new(&e.services)
            .resolve(&iv, "x")
            .await
            .is_err()
    );
}
