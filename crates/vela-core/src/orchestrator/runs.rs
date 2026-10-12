//! Run and worker lifecycle as transactions: each transition loads the state, applies the D01 table, writes the new state and
//! its journal events together, and publishes after the commit. An illegal transition changes nothing but is journaled.

use serde::Serialize;
use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::events::{
    DiscrepancyDetectedPayload, Event, RunCreatedPayload, WorkerStartedPayload,
};
use vela_domain::ids::{BranchName, CommitSha, RunId, TicketId, WorkerId, WorktreePath};
use vela_domain::state::{
    Cause, IntegrationHealth, RunPhase, RunState, WorkerPhase, WorkerState, WorkerStatus,
};
use vela_persistence::journal::NewEvent;
use vela_persistence::repositories::{f06_core, o01_workers};
use vela_persistence::writer::{Connection, WriteTx};

use super::Services;

fn encode<T: Serialize>(value: &T) -> Result<String, VelaError> {
    serde_json::to_string(value)
        .map_err(|e| VelaError::internal(format!("state could not be encoded: {e}")))
}

fn decode<T: serde::de::DeserializeOwned>(text: &str, what: &str) -> Result<T, VelaError> {
    serde_json::from_str(text).map_err(|e| {
        VelaError::of_class(
            ErrorClass::InternalBug,
            format!("the stored {what} is not readable: {e}"),
        )
    })
}

/// Reads a run's state from a connection.
pub fn read_run_state(conn: &Connection, run: &RunId) -> Result<RunState, VelaError> {
    let text = f06_core::run_state(conn, run.as_str())?.ok_or_else(|| {
        VelaError::of_class(
            ErrorClass::ValidationFailure,
            format!("there is no run {run}"),
        )
    })?;
    decode(&text, "run state")
}

/// Reads a worker's status from a connection.
pub fn read_worker_status(conn: &Connection, worker: &WorkerId) -> Result<WorkerStatus, VelaError> {
    let row = o01_workers::get(conn, worker.as_str())?.ok_or_else(|| {
        VelaError::of_class(
            ErrorClass::ValidationFailure,
            format!("there is no worker {worker}"),
        )
    })?;
    let state: WorkerState = decode(&row.state, "worker state")?;
    WorkerStatus::from_parts(state, row.waiting_approval).map_err(|e| {
        VelaError::of_class(
            ErrorClass::InternalBug,
            format!("the stored worker state is inconsistent: {e}"),
        )
    })
}

fn event(
    services: &Services,
    run: Option<&RunId>,
    ticket: Option<&TicketId>,
    worker: Option<&WorkerId>,
    event: Event,
) -> NewEvent {
    NewEvent {
        ts: services.now_ms(),
        run_id: run.cloned(),
        ticket_id: ticket.cloned(),
        worker_id: worker.cloned(),
        event,
    }
}

/// A journal entry for a rejected transition.
fn discrepancy(
    ts: i64,
    run: &RunId,
    worker: Option<&WorkerId>,
    subject: &str,
    from: &str,
    to: &str,
    why: &str,
) -> NewEvent {
    NewEvent {
        ts,
        run_id: Some(run.clone()),
        ticket_id: None,
        worker_id: worker.cloned(),
        event: Event::DiscrepancyDetected(DiscrepancyDetectedPayload {
            subject: subject.to_owned(),
            expected: format!("a transition the state machine allows from {from}"),
            observed: format!("{from} to {to}: {why}"),
        }),
    }
}

/// Run and worker operations over the shared services.
pub struct Runs<'a> {
    services: &'a Services,
}

impl<'a> Runs<'a> {
    /// Operations over `services`.
    pub fn new(services: &'a Services) -> Self {
        Self { services }
    }

