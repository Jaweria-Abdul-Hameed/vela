//! The worker state machine (`ORCHESTRATION_ENGINE.md` section 3) with the `WAITING_APPROVAL` overlay.

use serde::{Deserialize, Serialize};

use super::common::{Cause, InterventionKind, PausedReason};
use super::error::{KindRule, Machine, TransitionError};

/// A worker state that carries no payload and can be recorded as a `resume_state`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkerPhase {
    /// The worktree and profile are being provisioned.
    Provisioning,
    /// A fresh session loads the ticket context.
    ContextLoading,
    /// The agent implements the ticket.
    Implementing,
    /// Focused validation of the implementation.
    FocusedValidation,
    /// A checkpoint commit is being made.
    Checkpointing,
    /// The review loop runs against the checkpoint.
    Reviewing,
    /// The agent fixes review findings.
    Fixing,
    /// Validation after fixes.
    Validating,
    /// Reviewed and waiting for the merge lane.
    ReadyToMerge,
    /// In the merge lane.
    Merging,
    /// Resolving a textual conflict (ADR-011); entered from `MERGING`.
    ConflictResolution,
    /// Integration validation after the merge.
    IntegrationValidation,
}

impl WorkerPhase {
    /// Every phase, in lifecycle order.
    pub const ALL: [Self; 12] = [
        Self::Provisioning,
        Self::ContextLoading,
        Self::Implementing,
        Self::FocusedValidation,
        Self::Checkpointing,
        Self::Reviewing,
        Self::Fixing,
        Self::Validating,
        Self::ReadyToMerge,
        Self::Merging,
        Self::ConflictResolution,
        Self::IntegrationValidation,
    ];

    /// The specification name (also the wire form).
    pub const fn name(self) -> &'static str {
        match self {
            Self::Provisioning => "PROVISIONING",
            Self::ContextLoading => "CONTEXT_LOADING",
            Self::Implementing => "IMPLEMENTING",
            Self::FocusedValidation => "FOCUSED_VALIDATION",
            Self::Checkpointing => "CHECKPOINTING",
            Self::Reviewing => "REVIEWING",
            Self::Fixing => "FIXING",
            Self::Validating => "VALIDATING",
            Self::ReadyToMerge => "READY_TO_MERGE",
            Self::Merging => "MERGING",
            Self::ConflictResolution => "CONFLICT_RESOLUTION",
            Self::IntegrationValidation => "INTEGRATION_VALIDATION",
        }
    }

    /// Whether an agent session drives this state, so `WAITING_APPROVAL` can overlay it (decision request DR-D01-5).
    pub const fn is_agent_driven(self) -> bool {
        matches!(
            self,
            Self::ContextLoading
                | Self::Implementing
                | Self::Reviewing
                | Self::Fixing
                | Self::ConflictResolution
        )
    }
}

/// The transitions between payload-free worker phases (section 3), one entry per arrow.
const PHASE_EDGES: [(WorkerPhase, WorkerPhase); 14] = [
    (WorkerPhase::Provisioning, WorkerPhase::ContextLoading),
    (WorkerPhase::ContextLoading, WorkerPhase::Implementing),
    (WorkerPhase::Implementing, WorkerPhase::FocusedValidation),
    (WorkerPhase::FocusedValidation, WorkerPhase::Checkpointing),
    (WorkerPhase::Checkpointing, WorkerPhase::Reviewing),
    (WorkerPhase::Reviewing, WorkerPhase::Fixing),
    (WorkerPhase::Fixing, WorkerPhase::Validating),
    (WorkerPhase::Validating, WorkerPhase::Checkpointing),
    (WorkerPhase::Reviewing, WorkerPhase::ReadyToMerge),
    (WorkerPhase::ReadyToMerge, WorkerPhase::Merging),
    (WorkerPhase::Merging, WorkerPhase::ConflictResolution),
    (WorkerPhase::ConflictResolution, WorkerPhase::Checkpointing),
    (WorkerPhase::Merging, WorkerPhase::IntegrationValidation),
    // Section 5 step 7: a failed integration validation discards the merge and returns the worker to FIXING.
    (WorkerPhase::IntegrationValidation, WorkerPhase::Fixing),
];

