//! Interventions (S15): anything that needs a human becomes a record that preserves the work, names the last safe commit,
//! lists the evidence and the actions with their consequences, and can be resolved back to the recorded resume state.
//!
//! Raising an intervention moves the run (or the worker) to `NEEDS_HUMAN` with its kind and the state it returns to; resolving
//! needs the user's decision and any precondition the owning module registered for that kind. `PROVIDER_POLICY_BLOCK` has no
//! automatic resolution: its precondition is registered by S16.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::events::{
    Event, HumanActionRequiredPayload, RunResumedPayload, WorkerResumedPayload,
};
use vela_domain::ids::{CommitSha, RunId, WorkerId};
use vela_domain::state::{Cause, IntegrationHealth, InterventionKind, RunState, WorkerState};
use vela_persistence::repositories::s15_interventions::{self, Intervention};

use crate::orchestrator::Services;
use crate::orchestrator::runs::{Runs, read_run_state, read_worker_status};

/// A condition that must hold before an intervention of a kind can be resolved.
pub trait ResolutionPrecondition: Send + Sync {
    /// `Ok` when the intervention may be resolved now; `Err` carries what is still missing.
    fn check(&self, intervention: &Intervention) -> Result<(), String>;
}

/// Preconditions by kind, registered by the modules that own them (default: none).
#[derive(Default)]
pub struct PreconditionRegistry {
    by_kind: Mutex<HashMap<&'static str, Arc<dyn ResolutionPrecondition>>>,
}

impl PreconditionRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers the precondition for `kind` (replacing an earlier one).
    pub fn register(&self, kind: InterventionKind, precondition: Arc<dyn ResolutionPrecondition>) {
        self.by_kind
            .lock()
            .expect("preconditions lock")
            .insert(kind.name(), precondition);
    }

    fn get(&self, kind: &str) -> Option<Arc<dyn ResolutionPrecondition>> {
        self.by_kind
            .lock()
            .expect("preconditions lock")
            .get(kind)
            .cloned()
    }
}

/// What the human sees and what is preserved.
#[derive(Debug, Clone, Default)]
pub struct InterventionDetails {
    /// What is needed.
    pub summary: String,
    /// Short evidence strings.
    pub evidence: Vec<String>,
    /// What was preserved.
    pub preserved_work: String,
    /// The last commit known to be safe.
    pub last_safe_commit: Option<CommitSha>,
    /// The actions offered.
    pub actions: Vec<String>,
    /// What each action leads to.
    pub consequences: String,
}

fn to_json<T: serde::Serialize>(v: &T) -> Result<String, VelaError> {
    serde_json::to_string(v).map_err(|e| VelaError::internal(format!("could not encode: {e}")))
}

/// Interventions over the shared services.
pub struct Interventions<'a> {
    services: &'a Services,
    registry: Arc<PreconditionRegistry>,
}

impl<'a> Interventions<'a> {
    /// Interventions over `services`; the registry is the one registered there (created on first use).
    pub fn new(services: &'a Services) -> Self {
        let registry = services.get::<PreconditionRegistry>().unwrap_or_else(|| {
            let r = Arc::new(PreconditionRegistry::new());
            services.register(r.clone());
            r
        });
        Self { services, registry }
    }

    fn record(
        &self,
        run: &RunId,
        worker: Option<&WorkerId>,
        kind: InterventionKind,
        resume_state: String,
        details: &InterventionDetails,
    ) -> Result<(Intervention, String), VelaError> {
        let id = format!("iv-{}", self.services.ids.new_token());
        Ok((
            Intervention {
                id: id.clone(),
                run_id: run.as_str().to_owned(),
                worker_id: worker.map(|w| w.as_str().to_owned()),
                kind: kind.name().to_owned(),
                state: "OPEN".to_owned(),
                resume_state,
                summary: details.summary.clone(),
                evidence: to_json(&details.evidence)?,
                preserved_work: details.preserved_work.clone(),
                last_safe_commit: details
                    .last_safe_commit
                    .as_ref()
                    .map(|c| c.as_str().to_owned()),
                actions: to_json(&details.actions)?,
                consequences: details.consequences.clone(),
                created_ms: self.services.now_ms(),
                resolved_ms: None,
                resolution: None,
            },
            id,
        ))
    }

