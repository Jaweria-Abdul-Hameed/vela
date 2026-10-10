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
    /// Stop All is quiescing; becomes `PAUSED` (reason `STOP_ALL`) or `NEEDS_HUMAN`.
    Stopping {
        /// The state the run was in when Stop All began.
        resume_state: RunPhase,
        /// The human gate (`NEEDS_HUMAN` kind) the run was in when Stop All began, if any. A gated run keeps its gate
        /// through `STOPPING`: it can only return to `NEEDS_HUMAN` with the same kind (or fail), never to `PAUSED`, so
        /// Stop All cannot turn a human gate into an ordinary pause.
        gate: Option<InterventionKind>,
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
    /// The specification name of the state (without payload). The serde form of the enum is structural and is not
    /// validated on load; the persisted or contract wire form is fixed by the ticket that exports it.
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
            Self::Stopping { resume_state, .. }
            | Self::Paused { resume_state, .. }
            | Self::NeedsHuman { resume_state, .. } => Some(*resume_state),
            _ => None,
        }
    }

    /// Every payload combination of every run state (a superset of the states the table can reach; used by the
    /// exhaustive tests and by consumers that need the closed set).
    pub fn all() -> Vec<Self> {
        let mut states = Vec::new();
        for phase in RunPhase::ALL {
            states.push(Self::Active(phase));
            states.push(Self::Stopping {
                resume_state: phase,
                gate: None,
            });
            for kind in InterventionKind::ALL {
                states.push(Self::Stopping {
                    resume_state: phase,
                    gate: Some(kind),
                });
            }
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
    /// table is the only source of legality, plus the integration-health precondition of `RUNNING` to `FINAL_REVIEW`
    /// (section 1.1: integration health is `HEALTHY`; the other precondition, that every in-scope ticket is `DONE`, is
    /// not run state and stays with the Orchestrator).
    ///
    /// `cause` matters when leaving a state that waits for a human: a resume from, or a cancel of, `NEEDS_HUMAN` needs
    /// `Cause::UserDecision`, as does a resume or cancel of `PAUSED` unless the reason is automatic (`CAPACITY`,
    /// `INTERACTIVE_SESSION_UNAVAILABLE`). A `FAILED` outcome is a system outcome and needs no user decision.
    pub fn transition(
        self,
        to: Self,
        cause: Cause,
        health: IntegrationHealth,
    ) -> Result<Self, TransitionError> {
        if self == Self::Active(RunPhase::Running)
            && to == Self::Active(RunPhase::FinalReview)
            && !health.permits_final_review()
        {
            return Err(TransitionError::IntegrationNotHealthy {
                health: health.name(),
            });
        }
        self.table(to, cause)
    }

    /// The structural transition table (no health precondition).
    fn table(self, to: Self, cause: Cause) -> Result<Self, TransitionError> {
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
                Self::Failed => Ok(to),
                Self::Cancelled => require_user(cause).map(|()| to),
                Self::Stopping {
                    resume_state: stopping,
                    gate: Some(gate),
                } if stopping == resume_state && gate == kind => Ok(to),
                Self::Active(phase) if phase == resume_state => require_user(cause).map(|()| to),
                _ => Err(TransitionError::ProviderPolicyBlockHeld {
                    machine: Machine::Run,
                    to: to.name(),
                }),
            };
        }
        if let Self::Stopping {
            resume_state,
            gate: Some(kind),
        } = self
        {
            return match to {
                Self::Failed => Ok(to),
                Self::NeedsHuman {
                    kind: restored,
                    resume_state: target,
                } if restored == kind => same(to, resume_state, target),
                _ if kind.is_held() => Err(TransitionError::ProviderPolicyBlockHeld {
                    machine: Machine::Run,
                    to: to.name(),
                }),
                _ => Err(TransitionError::HumanGateHeld {
                    machine: Machine::Run,
                    kind,
                    to: to.name(),
                }),
            };
        }
        match (self, to) {
            (_, Self::Failed) => Ok(to),
            (Self::Active(RunPhase::FinalValidation), Self::Completed) => Ok(to),
            (Self::Active(from), Self::Active(target)) if PHASE_EDGES.contains(&(from, target)) => {
                Ok(to)
            }
            (
                Self::Active(from),
                Self::Stopping {
                    resume_state,
                    gate: None,
                },
            ) => same(to, from, resume_state),
            (
                Self::Paused {
                    resume_state: held, ..
                },
                Self::Stopping {
                    resume_state,
                    gate: None,
                },
            ) => same(to, held, resume_state),
            (
                Self::NeedsHuman {
                    kind,
                    resume_state: held,
                },
                Self::Stopping {
                    resume_state,
                    gate: Some(gate),
                },
            ) if gate == kind => same(to, held, resume_state),
            (
                Self::Active(RunPhase::Running),
                Self::Paused {
                    reason,
                    resume_state,
                },
            ) => {
                same(to, RunPhase::Running, resume_state)?;
                if reason == PausedReason::StopAll {
                    return Err(TransitionError::ReasonNotAllowed {
                        machine: Machine::Run,
                        reason,
                    });
                }
                Ok(to)
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
            (
                Self::Stopping {
                    resume_state: held,
                    gate: None,
                },
                Self::Paused {
                    reason,
                    resume_state,
                },
            ) => {
                same(to, held, resume_state)?;
                if reason != PausedReason::StopAll {
                    return Err(TransitionError::ReasonNotAllowed {
                        machine: Machine::Run,
                        reason,
                    });
                }
                Ok(to)
            }
            (
                Self::Stopping {
                    resume_state: held,
                    gate: None,
                },
                Self::NeedsHuman { kind, resume_state },
            ) => {
                same(to, held, resume_state)?;
                graph_invalid_rule(kind, held)?;
                Ok(to)
            }
            (
                Self::Paused {
                    resume_state: held, ..
                },
                Self::NeedsHuman { kind, resume_state },
            ) => {
                same(to, held, resume_state)?;
                graph_invalid_rule(kind, held)?;
                Ok(to)
            }
            // A human gate can escalate to a provider policy block (it is then held).
            (
                Self::NeedsHuman {
                    resume_state: held, ..
                },
                Self::NeedsHuman {
                    kind: InterventionKind::ProviderPolicyBlock,
                    resume_state,
                },
            ) => same(to, held, resume_state),
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
            (Self::Paused { reason, .. }, Self::Cancelled) => {
                if reason.resumes_automatically() {
                    Ok(to)
                } else {
                    require_user(cause).map(|()| to)
                }
            }
            (Self::NeedsHuman { .. }, Self::Cancelled) => require_user(cause).map(|()| to),
            _ => Err(TransitionError::Illegal {
                machine: Machine::Run,
                from: self.name(),
                to: to.name(),
            }),
        }
    }
}

