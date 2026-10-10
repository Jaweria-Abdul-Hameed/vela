//! Ticket state projection and UI node mapping (`DOMAIN_MODEL.md`). Ticket and UI node states are projections of worker
//! state (and, before a worker exists, of blocker states): they are derived here and never set independently.

use serde::{Deserialize, Serialize};

use super::common::{InterventionKind, PausedReason};
use super::run::IntegrationHealth;
use super::worker::{WorkerPhase, WorkerState, WorkerStatus};

/// The ticket state (`DOMAIN_MODEL.md`, "Ticket state projection").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TicketState {
    /// Not every explicit blocker is `DONE`; no worker.
    Pending,
    /// A blocker is itself `FAILED`, `NEEDS_HUMAN` (including `REVIEW_STALLED`) or `CANCELLED`.
    Blocked,
    /// All blockers `DONE`, integration healthy, eligible for the frontier, no worker yet.
    Ready,
    /// Worker in `PROVISIONING`, `CONTEXT_LOADING` or `IMPLEMENTING`.
    Running,
    /// Worker in `FOCUSED_VALIDATION` or `VALIDATING`.
    Testing,
    /// Worker in `CHECKPOINTING` or `REVIEWING`.
    Reviewing,
    /// Worker in `FIXING` or `CONFLICT_RESOLUTION`.
    Fixing,
    /// Worker in `READY_TO_MERGE`, `MERGING` or `INTEGRATION_VALIDATION`.
    Merging,
    /// Worker `COMPLETE`.
    Done,
    /// Worker `PAUSED` for any reason except `CAPACITY`.
    Paused,
    /// Worker `PAUSED` with reason `CAPACITY`.
    RateLimited,
    /// Worker `FAILED`.
    Failed,
    /// Worker `NEEDS_HUMAN` with kind `REVIEW_STALLED`.
    ReviewStalled,
    /// Worker `NEEDS_HUMAN` (any kind except `REVIEW_STALLED`; `APPROVAL_STALLED` included).
    NeedsHuman,
    /// Worker `CANCELLED`.
    Cancelled,
}

impl TicketState {
    /// Every ticket state.
    pub const ALL: [Self; 15] = [
        Self::Pending,
        Self::Blocked,
        Self::Ready,
        Self::Running,
        Self::Testing,
        Self::Reviewing,
        Self::Fixing,
        Self::Merging,
        Self::Done,
        Self::Paused,
        Self::RateLimited,
        Self::Failed,
        Self::ReviewStalled,
        Self::NeedsHuman,
        Self::Cancelled,
    ];

    /// The specification name (also the wire form).
    pub const fn name(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Blocked => "BLOCKED",
            Self::Ready => "READY",
            Self::Running => "RUNNING",
            Self::Testing => "TESTING",
            Self::Reviewing => "REVIEWING",
            Self::Fixing => "FIXING",
            Self::Merging => "MERGING",
            Self::Done => "DONE",
            Self::Paused => "PAUSED",
            Self::RateLimited => "RATE_LIMITED",
            Self::Failed => "FAILED",
            Self::ReviewStalled => "REVIEW_STALLED",
            Self::NeedsHuman => "NEEDS_HUMAN",
            Self::Cancelled => "CANCELLED",
        }
    }

    /// The projection of a worker state (the table in `DOMAIN_MODEL.md`).
    pub const fn from_worker_state(state: &WorkerState) -> Self {
        match state {
            WorkerState::Active(phase) => match phase {
                WorkerPhase::Provisioning
                | WorkerPhase::ContextLoading
                | WorkerPhase::Implementing => Self::Running,
                WorkerPhase::FocusedValidation | WorkerPhase::Validating => Self::Testing,
                WorkerPhase::Checkpointing | WorkerPhase::Reviewing => Self::Reviewing,
                WorkerPhase::Fixing | WorkerPhase::ConflictResolution => Self::Fixing,
                WorkerPhase::ReadyToMerge
                | WorkerPhase::Merging
                | WorkerPhase::IntegrationValidation => Self::Merging,
            },
            WorkerState::Complete => Self::Done,
            WorkerState::Paused {
                reason: PausedReason::Capacity,
                ..
            } => Self::RateLimited,
            WorkerState::Paused { .. } => Self::Paused,
            WorkerState::NeedsHuman {
                kind: InterventionKind::ReviewStalled,
                ..
            } => Self::ReviewStalled,
            WorkerState::NeedsHuman { .. } => Self::NeedsHuman,
            WorkerState::Failed => Self::Failed,
            WorkerState::Cancelled => Self::Cancelled,
        }
    }
}

