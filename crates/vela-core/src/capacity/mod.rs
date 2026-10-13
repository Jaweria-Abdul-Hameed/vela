//! Capacity and execution profiles (S16): a capacity interruption pauses only the affected work and resumes by itself when the
//! profile is available again; a provider policy block never does.
//!
//! A provider policy block (FR-024) is non-retryable. It stops provider work at the next safe point, is held as an intervention
//! on every affected worker (and on the run when nothing can progress), survives a restart, and clears only by an explicit user
//! decision after one user-initiated access re-check: no timer, restart or successful call clears it, and there is no backoff, no
//! account switch, no key fallback and no new conversation.

use std::sync::Arc;

use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::events::{
    Event, ProfileAvailablePayload, ProfileCooldownStartedPayload,
    ProviderPolicyBlockClearedPayload, ProviderPolicyBlockRecordedPayload, WorkerPausedPayload,
    WorkerResumedPayload,
};
use vela_domain::ids::{RunId, WorkerId};
use vela_domain::ports::PortFuture;
use vela_domain::state::{Cause, IntegrationHealth, InterventionKind, PausedReason, WorkerState};
use vela_persistence::journal::NewEvent;
use vela_persistence::repositories::{
    o01_workers,
    s16_profiles::{self, ProfileRow},
};
use vela_persistence::writer::StateStore;

use vela_persistence::repositories::s15_interventions::Intervention;

use crate::intervention::{InterventionDetails, Interventions, ResolutionPrecondition};
use crate::orchestrator::Services;
use crate::orchestrator::runs::Runs;
use crate::scheduler::{DEFAULT_PROFILE, ProfileAvailability, ProfileAvailabilityService};

/// First backoff step.
pub const BASE_BACKOFF_MS: i64 = 30_000;
/// Longest backoff.
pub const MAX_BACKOFF_MS: i64 = 15 * 60_000;

/// What the capability and authentication interface says when the user asks to re-check access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessState {
    /// Signed in and the provider is serving requests.
    Ok,
    /// The user must sign in.
    AuthRequired,
    /// The provider still restricts access.
    Blocked(String),
}

/// The user-initiated access re-check (the existing capability and authentication-state interface).
pub trait AccessChecker: Send + Sync {
    /// Checks once. Never retried by Vela.
    fn check(&self) -> PortFuture<'_, AccessState>;
}

/// The backoff after `attempts` consecutive interruptions, honouring a provider hint.
pub fn backoff_ms(attempts: u32, hint_ms: Option<i64>) -> i64 {
    let exp = BASE_BACKOFF_MS.saturating_mul(1_i64 << attempts.saturating_sub(1).min(10));
    hint_ms
        .filter(|h| *h > 0)
        .unwrap_or(exp)
        .clamp(1_000, MAX_BACKOFF_MS)
}

/// Answers the scheduler and recovery: may a profile start work now?
struct StoreAvailability {
    store: Arc<StateStore>,
    now: Arc<dyn Fn() -> i64 + Send + Sync>,
}

impl ProfileAvailability for StoreAvailability {
    fn is_available(&self, profile: &str) -> bool {
        let Ok(row) = self.store.read(|c| s16_profiles::get(c, profile)) else {
            // Unknown state is never treated as permission.
            return false;
        };
        match row.state.as_str() {
            "AVAILABLE" | "BUSY" => true,
            "COOLDOWN" => row
                .cooldown_until_ms
                .is_some_and(|until| until <= (self.now)()),
            _ => false,
        }
    }
}

/// Holds a `PROVIDER_POLICY_BLOCK` intervention until the user re-checked access once, after the block was recorded.
struct PolicyBlockPrecondition {
    store: Arc<StateStore>,
}

impl ResolutionPrecondition for PolicyBlockPrecondition {
    fn check(&self, _intervention: &Intervention) -> Result<(), String> {
        let row = self
            .store
            .read(|c| s16_profiles::get(c, DEFAULT_PROFILE))
            .map_err(|e| e.what_failed().to_owned())?;
        match (row.blocked_ms, row.access_ok_ms) {
            (Some(blocked), Some(ok)) if ok >= blocked => Ok(()),
            _ => Err("check access once first: a block is never cleared by waiting or by a successful call".to_owned()),
        }
    }
}

/// Registers the profile-availability service and the policy-block resolution precondition.
pub fn register(services: &Services) {
    let clock = services.clock.clone();
    services.register(Arc::new(ProfileAvailabilityService(Arc::new(
        StoreAvailability {
            store: services.store.clone(),
            now: Arc::new(move || clock.now_ms()),
        },
    ))));
    Interventions::new(services).registry().register(
        InterventionKind::ProviderPolicyBlock,
        Arc::new(PolicyBlockPrecondition {
            store: services.store.clone(),
        }),
    );
}

