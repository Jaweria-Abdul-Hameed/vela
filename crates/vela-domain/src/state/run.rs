//! The run state machine (`ORCHESTRATION_ENGINE.md` section 1.1) and integration health (section 1.2).

use serde::{Deserialize, Serialize};

use super::common::{Cause, InterventionKind, PausedReason};
use super::error::{KindRule, Machine, TransitionError};

/// A run state that carries no payload and can be recorded as a `resume_state`: every non-terminal state except
/// `STOPPING`, `PAUSED` and `NEEDS_HUMAN` (which wrap one of these).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RunPhase {
    /// The run exists.
    Created,
    /// Preflight checks run.
    Preflight,
    /// The graph and parallelization snapshot are being built.
    Analyzing,
    /// The graph is built and valid and awaits approval.
    GraphReady,
    /// Approved; waiting for the user to press Build.
    AwaitingStart,
    /// The integration branch and worktree are being created.
    Starting,
    /// Workers are scheduled.
    Running,
    /// Final review of the integrated work.
    FinalReview,
    /// Fixing final review or final validation failures.
    FinalFixing,
    /// Final validation of the integrated work.
    FinalValidation,
}

impl RunPhase {
    /// Every phase, in lifecycle order.
    pub const ALL: [Self; 10] = [
        Self::Created,
        Self::Preflight,
        Self::Analyzing,
        Self::GraphReady,
        Self::AwaitingStart,
        Self::Starting,
        Self::Running,
        Self::FinalReview,
        Self::FinalFixing,
        Self::FinalValidation,
    ];

    /// The specification name (also the wire form).
    pub const fn name(self) -> &'static str {
        match self {
            Self::Created => "CREATED",
            Self::Preflight => "PREFLIGHT",
            Self::Analyzing => "ANALYZING",
            Self::GraphReady => "GRAPH_READY",
            Self::AwaitingStart => "AWAITING_START",
            Self::Starting => "STARTING",
            Self::Running => "RUNNING",
            Self::FinalReview => "FINAL_REVIEW",
            Self::FinalFixing => "FINAL_FIXING",
            Self::FinalValidation => "FINAL_VALIDATION",
        }
    }
}

/// The forward and loop transitions between payload-free phases (section 1.1), one entry per table row.
const PHASE_EDGES: [(RunPhase, RunPhase); 12] = [
    (RunPhase::Created, RunPhase::Preflight),
    (RunPhase::Preflight, RunPhase::Preflight),
    (RunPhase::Preflight, RunPhase::Analyzing),
    (RunPhase::Analyzing, RunPhase::GraphReady),
    (RunPhase::GraphReady, RunPhase::AwaitingStart),
    (RunPhase::AwaitingStart, RunPhase::Starting),
    (RunPhase::Starting, RunPhase::Running),
    (RunPhase::Running, RunPhase::FinalReview),
    (RunPhase::FinalReview, RunPhase::FinalFixing),
    (RunPhase::FinalFixing, RunPhase::FinalReview),
    (RunPhase::FinalReview, RunPhase::FinalValidation),
    (RunPhase::FinalValidation, RunPhase::FinalFixing),
];

/// A run state. `PAUSED`, `NEEDS_HUMAN` and `STOPPING` record the `resume_state` they will return to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RunState {
    /// A payload-free, resumable state.
    Active(RunPhase),
    /// Stop All is quiescing; becomes `PAUSED` or `NEEDS_HUMAN`.
    Stopping {
        /// The state the run was in when Stop All began.
        resume_state: RunPhase,
    },
    /// Paused for a reason.
    Paused {
        /// Why the run is paused.
        reason: PausedReason,
        /// The state the run returns to on resume.
        resume_state: RunPhase,
    },
    /// A human decision is needed.
    NeedsHuman {
        /// What kind of decision.
        kind: InterventionKind,
        /// The state the run returns to once resolved.
        resume_state: RunPhase,
    },
    /// Terminal: validation passed and promotion completed (ADR-014).
    Completed,
    /// Terminal: unrecoverable internal error (the reason is recorded outside the machine; artifacts are preserved).
    Failed,
    /// Terminal: the user cancelled (branches, worktrees and commits are preserved).
    Cancelled,
}