/// Secondary marks shown on a node; none of them is a ticket state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubIndicators {
    /// The worker is `WAITING_APPROVAL`.
    pub waiting_approval: bool,
    /// The worker needs a human with kind `REVIEW_STALLED`.
    pub review_stalled: bool,
    /// The worker needs a human with kind `APPROVAL_STALLED`.
    pub approval_stalled: bool,
}

/// A ticket state together with its sub-indicators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TicketProjection {
    /// The ticket state.
    pub state: TicketState,
    /// The secondary marks.
    pub indicators: SubIndicators,
}

/// Projects a worker (state and overlay) onto the ticket state and its sub-indicators.
pub const fn project_worker(status: &WorkerStatus) -> TicketProjection {
    let (review_stalled, approval_stalled) = match status.state {
        WorkerState::NeedsHuman { kind, .. } => (
            matches!(kind, InterventionKind::ReviewStalled),
            matches!(kind, InterventionKind::ApprovalStalled),
        ),
        _ => (false, false),
    };
    TicketProjection {
        state: TicketState::from_worker_state(&status.state),
        indicators: SubIndicators {
            waiting_approval: status.waiting_approval,
            review_stalled,
            approval_stalled,
        },
    }
}

/// The ticket state before any worker exists: `BLOCKED` when a blocker is `FAILED`, `NEEDS_HUMAN`, `REVIEW_STALLED` or
/// `CANCELLED`; `READY` when every blocker is `DONE`, integration is healthy and the ticket is eligible for the
/// frontier; otherwise `PENDING` (decision request DR-D01-6).
pub fn project_unstarted(
    blockers: &[TicketState],
    health: IntegrationHealth,
    eligible: bool,
) -> TicketState {
    if blockers.iter().any(|blocker| {
        matches!(
            blocker,
            TicketState::Failed
                | TicketState::NeedsHuman
                | TicketState::ReviewStalled
                | TicketState::Cancelled
        )
    }) {
        TicketState::Blocked
    } else if blockers.iter().all(|blocker| *blocker == TicketState::Done)
        && health.permits_worker_start()
        && eligible
    {
        TicketState::Ready
    } else {
        TicketState::Pending
    }
}

/// The UI node state: the ticket state names of `UI_UX_SPEC.md`. `REVIEW_STALLED` is not a node state; it is a mark on a
/// `needs human` node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UiNodeState {
    /// `pending`.
    Pending,
    /// `ready`.
    Ready,
    /// `running`.
    Running,
    /// `testing`.
    Testing,
    /// `reviewing`.
    Reviewing,
    /// `fixing`.
    Fixing,
    /// `merging`.
    Merging,
    /// `done`.
    Done,
    /// `blocked`.
    Blocked,
    /// `paused` (settled/quiet treatment).
    Paused,
    /// `rate-limited`.
    RateLimited,
    /// `failed`.
    Failed,
    /// `needs human`.
    NeedsHuman,
    /// `cancelled` (settled/quiet treatment).
    Cancelled,
}

impl UiNodeState {
    /// Every node state.
    pub const ALL: [Self; 14] = [
        Self::Pending,
        Self::Ready,
        Self::Running,
        Self::Testing,
        Self::Reviewing,
        Self::Fixing,
        Self::Merging,
        Self::Done,
        Self::Blocked,
        Self::Paused,
        Self::RateLimited,
        Self::Failed,
        Self::NeedsHuman,
        Self::Cancelled,
    ];

    /// The label used by `DOMAIN_MODEL.md` (lower case, `rate-limited`, `needs human`).
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ready => "ready",
            Self::Running => "running",
            Self::Testing => "testing",
            Self::Reviewing => "reviewing",
            Self::Fixing => "fixing",
            Self::Merging => "merging",
            Self::Done => "done",
            Self::Blocked => "blocked",
            Self::Paused => "paused",
            Self::RateLimited => "rate-limited",
            Self::Failed => "failed",
            Self::NeedsHuman => "needs human",
            Self::Cancelled => "cancelled",
        }
    }

    /// The node state of a ticket state (`REVIEW_STALLED` shows as `needs human`).
    pub const fn from_ticket_state(state: TicketState) -> Self {
        match state {
            TicketState::Pending => Self::Pending,
            TicketState::Blocked => Self::Blocked,
            TicketState::Ready => Self::Ready,
            TicketState::Running => Self::Running,
            TicketState::Testing => Self::Testing,
            TicketState::Reviewing => Self::Reviewing,
            TicketState::Fixing => Self::Fixing,
            TicketState::Merging => Self::Merging,
            TicketState::Done => Self::Done,
            TicketState::Paused => Self::Paused,
            TicketState::RateLimited => Self::RateLimited,
            TicketState::Failed => Self::Failed,
            TicketState::ReviewStalled | TicketState::NeedsHuman => Self::NeedsHuman,
            TicketState::Cancelled => Self::Cancelled,
        }
    }
}