/// A worker state. `PAUSED` and `NEEDS_HUMAN` record the `resume_state` they return to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkerState {
    /// A payload-free, resumable state.
    Active(WorkerPhase),
    /// Paused for a reason (`CAPACITY` projects to `RATE_LIMITED`).
    Paused {
        /// Why the worker is paused.
        reason: PausedReason,
        /// The state the worker returns to on resume.
        resume_state: WorkerPhase,
    },
    /// The single human-gated worker state.
    NeedsHuman {
        /// What kind of decision.
        kind: InterventionKind,
        /// The state the worker returns to once resolved.
        resume_state: WorkerPhase,
    },
    /// Terminal: the merge was validated.
    Complete,
    /// Terminal: the worker failed.
    Failed,
    /// Terminal: the worker was cancelled.
    Cancelled,
}

impl WorkerState {
    /// The specification name of the state (without payload).
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Active(phase) => phase.name(),
            Self::Paused { .. } => "PAUSED",
            Self::NeedsHuman { .. } => "NEEDS_HUMAN",
            Self::Complete => "COMPLETE",
            Self::Failed => "FAILED",
            Self::Cancelled => "CANCELLED",
        }
    }

    /// Whether no transition leaves this state.
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Complete | Self::Failed | Self::Cancelled)
    }

    /// The `resume_state` recorded by `PAUSED` or `NEEDS_HUMAN`.
    pub const fn resume_state(&self) -> Option<WorkerPhase> {
        match self {
            Self::Paused { resume_state, .. } | Self::NeedsHuman { resume_state, .. } => {
                Some(*resume_state)
            }
            _ => None,
        }
    }

    /// Whether the state is one of the exceptional targets any non-terminal state can reach (section 3.1).
    pub const fn is_exceptional(&self) -> bool {
        matches!(
            self,
            Self::Paused { .. } | Self::NeedsHuman { .. } | Self::Failed | Self::Cancelled
        )
    }

    /// Every well-formed worker state (payload combinations included, worker-level kinds only).
    pub fn all() -> Vec<Self> {
        let mut states = Vec::new();
        for phase in WorkerPhase::ALL {
            states.push(Self::Active(phase));
            for reason in PausedReason::ALL {
                states.push(Self::Paused {
                    reason,
                    resume_state: phase,
                });
            }
            for kind in InterventionKind::ALL {
                if kind.is_worker_level() {
                    states.push(Self::NeedsHuman {
                        kind,
                        resume_state: phase,
                    });
                }
            }
        }
        states.extend([Self::Complete, Self::Failed, Self::Cancelled]);
        states
    }

    /// Applies one transition. Returns the new state (equal to `to`) or a typed error. `cause` matters when leaving a
    /// state that waits for a human: a resume from `NEEDS_HUMAN` needs `Cause::UserDecision` (a `PROVIDER_POLICY_BLOCK`
    /// resolves only that way), as does a resume from `PAUSED` unless the reason is automatic (`CAPACITY`,
    /// `INTERACTIVE_SESSION_UNAVAILABLE`).
    pub fn transition(self, to: Self, cause: Cause) -> Result<Self, TransitionError> {
        if self.is_terminal() {
            return Err(TransitionError::Terminal {
                machine: Machine::Worker,
                from: self.name(),
            });
        }
        if let Self::NeedsHuman { kind, resume_state } = self
            && kind.is_held()
        {
            return match to {
                Self::Cancelled | Self::Failed => Ok(to),
                Self::Active(phase) if phase == resume_state => require_user(cause).map(|()| to),
                _ => Err(TransitionError::ProviderPolicyBlockHeld {
                    machine: Machine::Worker,
                    to: to.name(),
                }),
            };
        }
        match (self, to) {
            (_, Self::Failed | Self::Cancelled) => Ok(to),
            (Self::Active(WorkerPhase::IntegrationValidation), Self::Complete) => Ok(to),
            (Self::Active(from), Self::Active(target)) if PHASE_EDGES.contains(&(from, target)) => {
                Ok(to)
            }
            (Self::Active(from), Self::Paused { resume_state, .. }) => same(to, from, resume_state),
            (Self::Active(from), Self::NeedsHuman { kind, resume_state }) => {
                same(to, from, resume_state)?;
                if kind.is_worker_level() {
                    Ok(to)
                } else {
                    Err(TransitionError::KindNotAllowed {
                        machine: Machine::Worker,
                        kind,
                        rule: KindRule::WrongLevel,
                    })
                }
            }
            (
                Self::Paused {
                    resume_state: held, ..
                },
                Self::NeedsHuman { kind, resume_state },
            ) => {
                same(to, held, resume_state)?;
                if kind.is_worker_level() {
                    Ok(to)
                } else {
                    Err(TransitionError::KindNotAllowed {
                        machine: Machine::Worker,
                        kind,
                        rule: KindRule::WrongLevel,
                    })
                }
            }
            (
                Self::Paused {
                    reason,
                    resume_state: held,
                },
                Self::Active(target),
            ) => {
                same(to, held, target)?;
                if reason.resumes_automatically() {
                    Ok(to)
                } else {
                    require_user(cause).map(|()| to)
                }
            }
            (
                Self::NeedsHuman {
                    resume_state: held, ..
                },
                Self::Active(target),
            ) => {
                same(to, held, target)?;
                require_user(cause).map(|()| to)
            }
            _ => Err(TransitionError::Illegal {
                machine: Machine::Worker,
                from: self.name(),
                to: to.name(),
            }),
        }
    }
}

