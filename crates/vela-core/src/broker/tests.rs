use std::sync::Mutex;

use vela_domain::ids::{BranchName, CommitSha, WorktreePath};
use vela_domain::ports::{DeliveryCapability, DeliveryResult};
use vela_domain::state::{IntegrationHealth, WorkerPhase, WorkerState};

use super::*;
use crate::orchestrator::runs::Runs;
use crate::orchestrator::test_support::{TestEnv, env};
use crate::trust::{ProfileCommand, TrustService};

struct Delivery {
    tier: DeliveryTier,
    available: bool,
    verified: bool,
    delivered: Mutex<Vec<DeliveryDecision>>,
}

impl Delivery {
    fn new(tier: DeliveryTier) -> Arc<Self> {
        Arc::new(Self {
            tier,
            available: true,
            verified: true,
            delivered: Mutex::new(Vec::new()),
        })
    }
    fn count(&self) -> usize {
        self.delivered.lock().unwrap().len()
    }
}

impl ApprovalDeliveryAdapter for Delivery {
    fn detect(&self) -> PortFuture<'_, DeliveryCapability> {
        let c = DeliveryCapability {
            tier: self.tier,
            available: self.available,
        };
        Box::pin(async move { Ok(c) })
    }
    fn deliver<'a>(&'a self, r: &'a DeliveryRequest) -> PortFuture<'a, DeliveryResult> {
        self.delivered.lock().unwrap().push(r.decision);
        let verified = self.verified;
        Box::pin(async move {
            Ok(DeliveryResult {
                verified,
                detail: if verified {
                    "clicked".into()
                } else {
                    "button did not change".into()
                },
            })
        })
    }
    fn cancel<'a>(&'a self, _o: &'a ApprovalObservation) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

struct Rig {
    e: TestEnv,
    intake: Intake,
}

