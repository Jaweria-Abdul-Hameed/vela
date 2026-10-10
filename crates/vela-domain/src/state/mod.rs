//! State machines (D01): the run machine, the worker machine, intervention kinds, and the ticket / UI node projection.
//!
//! Normative sources: `ORCHESTRATION_ENGINE.md` sections 1.1, 1.2, 3 and 3.2 (run and worker machines, intervention
//! kinds) and `DOMAIN_MODEL.md` (ticket state projection and UI node mapping). Everything here is pure: no I/O, no clock,
//! no provider-specific concept (ADR-006). A transition is a function from a current state, a requested target state and
//! a [`Cause`] to either the target state or a typed [`TransitionError`]; nothing is mutated and nothing is persisted.
//! Every transition not listed in the specification is rejected.
//!
//! Interpretations the specification leaves open are recorded in `docs/project/D01_DECISION_REQUESTS.md`.

#![warn(missing_docs)]

mod common;
mod error;
mod projection;
mod run;
mod worker;

pub use common::{Cause, InterventionKind, PausedReason};
pub use error::{KindRule, Machine, TransitionError};
pub use projection::{
    SubIndicators, TicketProjection, TicketState, UiNode, UiNodeState, project_unstarted,
    project_worker,
};
pub use run::{IntegrationHealth, RunPhase, RunState};
pub use worker::{WorkerPhase, WorkerState, WorkerStatus};
