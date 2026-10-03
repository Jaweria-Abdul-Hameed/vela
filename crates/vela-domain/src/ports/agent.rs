//! `AgentAdapter`, composed from capability traits (`ADAPTERS.md` "AgentAdapter", component C05).
//!
//! The core does not know whether execution happens through the Antigravity CLI, an SDK, a daemon or another provider
//! (ADR-006). Everything here is provider-neutral: the provider's own concepts, flags and wording stay inside the adapter
//! crate. [`AgentAdapter`] is the original port name: a supertrait of the three capabilities with a blanket
//! implementation.

use crate::ids::{CommitSha, WorkerId, WorktreePath};

use super::PortFuture;

/// Opaque identifier of a provider conversation (recorded for visibility and resume, ADR-020).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConversationId(pub String);

/// A capability a provider adapter can offer (`ADAPTERS.md` capability contract).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentCapability {
    /// Discover installation, version and the capability set.
    Discovery,
    /// Start a fresh isolated session bound to a worktree.
    FreshSession,
    /// Deliver the task and invoke installed skills or commands.
    TaskDelivery,
    /// Observe session lifecycle (completion and error signals at minimum).
    LifecycleObservation,
    /// Cancel a session Vela created.
    Cancel,
    /// Query session status.
    StatusQuery,
    /// Declare or discover the maximum concurrent sessions.
    ConcurrencyLimit,
    /// Expose a session, process or window identity usable for correlation.
    SessionIdentity,
    /// Report rate-limit and capacity conditions.
    CapacityReporting,
    /// Resume or recover a session.
    Resume,
    /// Inspect (and where supported configure) the native permission posture.
    PostureInspection,
    /// A fresh conversation per in-scope session that is discoverable in the provider's desktop application.
    ConversationVisibility,
}

/// What a provider adapter reports about the installed provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentCapabilities {
    /// Installed version string (informational; capabilities are never inferred from it).
    pub version: String,
    /// The capabilities that were feature-detected.
    pub supported: Vec<AgentCapability>,
    /// Maximum concurrent sessions, when declared or discovered.
    pub max_concurrent_sessions: Option<u32>,
}

impl AgentCapabilities {
    /// True when `capability` was feature-detected.
    pub fn supports(&self, capability: AgentCapability) -> bool {
        self.supported.contains(&capability)
    }
}

/// Result of the native permission posture check (ADR-009).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionPosture {
    /// The posture meets Vela's requirement.
    Meets,
    /// The posture does not meet it.
    DoesNotMeet,
    /// It could not be determined; treated as not met for Autonomous mode.
    Unknown,
}

/// One turn of a worker or reviewer conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnRequest {
    /// The worker the turn belongs to.
    pub worker: WorkerId,
    /// The worktree the session is bound to.
    pub worktree: WorktreePath,
    /// The prompt to deliver.
    pub prompt: String,
    /// The conversation to resume, or `None` for a fresh one.
    pub conversation: Option<ConversationId>,
    /// Explicit turn timeout in milliseconds (never the provider default).
    pub timeout_ms: u64,
}

/// Normalized events a provider adapter emits during a turn (component C05 outputs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentEvent {
    /// The session started.
    SessionStarted {
        /// The conversation id, when the provider reports one.
        conversation: Option<ConversationId>,
    },
    /// A step was observed.
    StepObserved {
        /// Redacted summary.
        summary: String,
    },
    /// A tool call was denied.
    ToolDenied {
        /// The tool.
        tool: String,
        /// The reason given.
        reason: String,
    },
    /// The session is blocked on an approval.
    ApprovalBlocked {
        /// Redacted description of what needs approval.
        description: String,
    },
    /// The turn finished.
    TurnFinished,
    /// A capacity or rate-limit condition was reported.
    CapacityError {
        /// Redacted message.
        message: String,
    },
    /// Authentication is required.
    AuthRequired {
        /// Redacted message.
        message: String,
    },
    /// The provider indicates the account or service is blocked, suspended, disabled or denied for terms or policy
    /// reasons. Provider-neutral (ADR-006): emitted only from documented or recorded evidence. Never retried, never
    /// worked around, never cleared automatically (`ERROR_HANDLING.md`, `PROVIDER_POLICY_BLOCK`).
    ProviderPolicyBlock {
        /// The documented or recorded evidence, redacted.
        evidence: String,
    },
}

/// Receives events while a turn runs.
pub trait AgentEventSink: Send + Sync {
    /// Called for each event, in order.
    fn on_event(&self, event: AgentEvent);
}

/// The final result of a turn. Outcomes are decided from events, errors and Git state, never from an exit code alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnResult {
    /// The conversation id parsed from the turn, when there is one.
    pub conversation: Option<ConversationId>,
    /// True when a `TurnFinished` event was observed.
    pub finished: bool,
}

/// Session status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    /// No turn is running.
    Idle,
    /// A turn is running.
    Running,
    /// The status could not be determined.
    Unknown,
}

/// A request for an authoritative review in a fresh reviewer session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewRequest {
    /// The worktree under review.
    pub worktree: WorktreePath,
    /// The fixed point the diff is taken against.
    pub fixed_point: CommitSha,
    /// The head under review.
    pub reviewed_head: CommitSha,
    /// The reviewer prompt (Vela's prompt contract).
    pub prompt: String,
    /// Explicit timeout in milliseconds.
    pub timeout_ms: u64,
}

/// The reviewer's raw output, classified by Vela afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewReport {
    /// The reviewer's output text.
    pub output: String,
    /// The reviewer conversation id, when there is one.
    pub conversation: Option<ConversationId>,
}

/// Running turns and cancelling sessions.
pub trait AgentSession: Send + Sync {
    /// Runs one turn, streaming events to `sink`.
    fn run_turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        sink: &'a dyn AgentEventSink,
    ) -> PortFuture<'a, TurnResult>;
    /// Cancels the session of `worker`. Cancelling an idle or unknown session succeeds.
    fn cancel<'a>(&'a self, worker: &'a WorkerId) -> PortFuture<'a, ()>;
    /// Status of the session of `worker`.
    fn status<'a>(&'a self, worker: &'a WorkerId) -> PortFuture<'a, SessionStatus>;
}

/// Capability discovery and the per-worker profile.
pub trait AgentProfile: Send + Sync {
    /// Feature-detects the installed provider.
    fn capabilities(&self) -> PortFuture<'_, AgentCapabilities>;
    /// Checks the native permission posture.
    fn permission_posture(&self) -> PortFuture<'_, PermissionPosture>;
    /// Prepares the isolated profile for `worker` (settings, rules, hook configuration).
    fn provision_profile<'a>(&'a self, worker: &'a WorkerId) -> PortFuture<'a, ()>;
}

/// Authoritative reviews in a separate, fresh session.
pub trait AgentReviewer: Send + Sync {
    /// Runs the review, streaming events to `sink`.
    fn review<'a>(
        &'a self,
        request: &'a ReviewRequest,
        sink: &'a dyn AgentEventSink,
    ) -> PortFuture<'a, ReviewReport>;
}

/// The original `AgentAdapter` port: every capability.
pub trait AgentAdapter: AgentSession + AgentProfile + AgentReviewer {}

impl<T> AgentAdapter for T where T: ?Sized + AgentSession + AgentProfile + AgentReviewer {}