/// A resume out of a human-gated state needs an explicit user decision.
fn require_user(cause: Cause) -> Result<(), TransitionError> {
    if cause == Cause::UserDecision {
        Ok(())
    } else {
        Err(TransitionError::ExplicitDecisionRequired {
            machine: Machine::Worker,
        })
    }
}

/// `resume_state` consistency: `actual` must equal `expected`.
fn same(
    to: WorkerState,
    expected: WorkerPhase,
    actual: WorkerPhase,
) -> Result<WorkerState, TransitionError> {
    if expected == actual {
        Ok(to)
    } else {
        Err(TransitionError::ResumeStateMismatch {
            machine: Machine::Worker,
            to: to.name(),
            expected: expected.name(),
            actual: actual.name(),
        })
    }
}

/// A worker's state together with the `WAITING_APPROVAL` overlay (section 3.1): the overlay is not a state; the worker
/// keeps its state (its own `resume_state`) while waiting. The overlay exists only on an agent-driven state, so it ends
/// when the worker leaves that state for `PAUSED`, `NEEDS_HUMAN`, `FAILED` or `CANCELLED`; the pending approval request
/// itself is owned by the approval broker, not by this machine. The fields are private so an inconsistent value
/// cannot be built (deserialization is validated the same way).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "WorkerStatusParts")]
pub struct WorkerStatus {
    state: WorkerState,
    waiting_approval: bool,
}

/// The unvalidated form of [`WorkerStatus`] used for deserialization.
#[derive(Deserialize)]
struct WorkerStatusParts {
    state: WorkerState,
    waiting_approval: bool,
}

impl TryFrom<WorkerStatusParts> for WorkerStatus {
    type Error = TransitionError;

    fn try_from(parts: WorkerStatusParts) -> Result<Self, Self::Error> {
        Self::from_parts(parts.state, parts.waiting_approval)
    }
}

