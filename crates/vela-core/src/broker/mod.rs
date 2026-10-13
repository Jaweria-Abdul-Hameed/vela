//! The approval broker (K01): approval requests are classified by policy, then delivered, asked about, or refused; everything is
//! persisted and journaled.
//!
//! Policy and delivery are separate. The broker never approves a *control*: it decides an *operation*, and only an operation
//! observed with bound evidence, classified `ALLOW`, with a permitted and consented delivery path, is ever delivered. Anything
//! ambiguous fails closed into an intervention. A decision is never replayed after a restart.

use std::sync::Arc;

use vela_domain::approval::Fingerprint;
use vela_domain::errors::VelaError;
use vela_domain::events::{
    ApprovalClassifiedPayload, ApprovalDeliveryAttemptedPayload, ApprovalDeliveryVerifiedPayload,
    ApprovalDetectedPayload, ApprovalStalledPayload, Event,
};
use vela_domain::fault::{self, FaultPoint};
use vela_domain::ids::{RunId, TicketId, WorkerId};
use vela_domain::policy::{Decision, Dialect, Operation, PolicyEngine, RunMode};
use vela_domain::ports::{
    ApprovalDeliveryAdapter, ApprovalObservation, DeliveryDecision, DeliveryRequest, DeliveryTier,
    EvidenceTier, PortFuture,
};
use vela_domain::state::InterventionKind;
use vela_persistence::journal::NewEvent;
use vela_persistence::repositories::k01_approvals::{self, ApprovalRow};

use crate::intervention::{InterventionDetails, Interventions};
use crate::operations::{OperationLog, Recipe, Reconciled};
use crate::orchestrator::Services;
use crate::settings::onboarding::Onboarding;
use crate::trust::TrustService;

pub mod rules;
pub mod session;
pub mod watchdog;

/// Repeats of one fingerprint without progress before the broker stops delivering and asks a human.
pub const DEFAULT_THRESHOLD: u32 = 3;

/// One observed request, with what the broker needs to decide it.
#[derive(Debug, Clone)]
pub struct Intake {
    /// The run.
    pub run: RunId,
    /// The worker.
    pub worker: WorkerId,
    /// Its ticket.
    pub ticket: TicketId,
    /// The project.
    pub project_id: String,
    /// The worker's worktree root.
    pub worktree_root: String,
    /// The run's autonomy mode.
    pub mode: RunMode,
    /// What was observed.
    pub observation: ApprovalObservation,
}

/// What the broker did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokerOutcome {
    /// Allowed once and delivered (verified).
    Delivered {
        /// The request.
        approval_id: String,
        /// The scoped rule that turned an ask into an allow, if one did.
        via_rule: Option<String>,
    },
    /// A human must decide.
    Asked {
        /// The request.
        approval_id: String,
        /// The intervention.
        intervention_id: String,
    },
    /// Refused by policy; a deny was delivered when a path existed.
    Denied {
        /// The request.
        approval_id: String,
    },
    /// Allowed by policy but it could not be delivered safely.
    Undeliverable {
        /// The request.
        approval_id: String,
        /// The intervention.
        intervention_id: String,
    },
    /// The same request repeated past the threshold without progress.
    Stalled {
        /// The request.
        approval_id: String,
        /// The intervention.
        intervention_id: String,
    },
}

/// The broker.
pub struct Broker {
    services: Arc<Services>,
    delivery: Option<Arc<dyn ApprovalDeliveryAdapter>>,
    adapter_name: String,
    threshold: u32,
}

fn tier_name(t: EvidenceTier) -> &'static str {
    match t {
        EvidenceTier::Weak => "WEAK",
        EvidenceTier::Observed => "OBSERVED",
        EvidenceTier::Bound => "BOUND",
    }
}

impl Broker {
    /// A broker with an optional delivery path.
    pub fn new(
        services: Arc<Services>,
        delivery: Option<Arc<dyn ApprovalDeliveryAdapter>>,
        adapter_name: &str,
    ) -> Self {
        Self {
            services,
            delivery,
            adapter_name: adapter_name.to_owned(),
            threshold: DEFAULT_THRESHOLD,
        }
    }

    /// Sets how many repeats stop delivery (at least 2).
    pub fn with_threshold(mut self, threshold: u32) -> Self {
        self.threshold = threshold.max(2);
        self
    }

    fn append(&self, i: &Intake, event: Event) -> NewEvent {
        NewEvent {
            ts: self.services.now_ms(),
            run_id: Some(i.run.clone()),
            ticket_id: Some(i.ticket.clone()),
            worker_id: Some(i.worker.clone()),
            event,
        }
    }

