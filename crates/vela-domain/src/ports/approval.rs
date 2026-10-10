//! `ApprovalSource` and `ApprovalDeliveryAdapter` (`ADAPTERS.md`; components C04 and C06).
//!
//! An approval UI is a transport surface, not the authority: these ports observe and deliver, and the policy engine
//! decides (ADR-009). Adapters never approve a control merely because a button labeled "Approve" exists somewhere.

use super::PortFuture;

/// How strong the evidence behind an observation is (ADR-009).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EvidenceTier {
    /// Weak or truncated evidence; the decision defaults to ASK.
    Weak,
    /// Assembled from a documented surface but not independently confirmed.
    Observed,
    /// Bound to a Vela-created session by a verified identity.
    Bound,
}

/// A pending approval observed by a source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalObservation {
    /// Identifier of the observed request, unique per source.
    pub source_id: String,
    /// The session, process or window identity it was correlated to, if any.
    pub session_identity: Option<String>,
    /// The assembled command or operation text (redacted).
    pub operation: String,
    /// The working directory shown, if any.
    pub working_directory: Option<String>,
    /// The options the surface offers.
    pub options: Vec<String>,
    /// Strength of the evidence.
    pub evidence: EvidenceTier,
}

/// Observes pending approvals (hook spool records, or Desktop cards when consented).
pub trait ApprovalSource: Send + Sync {
    /// The next pending observation, or `None` when there is none right now.
    fn next_observation(&self) -> PortFuture<'_, Option<ApprovalObservation>>;
}

/// A delivery tier (ADR-007, ADR-010). Tiers two and three require the user's consent for guarded UI automation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DeliveryTier {
    /// The provider's native permission mechanism.
    Native,
    /// Windows UI Automation.
    UiAutomation,
    /// Guarded visual compatibility path, last resort.
    Visual,
}

/// What a delivery adapter can do right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeliveryCapability {
    /// The adapter's tier.
    pub tier: DeliveryTier,
    /// True when the adapter can deliver at this moment.
    pub available: bool,
}

/// The decision to deliver. Persistent options are never selectable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryDecision {
    /// Allow this single request.
    AllowOnce,
    /// Refuse this request.
    Deny,
}

/// A request to deliver a decision to the surface that showed `observation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryRequest {
    /// The observation the decision answers; the adapter re-correlates before acting.
    pub observation: ApprovalObservation,
    /// The decision.
    pub decision: DeliveryDecision,
}

/// The result of a delivery attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryResult {
    /// True when the resulting state transition was observed (card cleared, progress seen).
    pub verified: bool,
    /// Non-secret detail for the journal.
    pub detail: String,
}

/// Delivers an approval decision to an approval surface.
pub trait ApprovalDeliveryAdapter: Send + Sync {
    /// Detects whether this adapter can deliver now.
    fn detect(&self) -> PortFuture<'_, DeliveryCapability>;
    /// Delivers `request` and verifies the resulting state transition. Fails closed, with a typed approval error, on any
    /// ambiguity.
    fn deliver<'a>(&'a self, request: &'a DeliveryRequest) -> PortFuture<'a, DeliveryResult>;
    /// Cancels a delivery in progress for `observation`.
    fn cancel<'a>(&'a self, observation: &'a ApprovalObservation) -> PortFuture<'a, ()>;
}