/// A UI node: its state and secondary marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UiNode {
    /// The node state.
    pub state: UiNodeState,
    /// The secondary marks (waiting approval, review stalled, approval stalled).
    pub indicators: SubIndicators,
}

impl UiNode {
    /// The node for a projection. A `REVIEW_STALLED` ticket gets the `review_stalled` mark.
    pub const fn from_projection(projection: &TicketProjection) -> Self {
        let mut indicators = projection.indicators;
        if matches!(projection.state, TicketState::ReviewStalled) {
            indicators.review_stalled = true;
        }
        Self {
            state: UiNodeState::from_ticket_state(projection.state),
            indicators,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use proptest::prelude::*;

    use super::super::common::Cause;
    use super::*;

    use TicketState as T;
    use WorkerPhase as P;

    #[test]
    fn every_worker_phase_projects_as_in_the_domain_model_table() {
        let expected = [
            (P::Provisioning, T::Running),
            (P::ContextLoading, T::Running),
            (P::Implementing, T::Running),
            (P::FocusedValidation, T::Testing),
            (P::Validating, T::Testing),
            (P::Checkpointing, T::Reviewing),
            (P::Reviewing, T::Reviewing),
            (P::Fixing, T::Fixing),
            (P::ConflictResolution, T::Fixing),
            (P::ReadyToMerge, T::Merging),
            (P::Merging, T::Merging),
            (P::IntegrationValidation, T::Merging),
        ];
        assert_eq!(expected.len(), P::ALL.len());
        for phase in P::ALL {
            let (_, want) = expected
                .iter()
                .find(|(p, _)| *p == phase)
                .expect("phase listed");
            assert_eq!(
                TicketState::from_worker_state(&WorkerState::Active(phase)),
                *want,
                "{phase:?}"
            );
        }
    }

    #[test]
    fn terminal_and_exceptional_worker_states_project() {
        assert_eq!(
            TicketState::from_worker_state(&WorkerState::Complete),
            T::Done
        );
        assert_eq!(
            TicketState::from_worker_state(&WorkerState::Failed),
            T::Failed
        );
        assert_eq!(
            TicketState::from_worker_state(&WorkerState::Cancelled),
            T::Cancelled
        );
        for reason in PausedReason::ALL {
            let state = WorkerState::Paused {
                reason,
                resume_state: P::Implementing,
            };
            let want = if reason == PausedReason::Capacity {
                T::RateLimited
            } else {
                T::Paused
            };
            assert_eq!(TicketState::from_worker_state(&state), want, "{reason:?}");
        }
    }

    #[test]
    fn every_intervention_kind_projects_and_marks() {
        for kind in InterventionKind::ALL {
            let status = WorkerStatus::new(WorkerState::NeedsHuman {
                kind,
                resume_state: P::Reviewing,
            });
            let projection = project_worker(&status);
            let want = if kind == InterventionKind::ReviewStalled {
                T::ReviewStalled
            } else {
                T::NeedsHuman
            };
            assert_eq!(projection.state, want, "{kind:?}");
            assert_eq!(
                projection.indicators.review_stalled,
                kind == InterventionKind::ReviewStalled
            );
            assert_eq!(
                projection.indicators.approval_stalled,
                kind == InterventionKind::ApprovalStalled
            );
            assert!(!projection.indicators.waiting_approval);
            // Every kind lands on a node state; REVIEW_STALLED and APPROVAL_STALLED are marks on `needs human`.
            let node = UiNode::from_projection(&projection);
            assert_eq!(node.state, UiNodeState::NeedsHuman);
            assert_eq!(
                node.indicators.review_stalled,
                kind == InterventionKind::ReviewStalled
            );
        }
    }

    #[test]
    fn provider_policy_block_is_needs_human_never_rate_limited() {
        let status = WorkerStatus::new(WorkerState::NeedsHuman {
            kind: InterventionKind::ProviderPolicyBlock,
            resume_state: P::Implementing,
        });
        assert_eq!(project_worker(&status).state, T::NeedsHuman);
        assert_eq!(
            UiNode::from_projection(&project_worker(&status)).state,
            UiNodeState::NeedsHuman
        );
    }

    #[test]
    fn waiting_approval_is_a_mark_not_a_state() {
        let waiting = WorkerStatus::new(WorkerState::Active(P::Implementing))
            .begin_approval_wait()
            .unwrap();
        let projection = project_worker(&waiting);
        assert_eq!(projection.state, T::Running);
        assert!(projection.indicators.waiting_approval);
        assert_eq!(
            UiNode::from_projection(&projection).state,
            UiNodeState::Running
        );
    }

    #[test]
    fn unstarted_ticket_projection() {
        use IntegrationHealth::{Healthy, Unhealthy, Unknown};
        assert_eq!(project_unstarted(&[], Healthy, true), T::Ready);
        assert_eq!(project_unstarted(&[], Healthy, false), T::Pending);
        assert_eq!(
            project_unstarted(&[T::Done, T::Done], Healthy, true),
            T::Ready
        );
        assert_eq!(
            project_unstarted(&[T::Done, T::Running], Healthy, true),
            T::Pending
        );
        assert_eq!(project_unstarted(&[T::Done], Unknown, true), T::Pending);
        assert_eq!(project_unstarted(&[T::Done], Unhealthy, true), T::Pending);
        for stuck in [T::Failed, T::NeedsHuman, T::ReviewStalled, T::Cancelled] {
            assert_eq!(
                project_unstarted(&[T::Done, stuck], Healthy, true),
                T::Blocked,
                "{stuck:?}"
            );
            assert_eq!(
                project_unstarted(&[T::Running, stuck], Unknown, false),
                T::Blocked,
                "{stuck:?}"
            );
        }
        for open in [
            T::Pending,
            T::Blocked,
            T::Ready,
            T::Running,
            T::Paused,
            T::RateLimited,
        ] {
            assert_eq!(
                project_unstarted(&[open], Healthy, true),
                T::Pending,
                "{open:?}"
            );
        }
    }

    #[test]
    fn ui_mapping_is_total_and_labelled() {
        let mut seen = HashSet::new();
        for state in TicketState::ALL {
            seen.insert(UiNodeState::from_ticket_state(state));
        }
        assert_eq!(seen.len(), UiNodeState::ALL.len());
        let labels: Vec<_> = UiNodeState::ALL.iter().map(|s| s.label()).collect();
        assert_eq!(
            labels,
            [
                "pending",
                "ready",
                "running",
                "testing",
                "reviewing",
                "fixing",
                "merging",
                "done",
                "blocked",
                "paused",
                "rate-limited",
                "failed",
                "needs human",
                "cancelled"
            ]
        );
    }

    #[test]
    fn every_ticket_state_is_reached_by_some_projection() {
        let mut reached: HashSet<TicketState> = WorkerState::all()
            .iter()
            .map(TicketState::from_worker_state)
            .collect();
        reached.extend([T::Pending, T::Blocked, T::Ready]);
        assert_eq!(reached.len(), TicketState::ALL.len());
    }

    proptest! {
        /// Walking any legal worker sequence always lands on a ticket state and a node state, and `DONE` appears
        /// exactly for `COMPLETE`.
        #[test]
        fn legal_worker_walks_never_reach_an_unmapped_state(picks in proptest::collection::vec((any::<prop::sample::Index>(), any::<bool>(), 0u8..4), 0..200)) {
            let universe = WorkerState::all();
            let mut status = WorkerStatus::new(WorkerState::Active(P::Provisioning));
            for (index, user, action) in picks {
                let next = match action {
                    0 => status.begin_approval_wait(),
                    1 => status.end_approval_wait(),
                    _ => status.transition(
                        universe[index.index(universe.len())],
                        if user { Cause::UserDecision } else { Cause::System },
                    ),
                };
                if let Ok(next) = next {
                    status = next;
                }
                let projection = project_worker(&status);
                prop_assert!(TicketState::ALL.contains(&projection.state));
                prop_assert_eq!(projection.state == T::Done, status.state == WorkerState::Complete);
                prop_assert!(UiNodeState::ALL.contains(&UiNode::from_projection(&projection).state));
                prop_assert_eq!(projection.indicators.waiting_approval, status.waiting_approval);
            }
        }
    }
}