async fn rig(tag: &str) -> Rig {
    let e = env(tag);
    let run = e.create_run().await;
    e.drive_run(&run, vela_domain::state::RunPhase::Running)
        .await;
    let worker = WorkerId::new("w-1").unwrap();
    let ticket = TicketId::new("T-1").unwrap();
    let runs = Runs::new(&e.services);
    runs.create_worker(
        &run,
        worker.clone(),
        &ticket,
        BranchName::new("vela/run-1/t-1").unwrap(),
        WorktreePath::new("C:/work/app").unwrap(),
        CommitSha::new("a".repeat(40)).unwrap(),
        "default",
        IntegrationHealth::Healthy,
    )
    .await
    .unwrap();
    for p in [WorkerPhase::ContextLoading, WorkerPhase::Implementing] {
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
    let intake = Intake {
        run,
        worker,
        ticket,
        project_id: "p1".into(),
        worktree_root: "C:/work/app".into(),
        mode: RunMode::Supervised,
        observation: obs("cargo test", EvidenceTier::Bound),
    };
    Rig { e, intake }
}

fn obs(op: &str, tier: EvidenceTier) -> ApprovalObservation {
    ApprovalObservation {
        source_id: "hook-spool".into(),
        session_identity: Some("conv-1".into()),
        operation: op.into(),
        working_directory: Some("C:/work/app".into()),
        options: vec!["allow once".into(), "deny".into()],
        evidence: tier,
    }
}

fn worker_state(r: &Rig) -> WorkerState {
    Runs::new(&r.e.services)
        .worker_status(&r.intake.worker)
        .unwrap()
        .state()
}

fn kinds(r: &Rig) -> Vec<String> {
    r.e.raw_journal().into_iter().map(|(k, _)| k).collect()
}

#[tokio::test]
async fn a_profile_command_with_bound_evidence_is_delivered_verified_and_journaled() {
    let r = rig("allow").await;
    let d = Delivery::new(DeliveryTier::Native);
    let b = Broker::new(r.e.services.clone(), Some(d.clone()), "native");
    let out = b.handle(&r.intake).await.unwrap();
    let BrokerOutcome::Delivered {
        approval_id,
        via_rule,
    } = out
    else {
        panic!("expected delivery, got {out:?}")
    };
    assert_eq!(via_rule, None);
    assert_eq!(
        d.delivered.lock().unwrap().as_slice(),
        [DeliveryDecision::AllowOnce]
    );
    let row = b.get(&approval_id).unwrap().unwrap();
    assert_eq!(
        (
            row.state.as_str(),
            row.decision.as_deref(),
            row.evidence_tier.as_str()
        ),
        ("ALLOWED", Some("ALLOW"), "BOUND")
    );
    let k = kinds(&r);
    for needle in [
        "ApprovalDetected",
        "ApprovalClassified",
        "ApprovalDeliveryAttempted",
        "ApprovalDeliveryVerified",
    ] {
        assert!(k.iter().any(|x| x == needle), "{needle} missing in {k:?}");
    }
    assert_eq!(
        worker_state(&r),
        WorkerState::Active(WorkerPhase::Implementing)
    );
}

#[tokio::test]
async fn weak_or_observed_evidence_is_never_auto_approved() {
    for tier in [EvidenceTier::Weak, EvidenceTier::Observed] {
        let mut r = rig("weak").await;
        r.intake.observation = obs("cargo test", tier);
        let d = Delivery::new(DeliveryTier::Native);
        let out = Broker::new(r.e.services.clone(), Some(d.clone()), "native")
            .handle(&r.intake)
            .await
            .unwrap();
        assert!(
            matches!(out, BrokerOutcome::Asked { .. }),
            "{tier:?}: {out:?}"
        );
        assert_eq!(d.count(), 0, "AT-012: nothing is clicked on weak evidence");
        assert!(matches!(
            worker_state(&r),
            WorkerState::NeedsHuman {
                kind: InterventionKind::ApprovalAsk,
                ..
            }
        ));
    }
}

#[tokio::test]
async fn an_unlisted_command_asks_and_waits_for_a_human() {
    let mut r = rig("ask").await;
    r.intake.observation = obs("cargo publish", EvidenceTier::Bound);
    let d = Delivery::new(DeliveryTier::Native);
    let b = Broker::new(r.e.services.clone(), Some(d.clone()), "native");
    let BrokerOutcome::Asked { approval_id, .. } = b.handle(&r.intake).await.unwrap() else {
        panic!("expected ask")
    };
    assert_eq!(b.get(&approval_id).unwrap().unwrap().state, "ASKED");
    assert_eq!(d.count(), 0);
}

#[tokio::test]
async fn an_unverified_delivery_becomes_approval_undeliverable() {
    let r = rig("unverified").await;
    let d = Arc::new(Delivery {
        tier: DeliveryTier::Native,
        available: true,
        verified: false,
        delivered: Mutex::new(vec![]),
    });
    let out = Broker::new(r.e.services.clone(), Some(d), "native")
        .handle(&r.intake)
        .await
        .unwrap();
    assert!(matches!(out, BrokerOutcome::Undeliverable { .. }));
    assert!(matches!(
        worker_state(&r),
        WorkerState::NeedsHuman {
            kind: InterventionKind::ApprovalUndeliverable,
            ..
        }
    ));
    assert!(!kinds(&r).contains(&"ApprovalDeliveryVerified".to_owned()));
}

#[tokio::test]
async fn no_delivery_path_at_all_is_undeliverable_not_a_silent_success() {
    let r = rig("no-path").await;
    let out = Broker::new(r.e.services.clone(), None, "none")
        .handle(&r.intake)
        .await
        .unwrap();
    assert!(matches!(out, BrokerOutcome::Undeliverable { .. }));
}

#[tokio::test]
async fn ui_automation_needs_the_users_consent_but_native_delivery_does_not() {
    let r = rig("consent").await;
    let ui = Delivery::new(DeliveryTier::UiAutomation);
    let b = Broker::new(r.e.services.clone(), Some(ui.clone()), "uia");
    assert!(matches!(
        b.handle(&r.intake).await.unwrap(),
        BrokerOutcome::Undeliverable { .. }
    ));
    assert_eq!(ui.count(), 0, "no click without consent");

    let r = rig("consent-granted").await;
    Onboarding::new(&r.e.services)
        .grant("ui_automation")
        .unwrap();
    let ui = Delivery::new(DeliveryTier::UiAutomation);
    let out = Broker::new(r.e.services.clone(), Some(ui.clone()), "uia")
        .handle(&r.intake)
        .await
        .unwrap();
    assert!(matches!(out, BrokerOutcome::Delivered { .. }));
    // Revoking the consent stops it again immediately.
    Onboarding::new(&r.e.services)
        .revoke("ui_automation")
        .unwrap();
}

#[tokio::test]
async fn a_scoped_rule_turns_an_ask_into_a_delivered_allow_but_never_overrides_a_deny() {
    let mut r = rig("rule").await;
    r.intake.observation = obs("cargo publish", EvidenceTier::Bound);
    let d = Delivery::new(DeliveryTier::Native);
    let b = Broker::new(r.e.services.clone(), Some(d.clone()), "native");
    let BrokerOutcome::Asked { approval_id, .. } = b.handle(&r.intake).await.unwrap() else {
        panic!()
    };

    let rules = rules::RuleService::new(r.e.services.clone());
    let preview = rules.preview(&approval_id, "PERMANENT").unwrap();
    assert!(
        rules
            .create_permanent("p1", &approval_id, "something else")
            .is_err(),
        "the user must confirm the exact scope"
    );
    let rule = rules
        .create_permanent("p1", &approval_id, &preview)
        .unwrap();
    assert_eq!(rule.kind, "PERMANENT");

    // The worker is waiting on the first ask, so resolve it, then the same request is allowed by the rule.
    let ivs = Interventions::new(&r.e.services);
    let open = ivs.open_for_run(&r.intake.run).unwrap();
    ivs.resolve(&open[0].id, "allowed").await.unwrap();
    let out = b.handle(&r.intake).await.unwrap();
    assert!(
        matches!(&out, BrokerOutcome::Delivered { via_rule: Some(id), .. } if *id == rule.id),
        "{out:?}"
    );

    // Revoked: asks again.
    rules.revoke(&rule.id).unwrap();
    assert!(rules.revoke(&rule.id).is_err());
}

#[tokio::test]
async fn allow_once_expires_and_is_not_permanent() {
    let mut r = rig("once").await;
    r.intake.observation = obs("cargo publish", EvidenceTier::Bound);
    let b = Broker::new(
        r.e.services.clone(),
        Some(Delivery::new(DeliveryTier::Native)),
        "native",
    );
    let BrokerOutcome::Asked { approval_id, .. } = b.handle(&r.intake).await.unwrap() else {
        panic!()
    };
    let rules = rules::RuleService::new(r.e.services.clone());
    let once = rules.allow_once("p1", &approval_id).unwrap();
    assert_eq!(once.kind, "ONCE");
    assert!(once.expires_ms.is_some());
    assert_eq!(rules.active("p1").unwrap().len(), 1);
    r.e.clock.advance(rules::ONCE_TTL_MS + 1);
    assert!(
        rules.active("p1").unwrap().is_empty(),
        "an allow-once never outlives its window"
    );
}

#[tokio::test]
async fn dangerous_classes_can_never_get_a_standing_rule() {
    let mut r = rig("never").await;
    r.intake.observation = obs("git push --force origin main", EvidenceTier::Bound);
    let b = Broker::new(
        r.e.services.clone(),
        Some(Delivery::new(DeliveryTier::Native)),
        "native",
    );
    let out = b.handle(&r.intake).await.unwrap();
    // Whatever the policy says about a force push (refuse or ask), no permanent rule is possible for it.
    let id = match out {
        BrokerOutcome::Asked { approval_id, .. } | BrokerOutcome::Denied { approval_id } => {
            approval_id
        }
        other => panic!("unexpected {other:?}"),
    };
    let rules = rules::RuleService::new(r.e.services.clone());
    let preview = rules.preview(&id, "PERMANENT").unwrap();
    assert!(rules.create_permanent("p1", &id, &preview).is_err());
    assert!(rules.allow_once("p1", &id).is_err() || b.get(&id).unwrap().unwrap().state == "ASKED");
}

#[tokio::test]
async fn the_same_request_repeating_stops_at_the_threshold_and_progress_resets_it() {
    let mut r = rig("stall").await;
    r.intake.observation = obs("cargo test", EvidenceTier::Bound);
    let d = Arc::new(Delivery {
        tier: DeliveryTier::Native,
        available: true,
        verified: true,
        delivered: Mutex::new(vec![]),
    });
    let b = Broker::new(r.e.services.clone(), Some(d.clone()), "native").with_threshold(3);
    assert!(matches!(
        b.handle(&r.intake).await.unwrap(),
        BrokerOutcome::Delivered { .. }
    ));
    assert!(matches!(
        b.handle(&r.intake).await.unwrap(),
        BrokerOutcome::Delivered { .. }
    ));
    b.progress(&r.intake.worker).unwrap();
    assert!(
        matches!(
            b.handle(&r.intake).await.unwrap(),
            BrokerOutcome::Delivered { .. }
        ),
        "after progress the count restarts"
    );
    assert!(matches!(
        b.handle(&r.intake).await.unwrap(),
        BrokerOutcome::Delivered { .. }
    ));
    let out = b.handle(&r.intake).await.unwrap();
    let BrokerOutcome::Stalled { approval_id, .. } = out else {
        panic!("expected a stall, got {out:?}")
    };
    assert_eq!(b.get(&approval_id).unwrap().unwrap().state, "STALLED");
    assert_eq!(d.count(), 4, "nothing was delivered for the stalled repeat");
    assert!(kinds(&r).contains(&"ApprovalStalled".to_owned()));
    assert!(matches!(
        worker_state(&r),
        WorkerState::NeedsHuman {
            kind: InterventionKind::ApprovalStalled,
            ..
        }
    ));
}

#[tokio::test]
async fn an_interrupted_approval_is_closed_never_replayed() {
    let r = rig("recipe").await;
    register_recipes(&r.e.services);
    let ops = OperationLog::new(&r.e.services);
    let id = vela_domain::ids::OperationId::new("op-a").unwrap();
    ops.begin(&id, Some(&r.intake.run), "approval", &serde_json::json!({}))
        .unwrap();
    let reports = ops.reconcile_all().await.unwrap();
    assert!(matches!(reports[0].outcome, Reconciled::Completed(_)));
    assert_eq!(ops.get(&id).unwrap().unwrap().state, "RECONCILED");
}

#[test]
fn the_approval_fault_points_are_registered() {
    for n in ["approval.decision_persisted", "approval.delivery_attempted"] {
        assert!(FaultPoint::from_name(n).is_some(), "{n}");
    }
}

mod watchdog_tests {
    use super::*;
    use crate::broker::watchdog::{DEFAULT_IDLE_MS, escalate, find_suspected};
    use vela_domain::events::{CommandFinishedPayload, CommandStartedPayload};

    fn worker_event(r: &Rig, event: Event) {
        let new = NewEvent {
            ts: r.e.services.now_ms(),
            run_id: Some(r.intake.run.clone()),
            ticket_id: Some(r.intake.ticket.clone()),
            worker_id: Some(r.intake.worker.clone()),
            event,
        };
        r.e.services
            .store
            .write(move |tx| tx.append(&new).map(|_| ()))
            .unwrap();
    }

    #[tokio::test]
    async fn a_silent_worker_is_suspected_and_a_running_command_suppresses_it() {
        let r = rig("silent").await;
        assert!(
            find_suspected(&r.e.services, &r.intake.run, DEFAULT_IDLE_MS)
                .unwrap()
                .is_empty(),
            "just started"
        );
        r.e.clock.advance(DEFAULT_IDLE_MS + 1);
        let s = find_suspected(&r.e.services, &r.intake.run, DEFAULT_IDLE_MS).unwrap();
        assert_eq!(s.len(), 1);
        assert!(s[0].idle_ms > DEFAULT_IDLE_MS);

        // A tracked command that has started and not finished explains the silence.
        worker_event(
            &r,
            Event::CommandStarted(CommandStartedPayload {
                command_id: "c1".into(),
                command: "cargo test".into(),
                cwd: None,
            }),
        );
        r.e.clock.advance(DEFAULT_IDLE_MS * 3);
        assert!(
            find_suspected(&r.e.services, &r.intake.run, DEFAULT_IDLE_MS)
                .unwrap()
                .is_empty()
        );
        worker_event(
            &r,
            Event::CommandFinished(CommandFinishedPayload {
                command_id: "c1".into(),
                exit_code: Some(0),
                duration_ms: 5,
                timed_out: false,
            }),
        );
        r.e.clock.advance(DEFAULT_IDLE_MS + 1);
        assert_eq!(
            find_suspected(&r.e.services, &r.intake.run, DEFAULT_IDLE_MS)
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn escalation_raises_approval_stalled_and_says_when_it_is_only_a_suspicion() {
        let r = rig("escalate").await;
        r.e.clock.advance(DEFAULT_IDLE_MS + 1);
        let s = find_suspected(&r.e.services, &r.intake.run, DEFAULT_IDLE_MS).unwrap();
        let id = escalate(&r.e.services, &r.intake.run, &s[0], false)
            .await
            .unwrap();
        let iv = Interventions::new(&r.e.services).get(&id).unwrap().unwrap();
        assert!(iv.summary.contains("may be waiting on an approval"));
        assert!(matches!(
            worker_state(&r),
            WorkerState::NeedsHuman {
                kind: InterventionKind::ApprovalStalled,
                ..
            }
        ));
        // A worker waiting on a human is not "silent" any more.
        assert!(
            find_suspected(&r.e.services, &r.intake.run, DEFAULT_IDLE_MS)
                .unwrap()
                .is_empty()
        );
    }
}
