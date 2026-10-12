//! `AgentAdapter`, composed from capability traits (`ADAPTERS.md` "AgentAdapter", component C05).
//!
//! The core does not know whether execution happens through the Antigravity CLI, an SDK, a daemon or another provider
//! (ADR-006). Everything here is provider-neutral: the provider's own concepts, flags and wording stay inside the adapter
//! crate. [`AgentAdapter`] is the original port name: a supertrait of the three capabilities with a blanket
//! implementation.

use std::num::NonZeroU64;

use crate::ids::{CommitSha, IdError, WorkerId, WorktreePath};

use super::PortFuture;

/// Which kind of session a [`SessionRef`] names. Workers and authoritative reviewers run in separate sessions with
/// separate profiles (C05: "reviewer in a separate profile"), so every operation that targets a session names both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionRole {
    /// The ticket worker's implementation session.
    Worker,
    /// The fresh authoritative-reviewer session for the worker's ticket.
    Reviewer,
}

/// Identifies one session Vela created: the worker it belongs to and its role. Stop All, cancellation, status and
/// profile provisioning address sessions through it, so a running review can be cancelled like any other session.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionRef {
    /// The worker (one ticket execution attempt) the session belongs to.
    pub worker: WorkerId,
    /// Which of the worker's sessions.
    pub role: SessionRole,
}

impl SessionRef {
    /// The worker's implementation session.
    pub fn worker(worker: WorkerId) -> Self {
        Self {
            worker,
            role: SessionRole::Worker,
        }
    }

    /// The worker's reviewer session.
    pub fn reviewer(worker: WorkerId) -> Self {
        Self {
            worker,
            role: SessionRole::Reviewer,
        }
    }
}

/// Identifier of a provider conversation (recorded for visibility and resume, ADR-020).
///
/// The value comes from provider output and is later passed back to the provider to resume the conversation, so it is
/// validated where it enters Vela: adapters build it with [`ConversationId::new`] and treat an error as a malformed
/// event (the turn fails with evidence). Only `[A-Za-z0-9._-]`, starting with a letter or digit, at most 128 bytes, is
/// accepted, so a stored id can never be read as a command-line option or carry whitespace or control characters. The
/// type has no unchecked constructor and every `conversation` field in this module uses it, so a malformed id cannot be
/// propagated into a later [`TurnRequest`]. If the real provider format turns out to need more characters, widen the
/// rule here (the spike for conversation visibility, ADR-020, records the observed format).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConversationId(String);

impl ConversationId {
    /// Validates provider-reported text as a conversation id.
    pub fn new(value: impl Into<String>) -> Result<Self, IdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(IdError::Empty);
        }
        if value.len() > 128 {
            return Err(IdError::TooLong { max: 128 });
        }
        if let Some(found) = value
            .chars()
            .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')))
        {
            return Err(IdError::InvalidCharacter { found });
        }
        if !value.starts_with(|c: char| c.is_ascii_alphanumeric()) {
            return Err(IdError::Malformed(
                "a conversation id must start with a letter or digit",
            ));
        }
        Ok(Self(value))
    }

    /// The id as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ConversationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

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
    /// Explicit turn timeout in milliseconds. It can never be zero: the provider's own default of `0s` means "wait for the
    /// turn without limit" (`ERROR_HANDLING.md`), which Vela never asks for.
    pub timeout_ms: NonZeroU64,
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
    /// The agent's final text (untrusted; the worker completion block is parsed from it).
    pub response: String,
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
    /// The worker whose work is reviewed; the review runs as that worker's [`SessionRole::Reviewer`] session.
    pub worker: WorkerId,
    /// The worktree under review.
    pub worktree: WorktreePath,
    /// The fixed point the diff is taken against.
    pub fixed_point: CommitSha,
    /// The head under review.
    pub reviewed_head: CommitSha,
    /// The reviewer prompt (Vela's prompt contract).
    pub prompt: String,
    /// Explicit timeout in milliseconds (never zero, see [`TurnRequest::timeout_ms`]).
    pub timeout_ms: NonZeroU64,
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
    /// Cancels `session` (a worker or reviewer session). Cancelling an idle or unknown session succeeds.
    fn cancel<'a>(&'a self, session: &'a SessionRef) -> PortFuture<'a, ()>;
    /// Status of `session`.
    fn status<'a>(&'a self, session: &'a SessionRef) -> PortFuture<'a, SessionStatus>;
}

/// Capability discovery and the per-worker profile.
pub trait AgentProfile: Send + Sync {
    /// Feature-detects the installed provider.
    fn capabilities(&self) -> PortFuture<'_, AgentCapabilities>;
    /// Checks the native permission posture.
    fn permission_posture(&self) -> PortFuture<'_, PermissionPosture>;
    /// Prepares the isolated profile for `session` (settings, rules, hook configuration); worker and reviewer
    /// sessions have separate profiles.
    fn provision_profile<'a>(&'a self, session: &'a SessionRef) -> PortFuture<'a, ()>;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_ids_accept_provider_style_ids_and_reject_anything_that_could_be_an_option_or_injection()
     {
        for ok in [
            "c-1",
            "4f6a2c1e-9b7d-4a55-8e3f-0a1b2c3d4e5f",
            "conv_01.v2",
            "A",
        ] {
            assert_eq!(ConversationId::new(ok).unwrap().as_str(), ok);
        }
        for bad in [
            "",
            "-x",
            "--dangerous-flag=1",
            "--conversation",
            ".hidden",
            "a b",
            "a\tb",
            "a\nb",
            "a\0b",
            "a/b",
            "a\\b",
            "a:b",
            "a;b",
            "a$b",
            "a`b",
            "é",
        ] {
            assert!(ConversationId::new(bad).is_err(), "{bad:?}");
        }
        assert!(ConversationId::new("x".repeat(128)).is_ok());
        assert_eq!(
            ConversationId::new("x".repeat(129)),
            Err(IdError::TooLong { max: 128 })
        );
    }

    #[test]
    fn a_conversation_id_can_only_reach_a_later_request_through_validation() {
        // Provider output is untrusted: the only way to get a `ConversationId` is `new`, so a request that resumes a
        // reported conversation can only carry a validated id.
        let reported = "--upload-pack=evil";
        let resume = ConversationId::new(reported).ok();
        assert!(resume.is_none());
        let request = TurnRequest {
            worker: WorkerId::new("w-1").unwrap(),
            worktree: WorktreePath::new("/repo").unwrap(),
            prompt: String::new(),
            conversation: resume,
            timeout_ms: NonZeroU64::MIN,
        };
        assert!(request.conversation.is_none());
    }
}