impl RunState {
    /// The specification name of the state (without payload).
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Active(phase) => phase.name(),
            Self::Stopping { .. } => "STOPPING",
            Self::Paused { .. } => "PAUSED",
            Self::NeedsHuman { .. } => "NEEDS_HUMAN",
            Self::Completed => "COMPLETED",
            Self::Failed => "FAILED",
            Self::Cancelled => "CANCELLED",
        }
    }

    /// Whether no transition leaves this state.
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// The `resume_state` recorded by `STOPPING`, `PAUSED` or `NEEDS_HUMAN`.
    pub const fn resume_state(&self) -> Option<RunPhase> {
        match self {
            Self::Stopping { resume_state }
            | Self::Paused { resume_state, .. }
            | Self::NeedsHuman { resume_state, .. } => Some(*resume_state),
            _ => None,
        }
    }

    /// Every well-formed run state (payload combinations included).
    pub fn all() -> Vec<Self> {
        let mut states = Vec::new();
        for phase in RunPhase::ALL {
            states.push(Self::Active(phase));
            states.push(Self::Stopping { resume_state: phase });
            for reason in PausedReason::ALL {
                states.push(Self::Paused {
                    reason,
                    resume_state: phase,
                });
            }
            for kind in InterventionKind::ALL {
                states.push(Self::NeedsHuman {
                    kind,
                    resume_state: phase,
                });
            }
        }
        states.extend([Self::Completed, Self::Failed, Self::Cancelled]);
        states
    }

    /// Applies one transition. Returns the new state (equal to `to`) or a typed error; the specification's transition
    /// table is the only source of legality. `cause` matters only for leaving a `PROVIDER_POLICY_BLOCK`.
    pub fn transition(self, to: Self, cause: Cause) -> Result<Self, TransitionError> {
        if self.is_terminal() {
            return Err(TransitionError::Terminal {
                machine: Machine::Run,
                from: self.name(),
            });
        }
        if let Self::NeedsHuman { kind, resume_state } = self
            && kind.is_held()
        {
            return match to {
                Self::Cancelled => Ok(to),
                Self::Active(phase) if phase == resume_state => {
                    if cause == Cause::UserDecision {
                        Ok(to)
                    } else {
                        Err(TransitionError::ExplicitDecisionRequired { machine: Machine::Run })
                    }
                }
                _ => Err(TransitionError::ProviderPolicyBlockHeld {
                    machine: Machine::Run,
                    to: to.name(),
                }),
            };
        }
        match (self, to) {
            (_, Self::Failed) => Ok(to),
            (Self::Active(RunPhase::FinalValidation), Self::Completed) => Ok(to),
            (Self::Active(from), Self::Active(target)) if PHASE_EDGES.contains(&(from, target)) => Ok(to),
            (Self::Active(from), Self::Stopping { resume_state }) => same(to, from, resume_state),
            (
                Self::Paused { resume_state: held, .. } | Self::NeedsHuman { resume_state: held, .. },
                Self::Stopping { resume_state },
            ) => same(to, held, resume_state),
            (Self::Active(RunPhase::Running), Self::Paused { resume_state, .. }) => {
                same(to, RunPhase::Running, resume_state)
            }
            (
                Self::Active(from @ (RunPhase::Running | RunPhase::Analyzing | RunPhase::Starting)),
                Self::NeedsHuman { kind, resume_state },
            ) => {
                same(to, from, resume_state)?;
                if from == RunPhase::Analyzing && kind != InterventionKind::GraphInvalid {
                    return Err(kind_error(kind, KindRule::AnalyzingRequiresGraphInvalid));
                }
                graph_invalid_rule(kind, from)?;
                Ok(to)
            }
            (Self::Stopping { resume_state: held }, Self::Paused { resume_state, .. }) => same(to, held, resume_state),
            (Self::Stopping { resume_state: held }, Self::NeedsHuman { kind, resume_state }) => {
                same(to, held, resume_state)?;
                graph_invalid_rule(kind, held)?;
                Ok(to)
            }
            (
                Self::Paused { resume_state: held, .. } | Self::NeedsHuman { resume_state: held, .. },
                Self::Active(target),
            ) => same(to, held, target),
            (Self::Paused { .. } | Self::NeedsHuman { .. }, Self::Cancelled) => Ok(to),
            _ => Err(TransitionError::Illegal {
                machine: Machine::Run,
                from: self.name(),
                to: to.name(),
            }),
        }
    }
}