impl WorkerStatus {
    /// A status from its parts; the overlay is accepted only on an agent-driven state.
    pub fn from_parts(state: WorkerState, waiting_approval: bool) -> Result<Self, TransitionError> {
        let status = Self::new(state);
        if waiting_approval {
            status.begin_approval_wait()
        } else {
            Ok(status)
        }
    }

    /// The worker state.
    pub const fn state(&self) -> WorkerState {
        self.state
    }

    /// Whether the worker is waiting for an approval decision.
    pub const fn waiting_approval(&self) -> bool {
        self.waiting_approval
    }

    /// A status with no overlay.
    pub const fn new(state: WorkerState) -> Self {
        Self {
            state,
            waiting_approval: false,
        }
    }

    /// Starts waiting for approval. Only an agent-driven state can carry the overlay, once.
    pub fn begin_approval_wait(self) -> Result<Self, TransitionError> {
        if self.waiting_approval {
            return Err(TransitionError::OverlayAlreadyActive);
        }
        match self.state {
            WorkerState::Active(phase) if phase.is_agent_driven() => Ok(Self {
                state: self.state,
                waiting_approval: true,
            }),
            other => Err(TransitionError::OverlayNotAllowed {
                state: other.name(),
            }),
        }
    }

    /// Stops waiting for approval (decision delivered and progress verified); the state is unchanged.
    pub fn end_approval_wait(self) -> Result<Self, TransitionError> {
        if self.waiting_approval {
            Ok(Self::new(self.state))
        } else {
            Err(TransitionError::OverlayNotActive)
        }
    }