/// Capacity handling over the shared services.
pub struct CapacityService {
    services: Arc<Services>,
}

impl CapacityService {
    /// Capacity over `services`.
    pub fn new(services: Arc<Services>) -> Self {
        Self { services }
    }

    /// The profile.
    pub fn profile(&self, id: &str) -> Result<ProfileRow, VelaError> {
        let id = id.to_owned();
        self.services.store.read(|c| s16_profiles::get(c, &id))
    }

    fn event(&self, run: &RunId, worker: Option<&WorkerId>, event: Event) -> NewEvent {
        NewEvent {
            ts: self.services.now_ms(),
            run_id: Some(run.clone()),
            ticket_id: None,
            worker_id: worker.cloned(),
            event,
        }
    }

    async fn active_phase(&self, worker: &WorkerId) -> Result<Option<WorkerState>, VelaError> {
        let w = worker.as_str().to_owned();
        let row = self.services.store.read(|c| o01_workers::get(c, &w))?;
        Ok(row.and_then(|r| serde_json::from_str::<WorkerState>(&r.state).ok()))
    }

    /// The provider reported exhausted capacity: the profile cools down with backoff and only `affected` workers pause.
    pub async fn on_capacity(
        &self,
        run: &RunId,
        profile_id: &str,
        retry_after_ms: Option<i64>,
        affected: &[WorkerId],
    ) -> Result<i64, VelaError> {
        let mut p = self.profile(profile_id)?;
        if p.state == "POLICY_BLOCKED" {
            // A block outranks capacity and is never softened by it.
            return Err(VelaError::of_class(
                ErrorClass::PolicyBlock,
                "the profile is held by a provider policy block",
            ));
        }
        p.attempts += 1;
        let delay = backoff_ms(p.attempts, retry_after_ms);
        let until = self.services.now_ms() + delay;
        p.state = "COOLDOWN".to_owned();
        p.cooldown_until_ms = Some(until);
        p.reason = "capacity exhausted".to_owned();
        p.updated_ms = self.services.now_ms();
        let started = self.event(
            run,
            None,
            Event::ProfileCooldownStarted(ProfileCooldownStartedPayload {
                profile_id: profile_id.to_owned(),
                reason: "capacity exhausted".to_owned(),
                available_at_ms: Some(until),
            }),
        );
        self.services.store.write(move |tx| {
            s16_profiles::put(tx.conn(), &p)?;
            tx.append(&started).map(|_| ())
        })?;
        let runs = Runs::new(&self.services);
        for w in affected {
            let Some(WorkerState::Active(phase)) = self.active_phase(w).await? else {
                continue;
            };
            let ev = Event::WorkerPaused(WorkerPausedPayload {
                reason: "capacity".to_owned(),
            });
            runs.transition_worker(
                run,
                w,
                WorkerState::Paused {
                    reason: PausedReason::Capacity,
                    resume_state: phase,
                },
                Cause::System,
                IntegrationHealth::Healthy,
                vec![ev],
            )
            .await?;
        }
        Ok(until)
    }

    /// Time passed: a profile whose cooldown ended becomes available and the workers paused for capacity resume where they were.
    /// Returns the workers resumed.
    pub async fn tick(&self, run: &RunId, profile_id: &str) -> Result<Vec<WorkerId>, VelaError> {
        let mut p = self.profile(profile_id)?;
        let due = p.state == "COOLDOWN"
            && p.cooldown_until_ms
                .is_some_and(|u| u <= self.services.now_ms());
        if !due {
            return Ok(Vec::new());
        }
        p.state = "AVAILABLE".to_owned();
        p.cooldown_until_ms = None;
        p.attempts = 0;
        p.reason.clear();
        p.updated_ms = self.services.now_ms();
        let available = self.event(
            run,
            None,
            Event::ProfileAvailable(ProfileAvailablePayload {
                profile_id: profile_id.to_owned(),
            }),
        );
        self.services.store.write(move |tx| {
            s16_profiles::put(tx.conn(), &p)?;
            tx.append(&available).map(|_| ())
        })?;
        let r = run.as_str().to_owned();
        let rows = self
            .services
            .store
            .read(|c| o01_workers::list_for_run(c, &r))?;
        let runs = Runs::new(&self.services);
        let mut resumed = Vec::new();
        for row in rows {
            let Ok(WorkerState::Paused {
                reason: PausedReason::Capacity,
                resume_state,
            }) = serde_json::from_str::<WorkerState>(&row.state)
            else {
                continue;
            };
            let id = WorkerId::new(row.id).map_err(|e| VelaError::internal(e.to_string()))?;
            let ev = Event::WorkerResumed(WorkerResumedPayload {
                from_state: "PAUSED".to_owned(),
            });
            runs.transition_worker(
                run,
                &id,
                WorkerState::Active(resume_state),
                Cause::System,
                IntegrationHealth::Healthy,
                vec![ev],
            )
            .await?;
            resumed.push(id);
        }
        Ok(resumed)
    }

