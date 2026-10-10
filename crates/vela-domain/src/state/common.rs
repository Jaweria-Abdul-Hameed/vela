//! Types shared by the run and worker machines: pause reasons, intervention kinds, and the cause of a transition.

use serde::{Deserialize, Serialize};

/// Why a run or a worker is `PAUSED` (`ORCHESTRATION_ENGINE.md` section 1.1 and `DOMAIN_MODEL.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PausedReason {
    /// The user paused.
    Manual,
    /// Quiescent after Stop All.
    StopAll,
    /// Recovery completed and awaits an explicit Resume.
    RecoveredAwaitingResume,
    /// Capacity limits (a worker paused for this reason projects to `RATE_LIMITED`).
    Capacity,
    /// The interactive session needed for UI-automation delivery is unavailable (ADR-012).
    InteractiveSessionUnavailable,
    /// Recovery needs the user's confirmation before continuing.
    AwaitingRecoveryConfirmation,
}

impl PausedReason {
    /// Every reason, in specification order.
    pub const ALL: [Self; 6] = [
        Self::Manual,
        Self::StopAll,
        Self::RecoveredAwaitingResume,
        Self::Capacity,
        Self::InteractiveSessionUnavailable,
        Self::AwaitingRecoveryConfirmation,
    ];

    /// Whether a pause for this reason ends without a human decision (capacity returns, the interactive session
    /// comes back). Every other reason waits for an explicit user resume.
    pub const fn resumes_automatically(self) -> bool {
        matches!(self, Self::Capacity | Self::InteractiveSessionUnavailable)
    }

    /// The specification name (also the wire form).
    pub const fn name(self) -> &'static str {
        match self {
            Self::Manual => "MANUAL",
            Self::StopAll => "STOP_ALL",
            Self::RecoveredAwaitingResume => "RECOVERED_AWAITING_RESUME",
            Self::Capacity => "CAPACITY",
            Self::InteractiveSessionUnavailable => "INTERACTIVE_SESSION_UNAVAILABLE",
            Self::AwaitingRecoveryConfirmation => "AWAITING_RECOVERY_CONFIRMATION",
        }
    }
}

/// The kind of a `NEEDS_HUMAN` state (`ORCHESTRATION_ENGINE.md` section 3.2, plus the run-level kinds named in
/// sections 1.1 and 1.2). The kind is persisted and displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InterventionKind {
    /// Ticket contradictory, impossible, or underspecified in an externally visible way.
    Ambiguity,
    /// Review loop cap reached, or reviews oscillate.
    ReviewStalled,
    /// Policy returned `ASK` for a request.
    ApprovalAsk,
    /// Repeated equivalent approval with no progress past the threshold.
    ApprovalStalled,
    /// A decision cannot be delivered by any permitted tier.
    ApprovalUndeliverable,
    /// A `DENY` is repeated beyond the threshold, or the denied operation is required by the ticket.
    PolicyDenied,
    /// A detective control fired (ADR-009 section 4).
    PolicyViolation,
    /// A conflict not resolvable under ADR-011 section 5.
    MergeConflict,
    /// Integration validation failures exceed the fix cap.
    IntegrationRegression,
    /// Credentials or provider authentication needed.
    AuthRequired,
    /// The provider reports the account or service as blocked, suspended, disabled or denied for terms or policy
    /// reasons. Valid at worker and run level. Never auto-resumed: it leaves only through an explicit user decision
    /// (resolve to `resume_state`) or a cancel.
    ProviderPolicyBlock,
    /// Refs, worktree, or issues changed outside Vela.
    ExternalDivergence,
    /// Any other defined human-gated condition.
    Other,
    /// Run level only: the graph is invalid (cycle, malformed issues); entered from `ANALYZING`.
    GraphInvalid,
    /// Run level only: integration health is `UNKNOWN` and the lane's recovery could not restore it.
    IntegrationUnknown,
    /// Run level only: integration health is `UNHEALTHY` and the lane's recovery could not restore it.
    IntegrationUnhealthy,
}

impl InterventionKind {
    /// Every kind: the thirteen of section 3.2, then the three run-level kinds.
    pub const ALL: [Self; 16] = [
        Self::Ambiguity,
        Self::ReviewStalled,
        Self::ApprovalAsk,
        Self::ApprovalStalled,
        Self::ApprovalUndeliverable,
        Self::PolicyDenied,
        Self::PolicyViolation,
        Self::MergeConflict,
        Self::IntegrationRegression,
        Self::AuthRequired,
        Self::ProviderPolicyBlock,
        Self::ExternalDivergence,
        Self::Other,
        Self::GraphInvalid,
        Self::IntegrationUnknown,
        Self::IntegrationUnhealthy,
    ];

    /// The specification name (also the wire form).
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ambiguity => "AMBIGUITY",
            Self::ReviewStalled => "REVIEW_STALLED",
            Self::ApprovalAsk => "APPROVAL_ASK",
            Self::ApprovalStalled => "APPROVAL_STALLED",
            Self::ApprovalUndeliverable => "APPROVAL_UNDELIVERABLE",
            Self::PolicyDenied => "POLICY_DENIED",
            Self::PolicyViolation => "POLICY_VIOLATION",
            Self::MergeConflict => "MERGE_CONFLICT",
            Self::IntegrationRegression => "INTEGRATION_REGRESSION",
            Self::AuthRequired => "AUTH_REQUIRED",
            Self::ProviderPolicyBlock => "PROVIDER_POLICY_BLOCK",
            Self::ExternalDivergence => "EXTERNAL_DIVERGENCE",
            Self::Other => "OTHER",
            Self::GraphInvalid => "GRAPH_INVALID",
            Self::IntegrationUnknown => "INTEGRATION_UNKNOWN",
            Self::IntegrationUnhealthy => "INTEGRATION_UNHEALTHY",
        }
    }

    /// Whether a worker can be `NEEDS_HUMAN` with this kind: exactly the thirteen kinds of section 3.2.
    pub const fn is_worker_level(self) -> bool {
        !matches!(
            self,
            Self::GraphInvalid | Self::IntegrationUnknown | Self::IntegrationUnhealthy
        )
    }

    /// Whether this kind never leaves automatically (only an explicit user resolve to `resume_state`, or a cancel).
    pub const fn is_held(self) -> bool {
        matches!(self, Self::ProviderPolicyBlock)
    }
}

/// What caused a transition request. The machines never infer this: the caller states it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Cause {
    /// Vela itself (timers, reconciliation, observation, recovery).
    System,
    /// An explicit user decision (resume, resolve, cancel).
    UserDecision,
}
