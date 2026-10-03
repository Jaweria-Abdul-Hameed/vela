//! The closed event vocabulary (`SYSTEM_ARCHITECTURE.md` section 4) and one typed payload per kind.
//!
//! A single macro invocation is the source of truth: it generates the payload DTOs, the `EventKind` enum, the
//! `Event` enum (kind plus typed payload) and `EventKind::ALL`, so a kind cannot exist without a payload. Adding a kind
//! is a documented specification change (`SHARED_SURFACE_PROTOCOL.md` section 4), not a ticket detail, and the
//! completeness test compares this list with the specification text.
//!
//! The envelope already carries `run_id`, `ticket_id` and `worker_id`, so payloads hold only what is specific to the
//! event. Payloads never contain secrets; free-text fields are redacted before they are journaled.

use serde::{Deserialize, Serialize};

use crate::errors::VelaError;
use crate::ids::{BranchName, CommitSha, GateResult, ReviewSeverity, WorktreePath};

macro_rules! event_vocabulary {
    ($(
        $(#[$kdoc:meta])*
        $kind:ident => $payload:ident {
            $( $(#[$fm:meta])* $field:ident : $ty:ty ),* $(,)?
        }
    ),+ $(,)?) => {
        $(
            $(#[$kdoc])*
            #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
            #[cfg_attr(feature = "full", derive(ts_rs::TS))]
            pub struct $payload {
                $( $(#[$fm])* pub $field: $ty ),*
            }

            #[cfg(test)]
            impl $payload {
                pub(crate) fn sample() -> Self {
                    Self { $( $field: <$ty as super::sample::Sample>::sample() ),* }
                }
            }
        )+

        /// The kind of a journal event: the closed vocabulary. The wire form is the specification's name.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[cfg_attr(feature = "full", derive(ts_rs::TS))]
        pub enum EventKind {
            $( $(#[$kdoc])* $kind ),+
        }

        impl EventKind {
            /// Every kind, in specification order.
            pub const ALL: &'static [EventKind] = &[ $( EventKind::$kind ),+ ];

            /// The specification name of the kind (also its wire form).
            pub const fn as_str(self) -> &'static str {
                match self {
                    $( EventKind::$kind => stringify!($kind) ),+
                }
            }
        }

        /// A journal event: its kind and typed payload. On the wire it is `{"kind": ..., "payload": {...}}`.
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[cfg_attr(feature = "full", derive(ts_rs::TS))]
        #[serde(tag = "kind", content = "payload")]
        pub enum Event {
            $( $kind($payload) ),+
        }

        impl Event {
            /// The kind of this event.
            pub fn kind(&self) -> EventKind {
                match self {
                    $( Event::$kind(_) => EventKind::$kind ),+
                }
            }
        }

        /// One sample event per kind, in specification order.
        #[cfg(test)]
        pub(crate) fn sample_events() -> Vec<Event> {
            vec![ $( Event::$kind($payload::sample()) ),+ ]
        }
    };
}

event_vocabulary! {
    // ---- Core vocabulary (SYSTEM_ARCHITECTURE.md section 4, first list) ----

    /// A build run was created.
    RunCreated => RunCreatedPayload {
        /// The recorded integration base (`run_base_sha`).
        run_base_sha: CommitSha,
        /// The integration branch cut from it.
        integration_branch: BranchName,
        /// The scope the run builds.
        scope: String,
    },
    /// Preflight finished.
    PreflightCompleted => PreflightCompletedPayload {
        /// True when no blocking finding remains.
        passed: bool,
        /// Number of blocking findings.
        blocking_findings: u32,
        /// Number of warnings.
        warnings: u32,
    },
    /// The dependency graph was built and stored as a snapshot.
    GraphBuilt => GraphBuiltPayload {
        /// Identifier of the stored graph snapshot.
        graph_snapshot_id: String,
        /// Number of tickets.
        ticket_count: u32,
        /// Number of dependency edges.
        edge_count: u32,
    },
    /// A ticket entered the ready frontier.
    TicketReady => TicketReadyPayload {
        /// Size of the ready frontier after the change.
        frontier_size: u32,
    },
    /// A worker started.
    WorkerStarted => WorkerStartedPayload {
        /// The worker's branch.
        branch: BranchName,
        /// The worker's worktree.
        worktree: WorktreePath,
        /// The base commit the worker started from.
        base_sha: CommitSha,
        /// The execution profile the worker runs under.
        profile_id: String,
    },
    /// A command started.
    CommandStarted => CommandStartedPayload {
        /// Identifier correlating start and finish.
        command_id: String,
        /// The redacted command line.
        command: String,
        /// Working directory, when it is a managed worktree.
        cwd: Option<WorktreePath>,
    },
    /// A command finished.
    CommandFinished => CommandFinishedPayload {
        /// Identifier correlating start and finish.
        command_id: String,
        /// Exit code, when the process exited normally.
        exit_code: Option<i32>,
        /// Wall-clock duration in milliseconds.
        #[cfg_attr(feature = "full", ts(type = "number"))]
        duration_ms: u64,
        /// True when the command was killed for exceeding its timeout.
        timed_out: bool,
    },
    /// A test or validation gate finished.
    TestGateFinished => TestGateFinishedPayload {
        /// The gate result.
        result: GateResult,
    },
    /// A checkpoint commit was created.
    CheckpointCommitted => CheckpointCommittedPayload {
        /// The checkpoint commit.
        commit: CommitSha,
        /// The review fixed point it is measured against.
        fixed_point: CommitSha,
    },
    /// A review iteration started.
    ReviewStarted => ReviewStartedPayload {
        /// Review iteration, starting at 1.
        iteration: u32,
        /// The fixed point the diff is taken against.
        fixed_point: CommitSha,
        /// The head under review.
        reviewed_head: CommitSha,
    },
    /// A review finding was raised.
    ReviewFindingRaised => ReviewFindingRaisedPayload {
        /// Stable finding identifier (fingerprint of category, location and requirement).
        finding_id: String,
        /// Severity.
        severity: ReviewSeverity,
        /// Finding category.
        category: String,
        /// Whether the policy engine classified it as blocking.
        blocking: bool,
        /// One-line summary.
        summary: String,
    },
    /// A review finding was resolved.
    ReviewFindingResolved => ReviewFindingResolvedPayload {
        /// Stable finding identifier.
        finding_id: String,
        /// How it was resolved.
        disposition: String,
    },
    /// A worker completed its ticket.
    WorkerCompleted => WorkerCompletedPayload {
        /// The worker's final head.
        head: CommitSha,
        /// The gates that passed for it.
        gates: Vec<GateResult>,
    },
    /// The merge lane started merging a worker.
    MergeStarted => MergeStartedPayload {
        /// The integration tip before the merge (the recorded reset point).
        integration_before: CommitSha,
    },
    /// A merge completed and was validated.
    MergeCompleted => MergeCompletedPayload {
        /// The integration tip before the merge.
        integration_before: CommitSha,
        /// The integration tip after the merge.
        integration_after: CommitSha,
    },
    /// Integration validation failed after a merge.
    IntegrationGateFailed => IntegrationGateFailedPayload {
        /// The integration tip before the merge.
        integration_before: CommitSha,
        /// The failing gate.
        result: GateResult,
    },
    /// A human must act.
    HumanActionRequired => HumanActionRequiredPayload {
        /// Intervention kind (for example `REVIEW_STALLED`).
        intervention_kind: String,
        /// What is needed.
        summary: String,
    },
    /// The run was paused.
    RunPaused => RunPausedPayload {
        /// Why it was paused.
        reason: String,
    },
    /// The run was resumed.
    RunResumed => RunResumedPayload {
        /// Why it was resumed.
        reason: String,
    },
    /// The run reached a terminal state.
    RunCompleted => RunCompletedPayload {
        /// Terminal outcome.
        outcome: String,
        /// Tickets that finished `DONE`.
        tickets_done: u32,
        /// Tickets in the run.
        tickets_total: u32,
    },

    // ---- Additional required events (second list) ----

    /// A project was trusted.
    ProjectTrusted => ProjectTrustedPayload {
        /// The repository identity the trust applies to.
        repository_identity: String,
        /// Version of the consent text the user saw.
        consent_text_version: String,
    },
    /// Trust in a project was revoked.
    ProjectTrustRevoked => ProjectTrustRevokedPayload {
        /// The repository identity the trust applied to.
        repository_identity: String,
    },
    /// An onboarding choice was recorded.
    OnboardingChoiceRecorded => OnboardingChoiceRecordedPayload {
        /// The choice key.
        key: String,
        /// The chosen value.
        value: String,
    },
    /// Consent to guarded UI automation changed.
    ApprovalAutomationConsentChanged => ApprovalAutomationConsentChangedPayload {
        /// True when the user enabled it.
        enabled: bool,
        /// Version of the consent text the user saw.
        consent_text_version: String,
    },
    /// An approval request was detected.
    ApprovalDetected => ApprovalDetectedPayload {
        /// Approval request identifier.
        approval_id: String,
        /// Observation source (for example the hook spool).
        source: String,
        /// Evidence tier (ADR-009).
        evidence_tier: String,
    },
    /// An approval request was classified by the policy engine.
    ApprovalClassified => ApprovalClassifiedPayload {
        /// Approval request identifier.
        approval_id: String,
        /// `ALLOW`, `ASK` or `DENY`.
        decision: String,
        /// The matching rule, if any.
        rule_id: Option<String>,
        /// Why.
        reason: String,
    },
    /// Delivery of an approval decision was attempted.
    ApprovalDeliveryAttempted => ApprovalDeliveryAttemptedPayload {
        /// Approval request identifier.
        approval_id: String,
        /// The delivery adapter.
        adapter: String,
        /// Attempt number, starting at 1.
        attempt: u32,
    },
    /// Delivery of an approval decision was verified.
    ApprovalDeliveryVerified => ApprovalDeliveryVerifiedPayload {
        /// Approval request identifier.
        approval_id: String,
        /// The delivery adapter.
        adapter: String,
    },
    /// The same approval repeated without progress.
    ApprovalStalled => ApprovalStalledPayload {
        /// Approval request identifier.
        approval_id: String,
        /// The repeated fingerprint (never contains secrets).
        fingerprint: String,
        /// How many times it repeated.
        repeat_count: u32,
    },
    /// A detective control fired.
    PolicyViolationDetected => PolicyViolationDetectedPayload {
        /// The rule that fired, if any.
        rule_id: Option<String>,
        /// What was detected.
        detail: String,
    },
    /// A stop was requested.
    StopRequested => StopRequestedPayload {
        /// Who asked (for example `user`).
        requested_by: String,
    },
    /// The run is stopping.
    RunStopping => RunStoppingPayload {
        /// Workers still running.
        active_workers: u32,
    },
    /// A worker was cancelled.
    WorkerCancelled => WorkerCancelledPayload {
        /// Why.
        reason: String,
    },
    /// A worker failed.
    WorkerFailed => WorkerFailedPayload {
        /// The typed error.
        error: VelaError,
    },
    /// A worker was paused.
    WorkerPaused => WorkerPausedPayload {
        /// Why (for example `CAPACITY`).
        reason: String,
    },
    /// A worker was resumed.
    WorkerResumed => WorkerResumedPayload {
        /// The state it resumed from.
        from_state: String,
    },
    /// Reconciliation started.
    ReconciliationStarted => ReconciliationStartedPayload {
        /// Why (for example `startup`).
        reason: String,
    },
    /// Reconciliation completed.
    ReconciliationCompleted => ReconciliationCompletedPayload {
        /// Number of discrepancies found.
        discrepancies: u32,
        /// Outcome summary.
        outcome: String,
    },
    /// A discrepancy between recorded and observed state was detected.
    DiscrepancyDetected => DiscrepancyDetectedPayload {
        /// What the discrepancy is about.
        subject: String,
        /// What the journal says.
        expected: String,
        /// What was observed.
        observed: String,
    },
    /// A push completed.
    PushCompleted => PushCompletedPayload {
        /// The pushed branch.
        branch: BranchName,
        /// The pushed head.
        head: CommitSha,
    },
    /// A push failed.
    PushFailed => PushFailedPayload {
        /// The branch that failed to push.
        branch: BranchName,
        /// The typed error.
        error: VelaError,
    },
    /// An unpublished merge was discarded.
    MergeDiscarded => MergeDiscardedPayload {
        /// The integration tip the lane reset to.
        integration_before: CommitSha,
        /// Why it was discarded.
        reason: String,
    },
    /// Conflict resolution started.
    ConflictResolutionStarted => ConflictResolutionStartedPayload {
        /// The conflicting paths.
        conflicting_paths: Vec<String>,
    },
    /// A worktree was provisioned.
    WorktreeProvisioned => WorktreeProvisionedPayload {
        /// The worktree.
        worktree: WorktreePath,
        /// Its branch.
        branch: BranchName,
        /// The base commit.
        base_sha: CommitSha,
    },
    /// A worktree was removed.
    WorktreeRemoved => WorktreeRemovedPayload {
        /// The worktree.
        worktree: WorktreePath,
    },
    /// An execution profile entered a capacity cooldown.
    ProfileCooldownStarted => ProfileCooldownStartedPayload {
        /// The profile.
        profile_id: String,
        /// Why (for example a quota condition).
        reason: String,
        /// Epoch milliseconds when it is expected to be available again, if known.
        #[cfg_attr(feature = "full", ts(type = "number | null"))]
        available_at_ms: Option<i64>,
    },
    /// An execution profile became available again.
    ProfileAvailable => ProfileAvailablePayload {
        /// The profile.
        profile_id: String,
    },
    /// The provider reported that the account or service is blocked for terms or policy reasons.
    /// Provider-neutral wording (ADR-006); never retried or cleared automatically.
    ProviderPolicyBlockRecorded => ProviderPolicyBlockRecordedPayload {
        /// The affected profile, when known.
        profile_id: Option<String>,
        /// The documented or recorded evidence, verbatim and redacted.
        evidence: String,
    },
    /// A recorded provider policy block was cleared by an explicit user action.
    ProviderPolicyBlockCleared => ProviderPolicyBlockClearedPayload {
        /// The affected profile, when known.
        profile_id: Option<String>,
        /// Who cleared it (always a user action).
        cleared_by: String,
    },
    /// The integration branch was promoted.
    PromotionCompleted => PromotionCompletedPayload {
        /// The branch promoted from.
        from_branch: BranchName,
        /// The branch promoted to.
        to_branch: BranchName,
        /// The resulting head.
        head: CommitSha,
    },
    /// A state-store backup was created.
    StateStoreBackupCreated => StateStoreBackupCreatedPayload {
        /// Backup identifier.
        backup_id: String,
        /// Why (for example `pre-migration`).
        reason: String,
    },
    /// A migration was applied.
    MigrationApplied => MigrationAppliedPayload {
        /// Schema version before.
        from_version: u32,
        /// Schema version after.
        to_version: u32,
    },
    /// A migration failed.
    MigrationFailed => MigrationFailedPayload {
        /// The schema version that was being applied.
        to_version: u32,
        /// The typed error.
        error: VelaError,
    },
}