    /// The provider restricted the account or service. Recorded with the profile state in one transaction; every non-terminal worker
    /// in `affected` is held on `PROVIDER_POLICY_BLOCK`, and so is the run, because nothing can progress without the profile.
    pub async fn on_provider_policy_block(
        &self,
        run: &RunId,
        profile_id: &str,
        evidence: &str,
        affected: &[WorkerId],
    ) -> Result<(), VelaError> {
        let mut p = self.profile(profile_id)?;
        let now = self.services.now_ms();
        let already = p.state == "POLICY_BLOCKED";
        p.state = "POLICY_BLOCKED".to_owned();
        p.cooldown_until_ms = None;
        p.reason = evidence.to_owned();
        p.blocked_ms = Some(now);
        p.access_ok_ms = None;
        p.updated_ms = now;
        let recorded = self.event(
            run,
            None,
            Event::ProviderPolicyBlockRecorded(ProviderPolicyBlockRecordedPayload {
                profile_id: Some(profile_id.to_owned()),
                evidence: evidence.to_owned(),
            }),
        );
        self.services.store.write(move |tx| {
            s16_profiles::put(tx.conn(), &p)?;
            tx.append(&recorded).map(|_| ())
        })?;
        if already {
            return Ok(());
        }
        let ivs = Interventions::new(&self.services);
        let details = |summary: &str| {
            InterventionDetails {
            summary: summary.to_owned(),
            evidence: vec![evidence.to_owned()],
            preserved_work: "every worktree, branch and checkpoint is untouched".to_owned(),
            last_safe_commit: None,
            actions: vec![
                "check access once".to_owned(),
                "view the evidence".to_owned(),
                "cancel the run".to_owned(),
            ],
            consequences: "Vela will not retry, switch accounts or work around this; nothing resumes until you decide".to_owned(),
        }
        };
        for w in affected {
            if matches!(
                self.active_phase(w).await?,
                Some(WorkerState::Active(_) | WorkerState::Paused { .. })
            ) {
                ivs.raise_for_worker(
                    run,
                    w,
                    InterventionKind::ProviderPolicyBlock,
                    details("the provider restricted this account or service"),
                )
                .await?;
            }
        }
        // One profile: with it blocked no ticket can progress, so the run is held too.
        if ivs
            .raise_for_run(
                run,
                InterventionKind::ProviderPolicyBlock,
                details("the provider restricted this account or service"),
            )
            .await
            .is_err()
        {
            // The run was not in a state that takes a human gate (for example already stopping); the workers stay held.
        }
        Ok(())
    }

    /// The user asks to re-check access, once. Success is remembered only if it happened after the block.
    pub async fn recheck_access(
        &self,
        profile_id: &str,
        checker: &dyn AccessChecker,
    ) -> Result<AccessState, VelaError> {
        let state = checker.check().await?;
        if state == AccessState::Ok {
            let mut p = self.profile(profile_id)?;
            p.access_ok_ms = Some(self.services.now_ms());
            p.updated_ms = self.services.now_ms();
            self.services
                .store
                .write(move |tx| s16_profiles::put(tx.conn(), &p))?;
        }
        Ok(state)
    }

    /// After the user resolved the intervention (which the precondition allowed only after a successful re-check): the profile is
    /// available again and the block is journaled as cleared by the user.
    pub fn clear_block(&self, run: &RunId, profile_id: &str) -> Result<(), VelaError> {
        let mut p = self.profile(profile_id)?;
        if p.state != "POLICY_BLOCKED" {
            return Ok(());
        }
        p.state = "AVAILABLE".to_owned();
        p.reason.clear();
        p.blocked_ms = None;
        p.access_ok_ms = None;
        p.attempts = 0;
        p.updated_ms = self.services.now_ms();
        let cleared = self.event(
            run,
            None,
            Event::ProviderPolicyBlockCleared(ProviderPolicyBlockClearedPayload {
                profile_id: Some(profile_id.to_owned()),
                cleared_by: "user".to_owned(),
            }),
        );
        self.services.store.write(move |tx| {
            s16_profiles::put(tx.conn(), &p)?;
            tx.append(&cleared).map(|_| ())
        })
    }
}

#[cfg(test)]
mod tests;