    /// Creates a run in `CREATED` and journals `RunCreated`, atomically.
    pub async fn create_run(
        &self,
        project_id: &str,
        run: RunId,
        base: CommitSha,
        integration_branch: BranchName,
        scope: &str,
    ) -> Result<RunId, VelaError> {
        let now = self.services.now_ms();
        let state = encode(&RunState::Active(RunPhase::Created))?;
        let created = event(
            self.services,
            Some(&run),
            None,
            None,
            Event::RunCreated(RunCreatedPayload {
                run_base_sha: base,
                integration_branch,
                scope: scope.to_owned(),
            }),
        );
        let (project_id, run_text) = (project_id.to_owned(), run.as_str().to_owned());
        self.services.store.write(move |tx: &mut WriteTx<'_>| {
            f06_core::insert_run(tx.conn(), &run_text, &project_id, &state, now)?;
            tx.append(&created).map(|_| ())
        })?;
        Ok(run)
    }

    /// The run's state.
    pub fn run_state(&self, run: &RunId) -> Result<RunState, VelaError> {
        self.services.store.read(|c| read_run_state(c, run))
    }

    /// Applies `to` to the run. A legal transition writes the new state and `events` in one transaction. An illegal one writes
    /// only a `DiscrepancyDetected` journal entry and returns a typed error; the state is unchanged either way.
    pub async fn transition_run(
        &self,
        run: &RunId,
        to: RunState,
        cause: Cause,
        health: IntegrationHealth,
        events: Vec<Event>,
    ) -> Result<RunState, VelaError> {
        let (services_now, run_id) = (self.services.now_ms(), run.clone());
        let new_events: Vec<NewEvent> = events
            .into_iter()
            .map(|e| event(self.services, Some(run), None, None, e))
            .collect();
        let outcome = self.services.store.write(move |tx| {
            let from = read_run_state(tx.conn(), &run_id)?;
            match from.transition(to, cause, health) {
                Ok(next) => {
                    f06_core::set_run_state(
                        tx.conn(),
                        run_id.as_str(),
                        &encode(&next)?,
                        services_now,
                    )?;
                    for e in &new_events {
                        tx.append(e)?;
                    }
                    Ok(Ok(next))
                }
                Err(rejected) => {
                    tx.append(&discrepancy(
                        services_now,
                        &run_id,
                        None,
                        "illegal run transition",
                        from.name(),
                        to.name(),
                        &rejected.to_string(),
                    ))?;
                    Ok(Err(rejected))
                }
            }
        })?;
        outcome.map_err(|e| {
            VelaError::of_class(
                ErrorClass::ValidationFailure,
                format!("the run cannot do that now: {e}"),
            )
        })
    }

    /// Creates a worker in `PROVISIONING` and journals `WorkerStarted`, atomically. Provisioning needs integration health to
    /// permit worker starts.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_worker(
        &self,
        run: &RunId,
        worker: WorkerId,
        ticket: &TicketId,
        branch: BranchName,
        worktree: WorktreePath,
        base: CommitSha,
        profile_id: &str,
        health: IntegrationHealth,
    ) -> Result<(), VelaError> {
        let status = WorkerStatus::provision(health).map_err(|e| {
            VelaError::of_class(
                ErrorClass::ValidationFailure,
                format!("a worker cannot start: {e}"),
            )
        })?;
        let now = self.services.now_ms();
        let state = encode(&status.state())?;
        let started = event(
            self.services,
            Some(run),
            Some(ticket),
            Some(&worker),
            Event::WorkerStarted(WorkerStartedPayload {
                branch: branch.clone(),
                worktree: worktree.clone(),
                base_sha: base.clone(),
                profile_id: profile_id.to_owned(),
            }),
        );
        let (run_text, worker_text, ticket_text) = (
            run.as_str().to_owned(),
            worker.as_str().to_owned(),
            ticket.as_str().to_owned(),
        );
        let (branch, worktree, base) = (
            branch.as_str().to_owned(),
            worktree.as_str().to_owned(),
            base.as_str().to_owned(),
        );
        self.services.store.write(move |tx| {
            o01_workers::insert(
                tx.conn(),
                &worker_text,
                &run_text,
                &ticket_text,
                &state,
                now,
            )?;
            o01_workers::set_location(tx.conn(), &worker_text, &branch, &worktree, &base, now)?;
            tx.append(&started).map(|_| ())
        })
    }

    /// The worker's status.
    pub fn worker_status(&self, worker: &WorkerId) -> Result<WorkerStatus, VelaError> {
        self.services.store.read(|c| read_worker_status(c, worker))
    }

    /// Applies `to` to the worker (see [`Runs::transition_run`] for the transaction rules).
    pub async fn transition_worker(
        &self,
        run: &RunId,
        worker: &WorkerId,
        to: WorkerState,
        cause: Cause,
        health: IntegrationHealth,
        events: Vec<Event>,
    ) -> Result<WorkerStatus, VelaError> {
        let now = self.services.now_ms();
        let (worker_id, run_id) = (worker.clone(), run.clone());
        let new_events: Vec<NewEvent> = events
            .into_iter()
            .map(|e| event(self.services, Some(run), None, Some(worker), e))
            .collect();
        let outcome = self.services.store.write(move |tx| {
            let from = read_worker_status(tx.conn(), &worker_id)?;
            match from.transition(to, cause, health) {
                Ok(next) => {
                    o01_workers::set_state(
                        tx.conn(),
                        worker_id.as_str(),
                        &encode(&next.state())?,
                        next.waiting_approval(),
                        now,
                    )?;
                    for e in &new_events {
                        tx.append(e)?;
                    }
                    Ok(Ok(next))
                }
                Err(why) => {
                    tx.append(&discrepancy(
                        now,
                        &run_id,
                        Some(&worker_id),
                        "illegal worker transition",
                        from.state().name(),
                        to.name(),
                        &why.to_string(),
                    ))?;
                    Ok(Err(why))
                }
            }
        })?;
        outcome.map_err(|e| {
            VelaError::of_class(
                ErrorClass::ValidationFailure,
                format!("the worker cannot do that now: {e}"),
            )
        })
    }

    /// Moves a worker through a legal chain of phases, journaling nothing extra (a convenience for drivers).
    pub async fn advance_worker(
        &self,
        run: &RunId,
        worker: &WorkerId,
        to: WorkerPhase,
        health: IntegrationHealth,
    ) -> Result<WorkerStatus, VelaError> {
        self.transition_worker(
            run,
            worker,
            WorkerState::Active(to),
            Cause::System,
            health,
            Vec::new(),
        )
        .await
    }
}

#[cfg(test)]
mod tests;
