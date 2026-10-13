//! Ports: the traits through which the core reaches the outside world (F02; `IMPLEMENTATION_ARCHITECTURE.md` section 3).
//!
//! `AgentAdapter`, `GitAdapter`, `IssueTrackerAdapter`, `ProcessRunner`, `Notifier`, `ApprovalSource`,
//! `ApprovalDeliveryAdapter`, `PowerManager`, `SessionProbe`, `Clock` and `IdGenerator`. Concrete adapters live in other
//! crates and are injected; this crate has no I/O and no async runtime.
//!
//! These traits are a central shared contract: later tickets must not change a signature without coordination. They are
//! dyn-compatible (asynchronous methods return a boxed [`PortFuture`], not `async fn`), every fallible method returns the
//! typed [`VelaError`](crate::errors::VelaError), and `GitAdapter` and `AgentAdapter` are composed from capability traits
//! so that each implementing ticket edits its own impl block.

#![warn(missing_docs)]

mod agent;
mod approval;
mod basic;
mod git;
mod process;
mod tracker;

use std::future::Future;
use std::pin::Pin;

use crate::errors::VelaError;

pub use agent::{
    AgentAdapter, AgentCapabilities, AgentCapability, AgentEvent, AgentEventSink, AgentProfile,
    AgentReviewer, AgentSession, ConversationId, PermissionPosture, ReviewReport, ReviewRequest,
    SessionRef, SessionRole, SessionStatus, TurnRequest, TurnResult,
};
pub use approval::{
    ApprovalDeliveryAdapter, ApprovalObservation, ApprovalSource, DeliveryCapability,
    DeliveryDecision, DeliveryRequest, DeliveryResult, DeliveryTier, EvidenceTier,
};
pub use basic::{
    Clock, IdGenerator, KeepAwakeToken, Notification, Notifier, PowerManager, SessionAvailability,
    SessionProbe,
};
pub use git::{
    CheckpointMessage, DiffSummary, FetchOutcome, GitAdapter, GitCheckpoints, GitContext,
    GitHistory, GitMerges, GitRead, GitRemote, GitWorktrees, MergeOutcome, PushOutcome, RemoteName,
    RepoStatus, RepoTrust, RevisionSpec, Trailer, WorktreeInfo, WorktreeRequest,
};
pub use process::{
    CommandSpec, OutputChunk, OutputSink, OutputStream, ProcessOutcome, ProcessRunner,
};
pub use tracker::{IssueTrackerAdapter, PullRequestPort, PullRequestRequest, TrackerTicket};

/// The future every asynchronous port method returns: boxed (so ports are dyn-compatible), `Send`, and fallible with
/// the typed error.
pub type PortFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, VelaError>> + Send + 'a>>;

/// An already-completed [`PortFuture`], for fakes and for adapters whose work is synchronous.
pub fn ready<'a, T: Send + 'a>(result: Result<T, VelaError>) -> PortFuture<'a, T> {
    Box::pin(std::future::ready(result))
}

#[cfg(test)]
mod tests;