    /// Applies a state transition. While waiting for approval only the exceptional targets are reachable (for example
    /// `NEEDS_HUMAN` with kind `APPROVAL_ASK`, recording the prior state as `resume_state`); the overlay then ends.
    /// Approval handling never advances the state by itself.
    pub fn transition(self, to: WorkerState, cause: Cause) -> Result<Self, TransitionError> {
        if self.waiting_approval && !to.is_exceptional() {
            return Err(TransitionError::OverlayBlocksAdvance {
                from: self.state.name(),
                to: to.name(),
            });
        }
        Ok(Self::new(self.state.transition(to, cause)?))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use proptest::prelude::*;

    use super::*;

    use WorkerPhase as P;
    use WorkerState as S;

    fn paused(reason: PausedReason, resume_state: WorkerPhase) -> S {
        S::Paused {
            reason,
            resume_state,
        }
    }

    fn needs(kind: InterventionKind, resume_state: WorkerPhase) -> S {
        S::NeedsHuman { kind, resume_state }
    }

    /// Section 3 written out independently of the implementation, as legal (from, to) pairs for a user decision.
    fn spec_pairs() -> HashSet<(S, S)> {
        let mut legal = HashSet::new();
        let mut add = |from: S, to: S| {
            legal.insert((from, to));
        };
        // The happy path, then the review/fix loop, then the merge-lane detour.
        add(S::Active(P::Provisioning), S::Active(P::ContextLoading));
        add(S::Active(P::ContextLoading), S::Active(P::Implementing));
        add(S::Active(P::Implementing), S::Active(P::FocusedValidation));
        add(S::Active(P::FocusedValidation), S::Active(P::Checkpointing));
        add(S::Active(P::Checkpointing), S::Active(P::Reviewing));
        add(S::Active(P::Reviewing), S::Active(P::Fixing));
        add(S::Active(P::Fixing), S::Active(P::Validating));
        add(S::Active(P::Validating), S::Active(P::Checkpointing));
        add(S::Active(P::Reviewing), S::Active(P::ReadyToMerge));
        add(S::Active(P::ReadyToMerge), S::Active(P::Merging));
        add(S::Active(P::Merging), S::Active(P::ConflictResolution));
        add(
            S::Active(P::ConflictResolution),
            S::Active(P::Checkpointing),
        );
        add(S::Active(P::Merging), S::Active(P::IntegrationValidation));
        add(S::Active(P::IntegrationValidation), S::Active(P::Fixing));
        add(S::Active(P::IntegrationValidation), S::Complete);
        // Every non-terminal state can reach the exceptional states.
        for phase in P::ALL {
            add(S::Active(phase), S::Failed);
            add(S::Active(phase), S::Cancelled);
            for reason in PausedReason::ALL {
                add(S::Active(phase), paused(reason, phase));
                add(paused(reason, phase), S::Active(phase));
                add(paused(reason, phase), S::Failed);
                add(paused(reason, phase), S::Cancelled);
                // A pause can escalate to a human gate (for example a provider policy block found while paused).
                for kind in InterventionKind::ALL {
                    if kind.is_worker_level() {
                        add(paused(reason, phase), needs(kind, phase));
                    }
                }
            }
            for kind in InterventionKind::ALL {
                if kind.is_worker_level() {
                    add(S::Active(phase), needs(kind, phase));
                    add(needs(kind, phase), S::Active(phase));
                    add(needs(kind, phase), S::Cancelled);
                    add(needs(kind, phase), S::Failed);
                }
            }
        }
        legal
    }

    #[test]
    fn every_transition_pair_matches_the_specification_table() {
        let legal = spec_pairs();
        let states = S::all();
        assert_eq!(states.len(), 12 * (1 + 6 + 13) + 3);
        let mut accepted = 0;
        for from in &states {
            for to in &states {
                let result = from.transition(*to, Cause::UserDecision);
                assert_eq!(
                    result.is_ok(),
                    legal.contains(&(*from, *to)),
                    "{from:?} -> {to:?}: {result:?}"
                );
                if let Ok(next) = result {
                    assert_eq!(next, *to);
                    accepted += 1;
                }
            }
        }
        assert_eq!(accepted, legal.len());
    }

    #[test]
    fn run_only_kinds_are_rejected_on_a_worker() {
        for kind in [
            InterventionKind::GraphInvalid,
            InterventionKind::IntegrationUnknown,
            InterventionKind::IntegrationUnhealthy,
        ] {
            assert_eq!(
                S::Active(P::Implementing).transition(needs(kind, P::Implementing), Cause::System),
                Err(TransitionError::KindNotAllowed {
                    machine: Machine::Worker,
                    kind,
                    rule: KindRule::WrongLevel,
                })
            );
        }
    }

    #[test]
    fn conflict_resolution_returns_through_checkpointing_and_reviewing() {
        let mut state = S::Active(P::Merging);
        for next in [
            P::ConflictResolution,
            P::Checkpointing,
            P::Reviewing,
            P::ReadyToMerge,
            P::Merging,
            P::IntegrationValidation,
        ] {
            state = state.transition(S::Active(next), Cause::System).unwrap();
        }
        assert_eq!(
            state.transition(S::Complete, Cause::System),
            Ok(S::Complete)
        );
        // CONFLICT_RESOLUTION is entered only from MERGING and cannot skip back to READY_TO_MERGE.
        assert!(
            S::Active(P::ReadyToMerge)
                .transition(S::Active(P::ConflictResolution), Cause::System)
                .is_err()
        );
        assert!(
            S::Active(P::ConflictResolution)
                .transition(S::Active(P::ReadyToMerge), Cause::System)
                .is_err()
        );
        assert!(
            S::Active(P::ConflictResolution)
                .transition(S::Active(P::Reviewing), Cause::System)
                .is_err()
        );
    }

    #[test]
    fn leaving_a_human_gate_needs_a_user_decision_unless_the_pause_is_automatic() {
        for state in S::all() {
            for to in S::all() {
                let by_user = state.transition(to, Cause::UserDecision);
                let by_system = state.transition(to, Cause::System);
                let automatic = matches!(
                    state,
                    S::Paused { reason, .. } if reason.resumes_automatically()
                );
                let resume = matches!((state, to), (S::Paused { .. } | S::NeedsHuman { .. }, S::Active(_)));
                if resume && by_user.is_ok() && !automatic {
                    assert_eq!(
                        by_system,
                        Err(TransitionError::ExplicitDecisionRequired { machine: Machine::Worker }),
                        "{state:?} -> {to:?}"
                    );
                } else {
                    assert_eq!(by_user, by_system, "{state:?} -> {to:?}");
                }
            }
        }
    }

    #[test]
    fn failed_integration_validation_returns_to_fixing() {
        let mut state = S::Active(P::IntegrationValidation);
        for next in [P::Fixing, P::Validating, P::Checkpointing, P::Reviewing, P::ReadyToMerge, P::Merging] {
            state = state.transition(S::Active(next), Cause::System).unwrap();
        }
        assert_eq!(state, S::Active(P::Merging));
    }

    #[test]
    fn a_capacity_pause_can_escalate_to_a_provider_policy_block() {
        let capacity = paused(PausedReason::Capacity, P::Implementing);
        let block = needs(InterventionKind::ProviderPolicyBlock, P::Implementing);
        assert_eq!(capacity.transition(block, Cause::System), Ok(block));
        // The reverse (de-escalation) is not a transition.
        assert!(block.transition(capacity, Cause::UserDecision).is_err());
    }

    #[cfg(feature = "full")]
    #[test]
    fn status_serde_rejects_an_inconsistent_overlay() {
        let bad = r#"{"state":{"PAUSED":{"reason":"MANUAL","resume_state":"IMPLEMENTING"}},"waiting_approval":true}"#;
        assert!(serde_json::from_str::<WorkerStatus>(bad).is_err());
        let ok = r#"{"state":{"ACTIVE":"IMPLEMENTING"},"waiting_approval":true}"#;
        assert!(serde_json::from_str::<WorkerStatus>(ok).unwrap().waiting_approval());
        assert!(WorkerStatus::from_parts(S::Active(P::Provisioning), true).is_err());
    }

    #[test]
    fn terminal_states_have_no_exits() {
        for terminal in [S::Complete, S::Failed, S::Cancelled] {
            for to in S::all() {
                assert!(matches!(
                    terminal.transition(to, Cause::UserDecision),
                    Err(TransitionError::Terminal { .. })
                ));
            }
        }
    }

    #[test]
    fn provider_policy_block_exits_only_by_explicit_resolve_or_cancel() {
        let block = needs(InterventionKind::ProviderPolicyBlock, P::Implementing);
        let mut exits = Vec::new();
        for to in S::all() {
            for cause in [Cause::System, Cause::UserDecision] {
                if block.transition(to, cause).is_ok() {
                    exits.push((to, cause));
                }
            }
        }
        assert_eq!(
            exits,
            vec![
                (S::Active(P::Implementing), Cause::UserDecision),
                (S::Failed, Cause::System),
                (S::Failed, Cause::UserDecision),
                (S::Cancelled, Cause::System),
                (S::Cancelled, Cause::UserDecision)
            ]
        );
        assert_eq!(
            block.transition(S::Active(P::Implementing), Cause::System),
            Err(TransitionError::ExplicitDecisionRequired {
                machine: Machine::Worker
            })
        );
        for to in [
            S::Complete,
            paused(PausedReason::Capacity, P::Implementing),
            needs(InterventionKind::AuthRequired, P::Implementing),
            S::Active(P::Reviewing),
        ] {
            assert_eq!(
                block.transition(to, Cause::UserDecision),
                Err(TransitionError::ProviderPolicyBlockHeld {
                    machine: Machine::Worker,
                    to: to.name()
                })
            );
        }
    }

    #[test]
    fn waiting_approval_is_an_overlay_that_keeps_the_state() {
        let status = WorkerStatus::new(S::Active(P::Implementing));
        let waiting = status.begin_approval_wait().unwrap();
        assert!(waiting.waiting_approval());
        assert_eq!(waiting.state(), S::Active(P::Implementing));
        assert_eq!(
            waiting.begin_approval_wait(),
            Err(TransitionError::OverlayAlreadyActive)
        );
        assert_eq!(
            status.end_approval_wait(),
            Err(TransitionError::OverlayNotActive)
        );
        assert_eq!(waiting.end_approval_wait(), Ok(status));
        // It cannot advance the worker by itself.
        assert_eq!(
            waiting.transition(S::Active(P::FocusedValidation), Cause::System),
            Err(TransitionError::OverlayBlocksAdvance {
                from: "IMPLEMENTING",
                to: "FOCUSED_VALIDATION"
            })
        );
        // ASK: NEEDS_HUMAN(APPROVAL_ASK) keeps the prior state as resume_state and clears the overlay.
        let asked = waiting
            .transition(
                needs(InterventionKind::ApprovalAsk, P::Implementing),
                Cause::System,
            )
            .unwrap();
        assert_eq!(
            asked,
            WorkerStatus::new(needs(InterventionKind::ApprovalAsk, P::Implementing))
        );
        // After the decision the worker resumes the state it had.
        assert_eq!(
            asked.transition(S::Active(P::Implementing), Cause::UserDecision),
            Ok(status)
        );
        // Interactive session lost while waiting: PAUSED with that reason.
        assert!(
            waiting
                .transition(
                    paused(PausedReason::InteractiveSessionUnavailable, P::Implementing),
                    Cause::System
                )
                .is_ok()
        );
    }

    #[test]
    fn overlay_applies_only_to_agent_driven_states() {
        for phase in P::ALL {
            let result = WorkerStatus::new(S::Active(phase)).begin_approval_wait();
            assert_eq!(result.is_ok(), phase.is_agent_driven(), "{phase:?}");
        }
        for state in [
            S::Complete,
            S::Failed,
            S::Cancelled,
            paused(PausedReason::Manual, P::Implementing),
        ] {
            assert!(matches!(
                WorkerStatus::new(state).begin_approval_wait(),
                Err(TransitionError::OverlayNotAllowed { .. })
            ));
        }
    }

    proptest! {
        /// Walking only legal transitions (with the overlay toggled at random) never leaves the enumerated state
        /// space, and a held block is left only through the explicit exits.
        #[test]
        fn legal_sequences_stay_in_the_mapped_state_space(picks in proptest::collection::vec((any::<prop::sample::Index>(), any::<bool>(), 0u8..4), 0..200)) {
            let universe = S::all();
            let known: HashSet<S> = universe.iter().copied().collect();
            let mut status = WorkerStatus::new(S::Active(P::Provisioning));
            for (index, user, action) in picks {
                let before = status;
                let cause = if user { Cause::UserDecision } else { Cause::System };
                let next = match action {
                    0 => status.begin_approval_wait().ok(),
                    1 => status.end_approval_wait().ok(),
                    _ => {
                        let legal: Vec<S> = universe
                            .iter()
                            .copied()
                            .filter(|target| status.transition(*target, cause).is_ok())
                            .collect();
                        if legal.is_empty() {
                            prop_assert!(status.state.is_terminal() || status.waiting_approval);
                            None
                        } else {
                            status.transition(legal[index.index(legal.len())], cause).ok()
                        }
                    }
                };
                let Some(next) = next else { continue };
                prop_assert!(known.contains(&next.state));
                prop_assert_eq!(WorkerStatus::from_parts(next.state, next.waiting_approval), Ok(next));
                if let S::NeedsHuman { kind: InterventionKind::ProviderPolicyBlock, resume_state } = before.state
                    && next.state != before.state
                {
                    prop_assert!(
                        matches!(next.state, S::Cancelled | S::Failed)
                            || (next.state == S::Active(resume_state) && user)
                    );
                }
                status = next;
            }
        }
    }
}