    /// Raises a run-level intervention: the run goes to `NEEDS_HUMAN` with `kind`, remembering the phase it returns to.
    pub async fn raise_for_run(
        &self,
        run: &RunId,
        kind: InterventionKind,
        details: InterventionDetails,
    ) -> Result<String, VelaError> {
        let runs = Runs::new(self.services);
        let current = runs.run_state(run)?;
        let RunState::Active(phase) = current else {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                format!(
                    "an intervention can only be raised while the run is active, not {}",
                    current.name()
                ),
            ));
        };
        let (record, id) = self.record(run, None, kind, to_json(&phase)?, &details)?;
        runs.transition_run(
            run,
            RunState::NeedsHuman {
                kind,
                resume_state: phase,
            },
            Cause::System,
            IntegrationHealth::Healthy,
            vec![Event::HumanActionRequired(HumanActionRequiredPayload {
                intervention_kind: kind.name().to_owned(),
                summary: details.summary.clone(),
            })],
        )
        .await?;
        self.services
            .store
            .write(move |tx| s15_interventions::insert(tx.conn(), &record))?;
        Ok(id)
    }

    /// Raises a worker-level intervention: the worker goes to `NEEDS_HUMAN` with `kind`, remembering its phase.
    pub async fn raise_for_worker(
        &self,
        run: &RunId,
        worker: &WorkerId,
        kind: InterventionKind,
        details: InterventionDetails,
    ) -> Result<String, VelaError> {
        if !kind.is_worker_level() {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                format!("{} is a run-level intervention", kind.name()),
            ));
        }
        let runs = Runs::new(self.services);
        let status = runs.worker_status(worker)?;
        let WorkerState::Active(phase) = status.state() else {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                format!(
                    "an intervention can only be raised while the worker is active, not {}",
                    status.state().name()
                ),
            ));
        };
        let (record, id) = self.record(run, Some(worker), kind, to_json(&phase)?, &details)?;
        runs.transition_worker(
            run,
            worker,
            WorkerState::NeedsHuman {
                kind,
                resume_state: phase,
            },
            Cause::System,
            IntegrationHealth::Healthy,
            vec![Event::HumanActionRequired(HumanActionRequiredPayload {
                intervention_kind: kind.name().to_owned(),
                summary: details.summary.clone(),
            })],
        )
        .await?;
        self.services
            .store
            .write(move |tx| s15_interventions::insert(tx.conn(), &record))?;
        Ok(id)
    }

    /// The intervention record.
    pub fn get(&self, id: &str) -> Result<Option<Intervention>, VelaError> {
        self.services.store.read(|c| s15_interventions::get(c, id))
    }

    /// Open interventions of a run.
    pub fn open_for_run(&self, run: &RunId) -> Result<Vec<Intervention>, VelaError> {
        self.services
            .store
            .read(|c| s15_interventions::list_open(c, run.as_str()))
    }

    /// Resolves an open intervention on the user's decision, returning the run or worker to its recorded resume state. A kind
    /// with a registered precondition cannot be resolved until the precondition holds.
    pub async fn resolve(&self, id: &str, resolution: &str) -> Result<(), VelaError> {
        let record = self.get(id)?.filter(|r| r.state == "OPEN").ok_or_else(|| {
            VelaError::of_class(
                ErrorClass::ValidationFailure,
                "there is no open intervention with that id",
            )
        })?;
        if let Some(precondition) = self.registry.get(&record.kind) {
            precondition.check(&record).map_err(|missing| {
                VelaError::of_class(
                    ErrorClass::UserActionRequired,
                    format!("the intervention cannot be resolved yet: {missing}"),
                )
            })?;
        }
        let run =
            RunId::new(record.run_id.clone()).map_err(|e| VelaError::internal(e.to_string()))?;
        let runs = Runs::new(self.services);
        match &record.worker_id {
            Some(worker_text) => {
                let worker = WorkerId::new(worker_text.clone())
                    .map_err(|e| VelaError::internal(e.to_string()))?;
                let phase = serde_json::from_str(&record.resume_state).map_err(|e| {
                    VelaError::internal(format!("the recorded resume state is unreadable: {e}"))
                })?;
                let status = self
                    .services
                    .store
                    .read(|c| read_worker_status(c, &worker))?;
                runs.transition_worker(
                    &run,
                    &worker,
                    WorkerState::Active(phase),
                    Cause::UserDecision,
                    IntegrationHealth::Healthy,
                    vec![Event::WorkerResumed(WorkerResumedPayload {
                        from_state: status.state().name().to_owned(),
                    })],
                )
                .await?;
            }
            None => {
                let phase = serde_json::from_str(&record.resume_state).map_err(|e| {
                    VelaError::internal(format!("the recorded resume state is unreadable: {e}"))
                })?;
                let _ = self.services.store.read(|c| read_run_state(c, &run))?;
                runs.transition_run(
                    &run,
                    RunState::Active(phase),
                    Cause::UserDecision,
                    IntegrationHealth::Healthy,
                    vec![Event::RunResumed(RunResumedPayload {
                        reason: format!("intervention {} resolved", record.kind),
                    })],
                )
                .await?;
            }
        }
        let (now, id_text, resolution) =
            (self.services.now_ms(), id.to_owned(), resolution.to_owned());
        self.services.store.write(move |tx| {
            s15_interventions::resolve(tx.conn(), &id_text, &resolution, now).map(|_| ())
        })
    }
}

#[cfg(test)]
mod tests;
