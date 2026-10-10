//! The typed error returned by every rejected transition.

use std::fmt;

use super::common::{InterventionKind, PausedReason};

/// Which machine rejected the transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Machine {
    /// The run machine.
    Run,
    /// The worker machine.
    Worker,
}

/// Why an intervention kind was rejected for a transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KindRule {
    /// The kind does not exist at this level (for example `GRAPH_INVALID` on a worker).
    WrongLevel,
    /// `GRAPH_INVALID` is entered only from `ANALYZING`.
    GraphInvalidOnlyFromAnalyzing,
    /// The `ANALYZING` to `NEEDS_HUMAN` transition carries kind `GRAPH_INVALID`.
    AnalyzingRequiresGraphInvalid,
}

/// A rejected transition. Rejection never changes any state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionError {
    /// The specification has no such transition.
    Illegal {
        /// The machine.
        machine: Machine,
        /// The current state name.
        from: &'static str,
        /// The requested state name.
        to: &'static str,
    },
    /// The current state is terminal: no transition leaves it.
    Terminal {
        /// The machine.
        machine: Machine,
        /// The terminal state name.
        from: &'static str,
    },
    /// `resume_state` is inconsistent: entering `PAUSED`, `NEEDS_HUMAN` or `STOPPING` must record the state being left,
    /// and resuming must return exactly to the recorded `resume_state`.
    ResumeStateMismatch {
        /// The machine.
        machine: Machine,
        /// The requested state name.
        to: &'static str,
        /// The state that was required.
        expected: &'static str,
        /// The state that was supplied.
        actual: &'static str,
    },
    /// The intervention kind is not allowed for this transition.
    KindNotAllowed {
        /// The machine.
        machine: Machine,
        /// The rejected kind.
        kind: InterventionKind,
        /// The rule that rejected it.
        rule: KindRule,
    },
    /// A resume or resolve out of a human-gated state (`NEEDS_HUMAN`, or `PAUSED` for a reason that waits for the user,
    /// including every `PROVIDER_POLICY_BLOCK` resolve) was attempted without an explicit user decision.
    ExplicitDecisionRequired {
        /// The machine.
        machine: Machine,
    },
    /// A `PROVIDER_POLICY_BLOCK` state can be left only by an explicit resolve to `resume_state` or a cancel.
    ProviderPolicyBlockHeld {
        /// The machine.
        machine: Machine,
        /// The requested state name.
        to: &'static str,
    },
    /// A run in `STOPPING` that kept a human gate can only return to that gate (or fail).
    HumanGateHeld {
        /// The machine.
        machine: Machine,
        /// The gate kept through `STOPPING`.
        kind: InterventionKind,
        /// The requested state name.
        to: &'static str,
    },
    /// The pause reason is not allowed for this transition (for example `STOP_ALL` is reached only through
    /// `STOPPING`, and `STOPPING` resolves to `PAUSED` only with that reason).
    ReasonNotAllowed {
        /// The machine.
        machine: Machine,
        /// The rejected reason.
        reason: PausedReason,
    },
    /// The transition needs integration health `HEALTHY` (`RUNNING` to `FINAL_REVIEW`, `READY_TO_MERGE` to `MERGING`,
    /// provisioning a new worker).
    IntegrationNotHealthy {
        /// The health value that was supplied.
        health: &'static str,
    },
    /// `WAITING_APPROVAL` can overlay only an agent-driven worker state.
    OverlayNotAllowed {
        /// The state that cannot carry the overlay.
        state: &'static str,
    },
    /// The worker is already waiting for approval.
    OverlayAlreadyActive,
    /// The worker is not waiting for approval.
    OverlayNotActive,
    /// While `WAITING_APPROVAL` the worker cannot advance; only the exceptional states are reachable (approval handling
    /// never advances state by itself).
    OverlayBlocksAdvance {
        /// The state the worker is in.
        from: &'static str,
        /// The requested state name.
        to: &'static str,
    },
}

impl fmt::Display for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Illegal { machine, from, to } => {
                write!(f, "illegal {machine:?} transition {from} -> {to}")
            }
            Self::Terminal { machine, from } => write!(f, "{machine:?} state {from} is terminal"),
            Self::ResumeStateMismatch {
                machine,
                to,
                expected,
                actual,
            } => write!(
                f,
                "{machine:?} transition to {to} needs resume_state {expected}, got {actual}"
            ),
            Self::KindNotAllowed {
                machine,
                kind,
                rule,
            } => {
                write!(
                    f,
                    "{machine:?} intervention kind {} rejected ({rule:?})",
                    kind.name()
                )
            }
            Self::ExplicitDecisionRequired { machine } => {
                write!(
                    f,
                    "{machine:?} resume out of a human-gated state needs an explicit user decision"
                )
            }
            Self::ProviderPolicyBlockHeld { machine, to } => write!(
                f,
                "{machine:?} PROVIDER_POLICY_BLOCK exits only by resolve to resume_state or cancel, not to {to}"
            ),
            Self::HumanGateHeld { machine, kind, to } => write!(
                f,
                "{machine:?} STOPPING keeps gate {} and cannot go to {to}",
                kind.name()
            ),
            Self::ReasonNotAllowed { machine, reason } => {
                write!(
                    f,
                    "{machine:?} pause reason {} not allowed here",
                    reason.name()
                )
            }
            Self::IntegrationNotHealthy { health } => {
                write!(
                    f,
                    "integration health is {health}; the transition needs HEALTHY"
                )
            }
            Self::OverlayNotAllowed { state } => {
                write!(f, "WAITING_APPROVAL cannot overlay {state}")
            }
            Self::OverlayAlreadyActive => write!(f, "worker is already WAITING_APPROVAL"),
            Self::OverlayNotActive => write!(f, "worker is not WAITING_APPROVAL"),
            Self::OverlayBlocksAdvance { from, to } => {
                write!(
                    f,
                    "worker WAITING_APPROVAL in {from} cannot advance to {to}"
                )
            }
        }
    }
}

impl std::error::Error for TransitionError {}