/// `resume_state` consistency: `actual` must equal `expected`.
fn same(to: RunState, expected: RunPhase, actual: RunPhase) -> Result<RunState, TransitionError> {
    if expected == actual {
        Ok(to)
    } else {
        Err(TransitionError::ResumeStateMismatch {
            machine: Machine::Run,
            to: to.name(),
            expected: expected.name(),
            actual: actual.name(),
        })
    }
}

fn kind_error(kind: InterventionKind, rule: KindRule) -> TransitionError {
    TransitionError::KindNotAllowed {
        machine: Machine::Run,
        kind,
        rule,
    }
}

/// `GRAPH_INVALID` belongs to `ANALYZING` only.
fn graph_invalid_rule(kind: InterventionKind, origin: RunPhase) -> Result<(), TransitionError> {
    if kind == InterventionKind::GraphInvalid && origin != RunPhase::Analyzing {
        Err(kind_error(kind, KindRule::GraphInvalidOnlyFromAnalyzing))
    } else {
        Ok(())
    }
}

/// Integration health, orthogonal to the run state (section 1.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IntegrationHealth {
    /// The integration branch is validated.
    Healthy,
    /// After a merge until validation completes, after an interrupted lane operation, or after an unexplained ref change.
    Unknown,
    /// Validation failed and the merge could not be safely discarded, or published integration history is broken.
    Unhealthy,
}

impl IntegrationHealth {
    /// Every value.
    pub const ALL: [Self; 3] = [Self::Healthy, Self::Unknown, Self::Unhealthy];

