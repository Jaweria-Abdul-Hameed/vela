//! Scheduling cycle (S09): on each material event the orchestrator reconciles what it knows, recomputes the ready frontier,
//! applies the parallel-safety engine and the capacity limit, and creates worker records transactionally.
//!
//! The cycle only decides and records; starting the worker's session is the worker's own job. It never assumes a UI is open,
//! never starts anything while integration health forbids it, and records why each ready ticket was **not** started.

use std::collections::BTreeSet;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::ids::{BranchName, CommitSha, RunId, TicketId, WorkerId, WorktreePath};
use vela_domain::parallel::{SurfaceRules, classify_pair};
use vela_domain::state::{IntegrationHealth, RunPhase, RunState, WorkerState};
use vela_persistence::repositories::{o01_workers, s06_run_graphs};

use crate::analysis::stored_graph;
use crate::orchestrator::Services;
use crate::orchestrator::runs::Runs;
use crate::settings::SettingsService;

/// The execution profile every worker uses until profiles are configured.
pub const DEFAULT_PROFILE: &str = "default";

/// Whether an execution profile may start work now (S16 registers the real service). Absent means available.
pub trait ProfileAvailability: Send + Sync {
    /// True when `profile` can start a worker.
    fn is_available(&self, profile: &str) -> bool;
}

/// The registry entry that carries the profile-availability service.
pub struct ProfileAvailabilityService(pub Arc<dyn ProfileAvailability>);

/// Why a ready ticket was not started this cycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Serialized {
    /// The concurrency cap was reached.
    Capacity,
    /// Not safe to run with a ticket that is running or was chosen first.
    Unsafe {
        /// The other ticket.
        with: TicketId,
        /// The evidence label.
        label: String,
    },
    /// The execution profile is blocked.
    ProfileBlocked,
}

/// What a cycle decided.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CycleOutcome {
    /// Workers created this cycle.
    pub started: Vec<TicketId>,
    /// Ready tickets that were held back, with the reason.
    pub held: Vec<(TicketId, Serialized)>,
    /// True when integration health stopped the cycle before it started anything.
    pub integration_gate: bool,
    /// The run is finished: every ticket is done.
    pub all_done: bool,
    /// Tickets that cannot proceed because a worker failed or was cancelled.
    pub stuck: Vec<TicketId>,
}

/// Where workers go.
#[derive(Debug, Clone)]
pub struct CycleInput {
    /// The run.
    pub run: RunId,
    /// The folder under which worker worktrees are created (one folder per ticket).
    pub worktree_root: String,
    /// The integration branch head workers start from.
    pub integration_head: CommitSha,
    /// The current integration health.
    pub health: IntegrationHealth,
}

/// The scheduler over the shared services.
pub struct Scheduler {
    services: Arc<Services>,
}