/// Leaving a human-gated state (resume or cancel) needs an explicit user decision.
fn require_user(cause: Cause) -> Result<(), TransitionError> {
    if cause == Cause::UserDecision {
        Ok(())
    } else {
        Err(TransitionError::ExplicitDecisionRequired {
            machine: Machine::Run,
        })
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
        S::Paused {
            reason,
            resume_state,
        }
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
            // STOP_ALL is reached only through STOPPING.
            if reason != PausedReason::StopAll {
                add(S::Active(P::Running), paused(reason, P::Running));
            }
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
        // Stop All from every non-terminal state, recording the state left; a human gate is kept through STOPPING.
        for phase in P::ALL {
            let plain = S::Stopping {
                resume_state: phase,
                gate: None,
            };
            add(S::Active(phase), plain);
            // STOPPING resolves to PAUSED with reason STOP_ALL (section 6) or to NEEDS_HUMAN
            // (GRAPH_INVALID belongs to ANALYZING only).
            add(plain, paused(PausedReason::StopAll, phase));
            for reason in PausedReason::ALL {
                add(paused(reason, phase), plain);
            }
            for kind in InterventionKind::ALL {
                let gated = S::Stopping {
                    resume_state: phase,
                    gate: Some(kind),
                };
                add(needs(kind, phase), gated);
                add(gated, needs(kind, phase));
                if kind != InterventionKind::GraphInvalid || phase == P::Analyzing {
                    add(plain, needs(kind, phase));
                }
            }
            // Resume returns to resume_state; a cancel ends the run; a pause can escalate to a human gate, and a human
            // gate can escalate to a provider policy block.
            for reason in PausedReason::ALL {
                add(paused(reason, phase), S::Active(phase));
                add(paused(reason, phase), S::Cancelled);
                for kind in InterventionKind::ALL {
                    if kind != InterventionKind::GraphInvalid || phase == P::Analyzing {
                        add(paused(reason, phase), needs(kind, phase));
                    }
                }
            }
            for kind in InterventionKind::ALL {
                add(needs(kind, phase), S::Active(phase));
                add(needs(kind, phase), S::Cancelled);
                if kind != InterventionKind::ProviderPolicyBlock {
                    add(
                        needs(kind, phase),
                        needs(InterventionKind::ProviderPolicyBlock, phase),
                    );
                }
            }
        }
        // FAILED from any non-terminal state.
        for state in S::all() {
            if !state.is_terminal() {
                add(state, S::Failed);
            }
        }
        legal
    }

    #[test]
    fn every_transition_pair_matches_the_specification_table() {
        let legal = spec_pairs();
        let states = S::all();
        assert_eq!(states.len(), 10 * (1 + 1 + 16 + 6 + 16) + 3);
        let mut accepted = 0;
        for from in &states {
            for to in &states {
                let result = from.table(*to, Cause::UserDecision);
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
    fn leaving_a_human_gate_needs_a_user_decision_unless_the_pause_is_automatic() {
        let states = S::all();
        for from in &states {
            for to in &states {
                let by_user = from.table(*to, Cause::UserDecision);
                let by_system = from.table(*to, Cause::System);
                let automatic = matches!(
                    from,
                    S::Paused { reason, .. } if reason.resumes_automatically()
                );
                let leaves_gate = matches!(
                    (from, to),
                    (
                        S::Paused { .. } | S::NeedsHuman { .. },
                        S::Active(_) | S::Cancelled
                    )
                );
                if leaves_gate && by_user.is_ok() && !automatic {
                    assert_eq!(
                        by_system,
                        Err(TransitionError::ExplicitDecisionRequired {
                            machine: Machine::Run
                        }),
                        "{from:?} -> {to:?}"
                    );
                } else {
                    assert_eq!(by_user, by_system, "{from:?} -> {to:?}");
                }
            }
        }
    }

    #[test]
    fn stop_all_keeps_a_human_gate() {
        let gate = needs(InterventionKind::PolicyViolation, P::Running);
        // A gated run cannot enter an ungated STOPPING (that would turn the gate into a plain pause).
        assert!(matches!(
            gate.table(
                S::Stopping {
                    resume_state: P::Running,
                    gate: None
                },
                Cause::System
            ),
            Err(TransitionError::Illegal { .. })
        ));
        let stopping = gate
            .table(
                S::Stopping {
                    resume_state: P::Running,
                    gate: Some(InterventionKind::PolicyViolation),
                },
                Cause::System,
            )
            .unwrap();
        // It returns only to the same gate (or fails).
        assert_eq!(stopping.table(gate, Cause::System), Ok(gate));
        assert_eq!(stopping.table(S::Failed, Cause::System), Ok(S::Failed));
        for to in [
            paused(PausedReason::StopAll, P::Running),
            needs(InterventionKind::Other, P::Running),
            S::Active(P::Running),
            S::Cancelled,
        ] {
            assert_eq!(
                stopping.table(to, Cause::UserDecision),
                Err(TransitionError::HumanGateHeld {
                    machine: Machine::Run,
                    kind: InterventionKind::PolicyViolation,
                    to: to.name()
                })
            );
        }
        // And the resume afterwards still needs the user.
        assert_eq!(
            gate.table(S::Active(P::Running), Cause::System),
            Err(TransitionError::ExplicitDecisionRequired {
                machine: Machine::Run
            })
        );
    }

    #[test]
    fn a_human_gate_can_escalate_to_a_provider_policy_block_but_not_back() {
        let gate = needs(InterventionKind::AuthRequired, P::Running);
        let block = needs(InterventionKind::ProviderPolicyBlock, P::Running);
        assert_eq!(gate.table(block, Cause::System), Ok(block));
        assert!(
            gate.table(needs(InterventionKind::Other, P::Running), Cause::System)
                .is_err()
        );
        assert!(block.table(gate, Cause::UserDecision).is_err());
        assert!(
            gate.table(
                needs(InterventionKind::ProviderPolicyBlock, P::Starting),
                Cause::System
            )
            .is_err()
        );
    }

    #[test]
    fn cancelling_a_human_gate_needs_a_user_decision() {
        for state in [
            needs(InterventionKind::AuthRequired, P::Running),
            needs(InterventionKind::ProviderPolicyBlock, P::Running),
            paused(PausedReason::Manual, P::Running),
        ] {
            assert_eq!(
                state.table(S::Cancelled, Cause::System),
                Err(TransitionError::ExplicitDecisionRequired {
                    machine: Machine::Run
                })
            );
            assert_eq!(
                state.table(S::Cancelled, Cause::UserDecision),
                Ok(S::Cancelled)
            );
        }
        // An automatic pause can be cancelled by the system.
        assert_eq!(
            paused(PausedReason::Capacity, P::Running).table(S::Cancelled, Cause::System),
            Ok(S::Cancelled)
        );
    }

    #[test]
    fn stop_all_pauses_with_stop_all_only() {
        let stopping = S::Stopping {
            resume_state: P::Running,
            gate: None,
        };
        for reason in PausedReason::ALL {
            let result = stopping.table(paused(reason, P::Running), Cause::System);
            let direct = S::Active(P::Running).table(paused(reason, P::Running), Cause::System);
            let rejected = Err(TransitionError::ReasonNotAllowed {
                machine: Machine::Run,
                reason,
            });
            if reason == PausedReason::StopAll {
                assert!(result.is_ok());
                assert_eq!(direct, rejected);
            } else {
                assert_eq!(result, rejected);
                assert!(direct.is_ok());
            }
        }
    }

    #[test]
    fn final_review_needs_healthy_integration() {
        let running = S::Active(P::Running);
        let review = S::Active(P::FinalReview);
        assert_eq!(
            running.transition(review, Cause::System, IntegrationHealth::Healthy),
            Ok(review)
        );
        for health in [IntegrationHealth::Unknown, IntegrationHealth::Unhealthy] {
            assert_eq!(
                running.transition(review, Cause::System, health),
                Err(TransitionError::IntegrationNotHealthy {
                    health: health.name()
                })
            );
            // Other transitions are not gated by health.
            assert!(
                running
                    .transition(
                        paused(PausedReason::Manual, P::Running),
                        Cause::System,
                        health
                    )
                    .is_ok()
            );
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
            (
                S::Active(P::Starting),
                needs(InterventionKind::Other, P::Starting),
            ),
            (S::Active(P::Starting), S::Failed),
            (
                S::Active(P::Running),
                paused(PausedReason::Manual, P::Running),
            ),
            (
                S::Active(P::Running),
                needs(InterventionKind::IntegrationUnknown, P::Running),
            ),
            (S::Active(P::Running), S::Active(P::FinalReview)),
            (S::Active(P::FinalValidation), S::Completed),
            (S::Active(P::FinalValidation), S::Active(P::FinalFixing)),
        ];
        for (from, to) in rows {
            assert_eq!(from.table(to, user), Ok(to), "{from:?} -> {to:?}");
        }
        // GRAPH_INVALID returns to ANALYZING after correction.
        let invalid = needs(InterventionKind::GraphInvalid, P::Analyzing);
        assert_eq!(
            invalid.table(S::Active(P::Analyzing), Cause::UserDecision),
            Ok(S::Active(P::Analyzing))
        );
        // Resume returns to resume_state, including the FINAL_* states.
        let held = paused(PausedReason::StopAll, P::FinalFixing);
        assert_eq!(
            held.table(S::Active(P::FinalFixing), Cause::UserDecision),
            Ok(S::Active(P::FinalFixing))
        );
        assert!(matches!(
            held.table(S::Active(P::Running), Cause::System),
            Err(TransitionError::ResumeStateMismatch { .. })
        ));
    }

    #[test]
    fn stop_all_round_trip_preserves_resume_state() {
        let start = S::Active(P::FinalValidation);
        let stopping = start
            .table(
                S::Stopping {
                    resume_state: P::FinalValidation,
                    gate: None,
                },
                Cause::UserDecision,
            )
            .unwrap();
        let paused_state = stopping
            .table(
                paused(PausedReason::StopAll, P::FinalValidation),
                Cause::System,
            )
            .unwrap();
        assert_eq!(
            paused_state.table(S::Active(P::FinalValidation), Cause::UserDecision),
            Ok(start)
        );
        // A wrong resume_state is a typed error, not a silent rewrite.
        assert!(matches!(
            S::Active(P::Running).table(
                S::Stopping {
                    resume_state: P::Created,
                    gate: None
                },
                Cause::System
            ),
            Err(TransitionError::ResumeStateMismatch { .. })
        ));
    }

    #[test]
    fn terminal_states_have_no_exits() {
        for terminal in [S::Completed, S::Failed, S::Cancelled] {
            for to in S::all() {
                assert!(matches!(
                    terminal.table(to, Cause::UserDecision),
                    Err(TransitionError::Terminal { .. })
                ));
            }
        }
    }

    #[test]
    fn graph_invalid_rules_are_typed() {
        assert_eq!(
            S::Active(P::Analyzing)
                .table(needs(InterventionKind::Other, P::Analyzing), Cause::System),
            Err(TransitionError::KindNotAllowed {
                machine: Machine::Run,
                kind: InterventionKind::Other,
                rule: KindRule::AnalyzingRequiresGraphInvalid,
            })
        );
        assert_eq!(
            S::Active(P::Running).table(
                needs(InterventionKind::GraphInvalid, P::Running),
                Cause::System
            ),
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
                if block.table(to, cause).is_ok() {
                    exits.push((to, cause));
                }
            }
        }
        assert_eq!(
            exits,
            vec![
                (S::Active(P::Running), Cause::UserDecision),
                (
                    S::Stopping {
                        resume_state: P::Running,
                        gate: Some(InterventionKind::ProviderPolicyBlock)
                    },
                    Cause::System
                ),
                (
                    S::Stopping {
                        resume_state: P::Running,
                        gate: Some(InterventionKind::ProviderPolicyBlock)
                    },
                    Cause::UserDecision
                ),
                (S::Failed, Cause::System),
                (S::Failed, Cause::UserDecision),
                (S::Cancelled, Cause::UserDecision)
            ]
        );
        assert_eq!(
            block.table(S::Active(P::Running), Cause::System),
            Err(TransitionError::ExplicitDecisionRequired {
                machine: Machine::Run
            })
        );
        for to in [
            S::Stopping {
                resume_state: P::Running,
                gate: None,
            },
            S::Stopping {
                resume_state: P::Preflight,
                gate: Some(InterventionKind::ProviderPolicyBlock),
            },
            paused(PausedReason::Manual, P::Running),
            needs(InterventionKind::Other, P::Running),
            S::Active(P::FinalReview),
            S::Active(P::Preflight),
        ] {
            assert_eq!(
                block.table(to, Cause::UserDecision),
                Err(TransitionError::ProviderPolicyBlockHeld {
                    machine: Machine::Run,
                    to: to.name()
                })
            );
        }
        // Stop All keeps the hold: STOPPING of a block returns only to the block (or fails), never to PAUSED or work.
        let stopping_block = S::Stopping {
            resume_state: P::Running,
            gate: Some(InterventionKind::ProviderPolicyBlock),
        };
        for to in S::all() {
            for cause in [Cause::System, Cause::UserDecision] {
                assert_eq!(
                    stopping_block.table(to, cause).is_ok(),
                    to == block || to == S::Failed,
                    "{to:?}"
                );
            }
        }
        // A block reached through STOPPING is held the same way.
        let via_stop = S::Stopping {
            resume_state: P::Running,
            gate: None,
        }
        .table(block, Cause::System)
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
                S::Active(P::Running).table(needs(kind, P::Running), Cause::System),
                Ok(needs(kind, P::Running))
            );
        }
        assert_eq!(IntegrationHealth::Healthy.unrecovered_kind(), None);
    }

    #[cfg(feature = "full")]
    #[test]
    fn serde_round_trip_keeps_specification_names() {
        let state = needs(InterventionKind::ProviderPolicyBlock, P::FinalReview);
        let json = serde_json::to_string(&state).unwrap();
        assert!(
            json.contains("PROVIDER_POLICY_BLOCK") && json.contains("FINAL_REVIEW"),
            "{json}"
        );
        assert_eq!(serde_json::from_str::<S>(&json).unwrap(), state);
    }

    proptest! {
        /// Walking only legal transitions never reaches a state outside the enumerated set, and a held block is left
        /// only through the explicit exits (resolve or cancel by user decision, fail, the same gate kept through STOPPING, or
        /// escalation to a provider policy block).
        #[test]
        fn legal_sequences_stay_in_the_mapped_state_space(picks in proptest::collection::vec((any::<prop::sample::Index>(), any::<bool>()), 0..200)) {
            let universe = S::all();
            let known: HashSet<S> = universe.iter().copied().collect();
            let mut state = S::Active(P::Created);
            for (index, user) in picks {
                let cause = if user { Cause::UserDecision } else { Cause::System };
                let legal: Vec<S> = universe
                    .iter()
                    .copied()
                    .filter(|target| state.table(*target, cause).is_ok())
                    .collect();
                if legal.is_empty() {
                    prop_assert!(state.is_terminal());
                    break;
                }
                let before = state;
                let next = legal[index.index(legal.len())];
                prop_assert_eq!(before.table(next, cause), Ok(next));
                prop_assert!(known.contains(&next));
                match before {
                    S::NeedsHuman { kind, resume_state } => {
                        let kept = S::Stopping { resume_state, gate: Some(kind) };
                        let escalated = needs(InterventionKind::ProviderPolicyBlock, resume_state);
                        prop_assert!(
                            next == S::Failed
                                || next == kept
                                || next == escalated
                                || (user && (next == S::Active(resume_state) || next == S::Cancelled))
                        );
                    }
                    S::Stopping { resume_state, gate: Some(kind) } => {
                        prop_assert!(next == S::Failed || next == needs(kind, resume_state));
                    }
                    _ => {}
                }
                state = next;
            }
        }
    }
}