    /// The specification name (also the wire form).
    pub const fn name(self) -> &'static str {
        match self {
            Self::Healthy => "HEALTHY",
            Self::Unknown => "UNKNOWN",
            Self::Unhealthy => "UNHEALTHY",
        }
    }

    /// Whether a new worker may start. While not `HEALTHY` none may.
    pub const fn permits_worker_start(self) -> bool {
        matches!(self, Self::Healthy)
    }

    /// Whether a worker may enter the merge lane. While not `HEALTHY` none may (running workers wait at
    /// `READY_TO_MERGE`).
    pub const fn permits_merge_lane(self) -> bool {
        matches!(self, Self::Healthy)
    }

    /// Whether `RUNNING` may advance to `FINAL_REVIEW` (the other condition is that every in-scope ticket is `DONE`).
    pub const fn permits_final_review(self) -> bool {
        matches!(self, Self::Healthy)
    }

    /// The run-level intervention kind raised when the lane's recovery cannot restore health, if any.
    pub const fn unrecovered_kind(self) -> Option<InterventionKind> {
        match self {
            Self::Healthy => None,
            Self::Unknown => Some(InterventionKind::IntegrationUnknown),
            Self::Unhealthy => Some(InterventionKind::IntegrationUnhealthy),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use proptest::prelude::*;

    use super::*;

    use RunPhase as P;
    use RunState as S;

    fn paused(reason: PausedReason, resume_state: RunPhase) -> S {
        S::Paused { reason, resume_state }
    }

    fn needs(kind: InterventionKind, resume_state: RunPhase) -> S {
        S::NeedsHuman { kind, resume_state }
    }

    /// The specification's transition table (section 1.1), written out independently of the implementation as the set
    /// of legal (from, to) pairs for a user decision.
    fn spec_pairs() -> HashSet<(S, S)> {
        let mut legal = HashSet::new();
        let mut add = |from: S, to: S| {
            legal.insert((from, to));
        };
        // Forward and loop rows.
        add(S::Active(P::Created), S::Active(P::Preflight));
        add(S::Active(P::Preflight), S::Active(P::Preflight));
        add(S::Active(P::Preflight), S::Active(P::Analyzing));
        add(S::Active(P::Analyzing), S::Active(P::GraphReady));
        add(S::Active(P::GraphReady), S::Active(P::AwaitingStart));
        add(S::Active(P::AwaitingStart), S::Active(P::Starting));
        add(S::Active(P::Starting), S::Active(P::Running));
        add(S::Active(P::Running), S::Active(P::FinalReview));
        add(S::Active(P::FinalReview), S::Active(P::FinalFixing));
        add(S::Active(P::FinalFixing), S::Active(P::FinalReview));
        add(S::Active(P::FinalReview), S::Active(P::FinalValidation));
        add(S::Active(P::FinalValidation), S::Active(P::FinalFixing));
        add(S::Active(P::FinalValidation), S::Completed);
        // Running pauses; Running, Starting need a human; Analyzing needs a human only for GRAPH_INVALID.
        for reason in PausedReason::ALL {
            add(S::Active(P::Running), paused(reason, P::Running));
        }
        for kind in InterventionKind::ALL {
            if kind != InterventionKind::GraphInvalid {
                add(S::Active(P::Running), needs(kind, P::Running));
                add(S::Active(P::Starting), needs(kind, P::Starting));
            }
        }
        add(
            S::Active(P::Analyzing),
            needs(InterventionKind::GraphInvalid, P::Analyzing),
        );
        // Stop All from every non-terminal state, recording the state left.
        for phase in P::ALL {
            add(S::Active(phase), S::Stopping { resume_state: phase });
            for reason in PausedReason::ALL {
                add(paused(reason, phase), S::Stopping { resume_state: phase });
                // STOPPING resolves to PAUSED.
                add(S::Stopping { resume_state: phase }, paused(reason, phase));
            }
            for kind in InterventionKind::ALL {
                add(needs(kind, phase), S::Stopping { resume_state: phase });
                // STOPPING resolves to NEEDS_HUMAN (GRAPH_INVALID belongs to ANALYZING only).
                if kind != InterventionKind::GraphInvalid || phase == P::Analyzing {
                    add(S::Stopping { resume_state: phase }, needs(kind, phase));
                }
            }
            // Resume returns to resume_state; a cancel ends the run.
            for reason in PausedReason::ALL {
                add(paused(reason, phase), S::Active(phase));
                add(paused(reason, phase), S::Cancelled);
            }
            for kind in InterventionKind::ALL {
                add(needs(kind, phase), S::Active(phase));
                add(needs(kind, phase), S::Cancelled);
            }
        }
        // FAILED from any non-terminal state.
        for state in S::all() {
            if !state.is_terminal() {
                add(state, S::Failed);
            }
        }
        // PROVIDER_POLICY_BLOCK leaves only by resolve to resume_state or cancel (no FAILED, no STOPPING).
        legal.retain(|(from, to)| {
            !matches!(
                (from, to),
                (
                    S::NeedsHuman {
                        kind: InterventionKind::ProviderPolicyBlock,
                        ..
                    },
                    S::Failed | S::Stopping { .. }
                )
            )
        });
        legal
    }

    #[test]
    fn every_transition_pair_matches_the_specification_table() {
        let legal = spec_pairs();
        let states = S::all();
        assert_eq!(states.len(), 10 * (1 + 1 + 6 + 16) + 3);
        let mut accepted = 0;
        for from in &states {
            for to in &states {
                let result = from.transition(*to, Cause::UserDecision);
                let expected = legal.contains(&(*from, *to));
                assert_eq!(result.is_ok(), expected, "{from:?} -> {to:?}: {result:?}");
                if let Ok(next) = result {
                    assert_eq!(next, *to);
                    accepted += 1;
                }
            }
        }
        assert_eq!(accepted, legal.len());
    }

    #[test]
    fn system_cause_differs_only_for_provider_policy_block_resolve() {
        let states = S::all();
        for from in &states {
            for to in &states {
                let by_user = from.transition(*to, Cause::UserDecision);
                let by_system = from.transition(*to, Cause::System);
                let held = matches!(
                    from,
                    S::NeedsHuman {
                        kind: InterventionKind::ProviderPolicyBlock,
                        ..
                    }
                ) && matches!(to, S::Active(_));
                if held && by_user.is_ok() {
                    assert_eq!(
                        by_system,
                        Err(TransitionError::ExplicitDecisionRequired { machine: Machine::Run })
                    );
                } else {
                    assert_eq!(by_user, by_system, "{from:?} -> {to:?}");
                }
            }
        }
    }

    #[test]
    fn every_specification_row_is_accepted() {
        let user = Cause::UserDecision;
        let rows: [(S, S); 15] = [
            (S::Active(P::Created), S::Active(P::Preflight)),
            (S::Active(P::Preflight), S::Active(P::Preflight)),
            (S::Active(P::Preflight), S::Active(P::Analyzing)),
            (S::Active(P::Analyzing), S::Active(P::GraphReady)),
            (
                S::Active(P::Analyzing),
                needs(InterventionKind::GraphInvalid, P::Analyzing),
            ),
            (S::Active(P::GraphReady), S::Active(P::AwaitingStart)),
            (S::Active(P::AwaitingStart), S::Active(P::Starting)),
            (S::Active(P::Starting), S::Active(P::Running)),
            (S::Active(P::Starting), needs(InterventionKind::Other, P::Starting)),
            (S::Active(P::Starting), S::Failed),
            (S::Active(P::Running), paused(PausedReason::Manual, P::Running)),
            (
                S::Active(P::Running),
                needs(InterventionKind::IntegrationUnknown, P::Running),
            ),
            (S::Active(P::Running), S::Active(P::FinalReview)),
            (S::Active(P::FinalValidation), S::Completed),
            (S::Active(P::FinalValidation), S::Active(P::FinalFixing)),
        ];
        for (from, to) in rows {
            assert_eq!(from.transition(to, user), Ok(to), "{from:?} -> {to:?}");
        }
        // GRAPH_INVALID returns to ANALYZING after correction.
        let invalid = needs(InterventionKind::GraphInvalid, P::Analyzing);
        assert_eq!(invalid.transition(S::Active(P::Analyzing), Cause::System), Ok(S::Active(P::Analyzing)));
        // Resume returns to resume_state, including the FINAL_* states.
        let held = paused(PausedReason::StopAll, P::FinalFixing);
        assert_eq!(held.transition(S::Active(P::FinalFixing), Cause::System), Ok(S::Active(P::FinalFixing)));
        assert!(matches!(
            held.transition(S::Active(P::Running), Cause::System),
            Err(TransitionError::ResumeStateMismatch { .. })
        ));
    }

    #[test]
    fn stop_all_round_trip_preserves_resume_state() {
        let start = S::Active(P::FinalValidation);
        let stopping = start
            .transition(S::Stopping { resume_state: P::FinalValidation }, Cause::UserDecision)
            .unwrap();
        let paused_state = stopping
            .transition(paused(PausedReason::StopAll, P::FinalValidation), Cause::System)
            .unwrap();
        assert_eq!(
            paused_state.transition(S::Active(P::FinalValidation), Cause::UserDecision),
            Ok(start)
        );
        // A wrong resume_state is a typed error, not a silent rewrite.
        assert!(matches!(
            S::Active(P::Running).transition(S::Stopping { resume_state: P::Created }, Cause::System),
            Err(TransitionError::ResumeStateMismatch { .. })
        ));
    }

    #[test]
    fn terminal_states_have_no_exits() {
        for terminal in [S::Completed, S::Failed, S::Cancelled] {
            for to in S::all() {
                assert!(matches!(
                    terminal.transition(to, Cause::UserDecision),
                    Err(TransitionError::Terminal { .. })
                ));
            }
        }
    }

    #[test]
    fn graph_invalid_rules_are_typed() {
        assert_eq!(
            S::Active(P::Analyzing).transition(needs(InterventionKind::Other, P::Analyzing), Cause::System),
            Err(TransitionError::KindNotAllowed {
                machine: Machine::Run,
                kind: InterventionKind::Other,
                rule: KindRule::AnalyzingRequiresGraphInvalid,
            })
        );
        assert_eq!(
            S::Active(P::Running).transition(needs(InterventionKind::GraphInvalid, P::Running), Cause::System),
            Err(TransitionError::KindNotAllowed {
                machine: Machine::Run,
                kind: InterventionKind::GraphInvalid,
                rule: KindRule::GraphInvalidOnlyFromAnalyzing,
            })
        );
    }

    #[test]
    fn provider_policy_block_exits_only_by_explicit_resolve_or_cancel() {
        let block = needs(InterventionKind::ProviderPolicyBlock, P::Running);
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
            vec![(S::Active(P::Running), Cause::UserDecision), (S::Cancelled, Cause::System), (S::Cancelled, Cause::UserDecision)]
        );
        assert_eq!(
            block.transition(S::Active(P::Running), Cause::System),
            Err(TransitionError::ExplicitDecisionRequired { machine: Machine::Run })
        );
        for to in [
            S::Failed,
            S::Stopping { resume_state: P::Running },
            paused(PausedReason::Manual, P::Running),
            needs(InterventionKind::Other, P::Running),
            S::Active(P::FinalReview),
            S::Active(P::Preflight),
        ] {
            assert_eq!(
                block.transition(to, Cause::UserDecision),
                Err(TransitionError::ProviderPolicyBlockHeld { machine: Machine::Run, to: to.name() })
            );
        }
        // A block reached through STOPPING is held the same way.
        let via_stop = S::Stopping { resume_state: P::Running }
            .transition(block, Cause::System)
            .unwrap();
        assert_eq!(via_stop, block);
    }

    #[test]
    fn integration_health_gates() {
        assert!(IntegrationHealth::Healthy.permits_worker_start());
        assert!(IntegrationHealth::Healthy.permits_merge_lane());
        assert!(IntegrationHealth::Healthy.permits_final_review());
        for health in [IntegrationHealth::Unknown, IntegrationHealth::Unhealthy] {
            assert!(!health.permits_worker_start());
            assert!(!health.permits_merge_lane());
            assert!(!health.permits_final_review());
            let kind = health.unrecovered_kind().unwrap();
            assert_eq!(
                S::Active(P::Running).transition(needs(kind, P::Running), Cause::System),
                Ok(needs(kind, P::Running))
            );
        }
        assert_eq!(IntegrationHealth::Healthy.unrecovered_kind(), None);
    }

    #[cfg(feature = "full")]
    #[test]
    fn serde_round_trip_uses_specification_names() {
        let state = needs(InterventionKind::ProviderPolicyBlock, P::FinalReview);
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("PROVIDER_POLICY_BLOCK") && json.contains("FINAL_REVIEW"), "{json}");
        assert_eq!(serde_json::from_str::<S>(&json).unwrap(), state);
    }

    proptest! {
        /// No sequence of legal transitions reaches a state outside the enumerated set, a state that is not
        /// well-formed, or a held block that escaped without the explicit exits.
        #[test]
        fn legal_sequences_stay_in_the_mapped_state_space(picks in proptest::collection::vec((any::<prop::sample::Index>(), any::<bool>()), 0..200)) {
            let universe = S::all();
            let known: HashSet<S> = universe.iter().copied().collect();
            let mut state = S::Active(P::Created);
            for (index, user) in picks {
                let target = universe[index.index(universe.len())];
                let cause = if user { Cause::UserDecision } else { Cause::System };
                let before = state;
                if let Ok(next) = state.transition(target, cause) {
                    prop_assert!(known.contains(&next));
                    if let S::NeedsHuman { kind: InterventionKind::ProviderPolicyBlock, resume_state } = before {
                        prop_assert!(next == S::Cancelled || (next == S::Active(resume_state) && user));
                    }
                    state = next;
                } else {
                    prop_assert_eq!(state, before);
                }
                prop_assert!(known.contains(&state));
            }
        }
    }
}