fn slug(ticket: &TicketId) -> String {
    ticket
        .as_str()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

impl Scheduler {
    /// A scheduler over `services`.
    pub fn new(services: Arc<Services>) -> Self {
        Self { services }
    }

    /// Runs one scheduling cycle for a `RUNNING` run.
    pub async fn cycle(&self, input: &CycleInput) -> Result<CycleOutcome, VelaError> {
        let runs = Runs::new(&self.services);
        match runs.run_state(&input.run)? {
            RunState::Active(RunPhase::Running) => {}
            _ => {
                return Err(VelaError::of_class(
                    ErrorClass::ValidationFailure,
                    "workers can only be scheduled while the run is running",
                ));
            }
        }
        let stored = stored_graph(&self.services, &input.run)?.ok_or_else(|| {
            VelaError::of_class(ErrorClass::ValidationFailure, "the run has no stored graph")
        })?;
        let graph = stored.graph()?;

        // Reconcile: what each ticket's worker is doing, from the store (never from memory).
        let rows = {
            let r = input.run.as_str().to_owned();
            self.services
                .store
                .read(|c| o01_workers::list_for_run(c, &r))?
        };
        let mut done = BTreeSet::new();
        let mut active: Vec<TicketId> = Vec::new();
        let mut touched = BTreeSet::new();
        let mut stuck = Vec::new();
        for row in &rows {
            let ticket = TicketId::new(row.ticket_id.clone())
                .map_err(|e| VelaError::internal(format!("a stored ticket id is invalid: {e}")))?;
            let state: WorkerState = serde_json::from_str(&row.state).map_err(|e| {
                VelaError::internal(format!("a stored worker state is unreadable: {e}"))
            })?;
            touched.insert(ticket.clone());
            match state {
                WorkerState::Complete => {
                    done.insert(ticket);
                }
                WorkerState::Failed | WorkerState::Cancelled => stuck.push(ticket),
                _ => active.push(ticket),
            }
        }

        let mut outcome = CycleOutcome {
            stuck,
            ..CycleOutcome::default()
        };
        if done.len() == stored.snapshot.tickets.len() {
            outcome.all_done = true;
            self.record(&input.run, &outcome)?;
            return Ok(outcome);
        }

        // Integration-health gate: nothing new starts unless the integration branch is validated.
        if input.health != IntegrationHealth::Healthy {
            outcome.integration_gate = true;
            self.record(&input.run, &outcome)?;
            return Ok(outcome);
        }

        let frontier: Vec<TicketId> = graph
            .frontier(&done)
            .map_err(|e| VelaError::internal(format!("the frontier cannot be computed: {e}")))?
            .into_iter()
            .filter(|t| !touched.contains(t))
            .collect();

        let settings = SettingsService::new(&self.services);
        let cap: usize = settings
            .run_value(&input.run, None, "max_parallel_workers")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2);
        let profile_ok = self
            .services
            .get::<ProfileAvailabilityService>()
            .is_none_or(|p| p.0.is_available(DEFAULT_PROFILE));
        let rules = SurfaceRules::default();

        let mut chosen: Vec<TicketId> = Vec::new();
        for candidate in frontier {
            if !profile_ok {
                outcome.held.push((candidate, Serialized::ProfileBlocked));
                continue;
            }
            if active.len() + chosen.len() >= cap {
                outcome.held.push((candidate, Serialized::Capacity));
                continue;
            }
            let mut blocked_by = None;
            for other in active.iter().chain(chosen.iter()) {
                let verdict = classify_pair(
                    &graph,
                    (other, stored.analyst.write_sets.get(other)),
                    (&candidate, stored.analyst.write_sets.get(&candidate)),
                    &stored.analyst.hazards,
                    &rules,
                );
                if !verdict.label.is_parallel_safe() {
                    blocked_by = Some((other.clone(), format!("{:?}", verdict.label)));
                    break;
                }
            }
            match blocked_by {
                Some((with, label)) => outcome
                    .held
                    .push((candidate, Serialized::Unsafe { with, label })),
                None => chosen.push(candidate),
            }
        }

        for ticket in &chosen {
            let worker = self.services.ids.new_worker_id();
            self.create_worker(input, &worker, ticket).await?;
            outcome.started.push(ticket.clone());
        }
        self.record(&input.run, &outcome)?;
        Ok(outcome)
    }

    async fn create_worker(
        &self,
        input: &CycleInput,
        worker: &WorkerId,
        ticket: &TicketId,
    ) -> Result<(), VelaError> {
        let name = slug(ticket);
        let branch = BranchName::new(format!("vela/{}/{name}", input.run.as_str()))
            .map_err(|e| VelaError::of_class(ErrorClass::ValidationFailure, e.to_string()))?;
        let root = input.worktree_root.trim_end_matches(['/', '\\']);
        let path = WorktreePath::new(format!("{root}/{}/{name}", input.run.as_str()))
            .map_err(|e| VelaError::of_class(ErrorClass::ValidationFailure, e.to_string()))?;
        Runs::new(&self.services)
            .create_worker(
                &input.run,
                worker.clone(),
                ticket,
                branch,
                path,
                input.integration_head.clone(),
                DEFAULT_PROFILE,
                input.health,
            )
            .await
    }

    fn record(&self, run: &RunId, outcome: &CycleOutcome) -> Result<(), VelaError> {
        let json =
            serde_json::to_string(outcome).map_err(|e| VelaError::internal(e.to_string()))?;
        let r = run.as_str().to_owned();
        self.services
            .store
            .write(move |tx| s06_run_graphs::set_last_cycle(tx.conn(), &r, &json))
    }

    /// What the last cycle decided (the recorded serialization reasons).
    pub fn last_cycle(&self, run: &RunId) -> Result<Option<CycleOutcome>, VelaError> {
        let r = run.as_str().to_owned();
        let row = self.services.store.read(|c| s06_run_graphs::get(c, &r))?;
        match row {
            Some(g) if g.last_cycle != "{}" => serde_json::from_str(&g.last_cycle)
                .map(Some)
                .map_err(|e| VelaError::internal(format!("the recorded cycle is unreadable: {e}"))),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests;