    /// Progress was made (a head advanced, a command completed): repeats start from zero again.
    pub fn progress(&self, worker: &WorkerId) -> Result<(), VelaError> {
        let w = worker.as_str().to_owned();
        self.services
            .store
            .write(move |tx| k01_approvals::clear_counters(tx.conn(), &w))
    }

    async fn raise(
        &self,
        i: &Intake,
        kind: InterventionKind,
        summary: String,
        evidence: Vec<String>,
    ) -> Result<String, VelaError> {
        Interventions::new(&self.services)
            .raise_for_worker(
                &i.run,
                &i.worker,
                kind,
                InterventionDetails {
                    summary,
                    evidence,
                    preserved_work: "the worker's worktree and conversation are untouched"
                        .to_owned(),
                    last_safe_commit: None,
                    actions: vec![
                        "allow it once".to_owned(),
                        "keep refusing and let the worker try another way".to_owned(),
                        "cancel the worker".to_owned(),
                    ],
                    consequences:
                        "the worker stays blocked until you decide; other workers continue"
                            .to_owned(),
                },
            )
            .await
    }

    /// Decides one observed request.
    pub async fn handle(&self, i: &Intake) -> Result<BrokerOutcome, VelaError> {
        let obs = &i.observation;
        let trust = TrustService::new(&self.services);
        let context = trust.policy_context(&i.project_id, &i.worktree_root, i.mode)?;
        let decision = PolicyEngine::new(context).evaluate(&Operation::Command {
            dialect: Dialect::Cmd,
            text: obs.operation.clone(),
            cwd: obs.working_directory.clone(),
        });
        let class = format!("{:?}", decision.scope.class);
        let executable = decision.scope.executable.clone().unwrap_or_default();
        let fingerprint = Fingerprint::new(
            "antigravity",
            &i.worker,
            decision.scope.class,
            &obs.operation,
            &i.worktree_root,
            &format!("{:?}", decision.scope.placement),
        );
        let id = format!("ap-{}", self.services.ids.new_token());
        let now = self.services.now_ms();

        // Persist the request, count the repeat, and journal the detection: one transaction.
        let row = ApprovalRow {
            id: id.clone(),
            run_id: i.run.as_str().to_owned(),
            worker_id: i.worker.as_str().to_owned(),
            source: obs.source_id.clone(),
            evidence_tier: tier_name(obs.evidence).to_owned(),
            class: class.clone(),
            executable: executable.clone(),
            fingerprint: fingerprint.as_str().to_owned(),
            state: "PENDING".to_owned(),
            decision: None,
            rule_id: None,
            reason: String::new(),
            created_ms: now,
            decided_ms: None,
        };
        let detected = self.append(
            i,
            Event::ApprovalDetected(ApprovalDetectedPayload {
                approval_id: id.clone(),
                source: obs.source_id.clone(),
                evidence_tier: tier_name(obs.evidence).to_owned(),
            }),
        );
        let (w, f) = (
            i.worker.as_str().to_owned(),
            fingerprint.as_str().to_owned(),
        );
        let count = self.services.store.write(move |tx| {
            k01_approvals::insert_request(tx.conn(), &row)?;
            tx.append(&detected)?;
            k01_approvals::bump_counter(tx.conn(), &w, &f)
        })?;

        let denied = decision.decision == Decision::Deny;
        if count >= self.threshold {
            return self
                .stall(
                    i,
                    &id,
                    fingerprint.as_str(),
                    count,
                    denied,
                    &class,
                    &executable,
                )
                .await;
        }

        // A standing or allow-once rule can turn an ASK into an allow; nothing turns a DENY into one.
        let mut effective = decision.decision;
        let mut via_rule = None;
        if effective == Decision::Ask
            && let Some(rule) = rules::RuleService::new(self.services.clone()).matching(
                &i.project_id,
                &class,
                &executable,
            )?
        {
            effective = Decision::Allow;
            via_rule = Some(rule.id);
        }
        // Allow needs bound evidence: anything weaker is only ever asked about.
        let mut reason = decision.reason.clone();
        if effective == Decision::Allow && obs.evidence != EvidenceTier::Bound {
            effective = Decision::Ask;
            reason = format!(
                "{reason}; the evidence for this request is not bound to the worker, so it is asked about"
            );
        }
        let label = match effective {
            Decision::Allow => "ALLOW",
            Decision::Ask => "ASK",
            Decision::Deny => "DENY",
        };
        let state = match effective {
            Decision::Allow => "ALLOWED",
            Decision::Ask => "ASKED",
            Decision::Deny => "DENIED",
        };
        let classified = self.append(
            i,
            Event::ApprovalClassified(ApprovalClassifiedPayload {
                approval_id: id.clone(),
                decision: label.to_owned(),
                rule_id: via_rule
                    .clone()
                    .or_else(|| Some(decision.rule_id.to_string())),
                reason: reason.clone(),
            }),
        );
        let (aid, rid, why, now2) = (
            id.clone(),
            via_rule
                .clone()
                .unwrap_or_else(|| decision.rule_id.to_string()),
            reason.clone(),
            self.services.now_ms(),
        );
        self.services.store.write(move |tx| {
            // A request that must still be delivered stays PENDING until its delivery is verified.
            let stored = if label == "ALLOW" { "PENDING" } else { state };
            k01_approvals::set_decision(tx.conn(), &aid, stored, label, Some(&rid), &why, now2)?;
            tx.append(&classified).map(|_| ())
        })?;
        fault::hit(FaultPoint::ApprovalDecisionPersisted);

        match effective {
            Decision::Allow => {
                self.deliver_allow(i, &id, via_rule, &class, &executable)
                    .await
            }
            Decision::Ask => {
                let intervention_id = self
                    .raise(
                        i,
                        InterventionKind::ApprovalAsk,
                        format!("{} wants to run `{executable}` ({class})", i.ticket),
                        vec![reason, format!("source: {}", obs.source_id)],
                    )
                    .await?;
                Ok(BrokerOutcome::Asked {
                    approval_id: id,
                    intervention_id,
                })
            }
            Decision::Deny => {
                // Best effort: tell the surface "deny" when a safe path exists; the hook already refuses on its own.
                if let Some(adapter) = self.usable_delivery(obs).await {
                    let _ = adapter
                        .deliver(&DeliveryRequest {
                            observation: obs.clone(),
                            decision: DeliveryDecision::Deny,
                        })
                        .await;
                }
                Ok(BrokerOutcome::Denied { approval_id: id })
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn stall(
        &self,
        i: &Intake,
        id: &str,
        fingerprint: &str,
        count: u32,
        denied: bool,
        class: &str,
        executable: &str,
    ) -> Result<BrokerOutcome, VelaError> {
        let now = self.services.now_ms();
        let stalled = self.append(
            i,
            Event::ApprovalStalled(ApprovalStalledPayload {
                approval_id: id.to_owned(),
                fingerprint: fingerprint.to_owned(),
                repeat_count: count,
            }),
        );
        let aid = id.to_owned();
        self.services.store.write(move |tx| {
            k01_approvals::set_state(tx.conn(), &aid, "STALLED", now)?;
            tx.append(&stalled).map(|_| ())
        })?;
        let kind = if denied {
            InterventionKind::PolicyDenied
        } else {
            InterventionKind::ApprovalStalled
        };
        let intervention_id = self
            .raise(
                i,
                kind,
                format!(
                    "{} asked for `{executable}` ({class}) {count} times without progress",
                    i.ticket
                ),
                vec![format!("fingerprint {fingerprint}")],
            )
            .await?;
        Ok(BrokerOutcome::Stalled {
            approval_id: id.to_owned(),
            intervention_id,
        })
    }

    /// The delivery adapter, when it is available and allowed for this request (consent for UI automation).
    async fn usable_delivery(
        &self,
        obs: &ApprovalObservation,
    ) -> Option<&Arc<dyn ApprovalDeliveryAdapter>> {
        let adapter = self.delivery.as_ref()?;
        let capability = adapter.detect().await.ok()?;
        if !capability.available || obs.evidence == EvidenceTier::Weak {
            return None;
        }
        if capability.tier != DeliveryTier::Native
            && !Onboarding::new(&self.services)
                .is_granted("ui_automation")
                .unwrap_or(false)
        {
            return None;
        }
        Some(adapter)
    }

    async fn deliver_allow(
        &self,
        i: &Intake,
        id: &str,
        via_rule: Option<String>,
        class: &str,
        executable: &str,
    ) -> Result<BrokerOutcome, VelaError> {
        let obs = &i.observation;
        let Some(adapter) = self.usable_delivery(obs).await else {
            return self.undeliverable(i, id, "no permitted way to deliver this approval (no available path, or UI automation is not consented)").await;
        };
        // Intent before the click: a crash in between is reconciled by never replaying it.
        let ops = OperationLog::new(&self.services);
        let op = self.services.ids.new_operation_id();
        ops.begin(
            &op,
            Some(&i.run),
            "approval",
            &serde_json::json!({"approval": id, "class": class, "executable": executable}),
        )?;
        let result = adapter
            .deliver(&DeliveryRequest {
                observation: obs.clone(),
                decision: DeliveryDecision::AllowOnce,
            })
            .await;
        fault::hit(FaultPoint::ApprovalDeliveryAttempted);
        let (verified, detail) = match &result {
            Ok(r) => (r.verified, r.detail.clone()),
            Err(e) => (false, e.what_failed().to_owned()),
        };
        let (did, aid, name, now) = (
            format!("del-{}", self.services.ids.new_token()),
            id.to_owned(),
            self.adapter_name.clone(),
            self.services.now_ms(),
        );
        let attempt = {
            let detail = detail.clone();
            self.services.store.write(move |tx| {
                let n = k01_approvals::insert_delivery(
                    tx.conn(),
                    &did,
                    &aid,
                    &name,
                    verified,
                    &detail,
                    now,
                )?;
                let ev = NewEvent {
                    ts: now,
                    run_id: None,
                    ticket_id: None,
                    worker_id: None,
                    event: Event::ApprovalDeliveryAttempted(ApprovalDeliveryAttemptedPayload {
                        approval_id: aid.clone(),
                        adapter: name.clone(),
                        attempt: n,
                    }),
                };
                tx.append(&ev)?;
                Ok(n)
            })?
        };
        let _ = attempt;
        ops.finish(&op, verified, &serde_json::json!({"verified": verified}))?;
        if !verified {
            return self
                .undeliverable(
                    i,
                    id,
                    &format!("the approval was not verified as delivered: {detail}"),
                )
                .await;
        }
        let verified_ev = self.append(
            i,
            Event::ApprovalDeliveryVerified(ApprovalDeliveryVerifiedPayload {
                approval_id: id.to_owned(),
                adapter: self.adapter_name.clone(),
            }),
        );
        let (aid, now) = (id.to_owned(), self.services.now_ms());
        self.services.store.write(move |tx| {
            k01_approvals::set_state(tx.conn(), &aid, "ALLOWED", now)?;
            tx.append(&verified_ev).map(|_| ())
        })?;
        Ok(BrokerOutcome::Delivered {
            approval_id: id.to_owned(),
            via_rule,
        })
    }

    async fn undeliverable(
        &self,
        i: &Intake,
        id: &str,
        why: &str,
    ) -> Result<BrokerOutcome, VelaError> {
        let (aid, now) = (id.to_owned(), self.services.now_ms());
        self.services
            .store
            .write(move |tx| k01_approvals::set_state(tx.conn(), &aid, "UNDELIVERABLE", now))?;
        let intervention_id = self
            .raise(
                i,
                InterventionKind::ApprovalUndeliverable,
                format!("the policy allows this, but Vela cannot deliver the approval: {why}"),
                vec![why.to_owned()],
            )
            .await?;
        Ok(BrokerOutcome::Undeliverable {
            approval_id: id.to_owned(),
            intervention_id,
        })
    }

    /// A request, as stored.
    pub fn get(&self, id: &str) -> Result<Option<ApprovalRow>, VelaError> {
        let id = id.to_owned();
        self.services
            .store
            .read(|c| k01_approvals::get_request(c, &id))
    }
}

/// Reconciles an interrupted approval operation. A stale decision or click is never replayed: the record is closed as abandoned,
/// and the worker's own block (still on screen, or resolved by the human) decides what happens next.
pub struct ApprovalRecipe;

impl Recipe for ApprovalRecipe {
    fn kind(&self) -> &'static str {
        "approval"
    }

    fn reconcile<'a>(
        &'a self,
        _operation: &'a vela_persistence::repositories::r01_operations::Operation,
    ) -> PortFuture<'a, Reconciled> {
        Box::pin(async {
            Ok(Reconciled::Completed(serde_json::json!({
                "abandoned": "an interrupted approval is never replayed"
            })))
        })
    }
}

/// Registers the approval recipe with the operation framework.
pub fn register_recipes(services: &Services) {
    let _ = OperationLog::new(services);
    if let Some(registry) = services.get::<crate::operations::RecipeRegistry>() {
        registry.register(Arc::new(ApprovalRecipe));
    }
}

#[cfg(test)]
mod tests;
