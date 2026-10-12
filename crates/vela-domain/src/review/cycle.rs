//! Review stages and the per-ticket cycle rules (`REVIEW_PROTOCOL.md` "Review workflow"; `ORCHESTRATION_ENGINE.md`
//! section 4; ADR-022).
//!
//! [`ReviewHistory`] is the record of one ticket's review cycles. It enforces: at most one `COMPREHENSIVE` cycle unless
//! a recorded user decision requests another; a `VERIFICATION` cycle only after a fix set; every failed verification is
//! an escalation (`REVIEW_STALLED`); and a stalled history leaves that state only through a recorded user decision.

use std::collections::BTreeSet;
use std::fmt;

use super::finding::Finding;
use super::fingerprint::FindingId;
use super::verification::{ReviewOutcome, Verdict, VerificationInput, assess};
use crate::state::Cause;

/// The stage of a review cycle (`DOMAIN_MODEL.md`, `ReviewCycle`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CycleStage {
    /// The one full independent review.
    Comprehensive,
    /// Targeted verification of a fix set.
    Verification,
}

impl CycleStage {
    const fn name(self) -> &'static str {
        match self {
            Self::Comprehensive => "COMPREHENSIVE",
            Self::Verification => "VERIFICATION",
        }
    }
}

/// Where a fix set came from (`ORCHESTRATION_ENGINE.md` section 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixSetKind {
    /// Fixes for the blocking findings of the review.
    ReviewFix,
    /// Fixes after a failed integration validation.
    IntegrationValidationFix,
    /// A conflict resolution.
    ConflictResolution,
}

/// Why a cycle is being started. None of these, alone, can start a second comprehensive review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StartOrigin {
    /// An ordinary start.
    New,
    /// The fixed point advanced because the integration tip was merged into the worker branch.
    FixedPointAdvance,
    /// A retry.
    Retry,
    /// Recovery after an interruption.
    Recovery,
}

/// A rejected review operation. Rejection never changes the history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewError {
    /// A second comprehensive review without a recorded user decision, whatever the origin.
    SecondComprehensiveNeedsUserDecision,
    /// A verification with no fix set recorded since the last review.
    VerificationNeedsFixSet,
    /// A fix set with no completed review cycle before it.
    FixSetNeedsCompletedReview,
    /// A fix set after a stall needs a recorded user decision.
    StalledNeedsUserDecision,
    /// Another cycle is still in flight.
    CycleInFlight,
    /// No cycle of that stage is in flight.
    NoCycleInFlight {
        /// The stage that was being completed.
        expected: CycleStage,
    },
}

impl fmt::Display for ReviewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SecondComprehensiveNeedsUserDecision => f.write_str(
                "a ticket has one comprehensive review; another needs a recorded user decision",
            ),
            Self::VerificationNeedsFixSet => {
                f.write_str("a verification follows a fix set and none is recorded")
            }
            Self::FixSetNeedsCompletedReview => {
                f.write_str("a fix set needs a completed review cycle before it")
            }
            Self::StalledNeedsUserDecision => {
                f.write_str("the review is stalled; only a recorded user decision may continue it")
            }
            Self::CycleInFlight => f.write_str("another review cycle is in flight"),
            Self::NoCycleInFlight { expected } => {
                write!(f, "no {} cycle is in flight", expected.name())
            }
        }
    }
}

impl std::error::Error for ReviewError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CycleRecord {
    stage: CycleStage,
    completed: bool,
}

/// The review cycles of one ticket.
#[derive(Debug, Clone, Default)]
pub struct ReviewHistory {
    cycles: Vec<CycleRecord>,
    pending_fix: Option<FixSetKind>,
    stalled: bool,
    passed: bool,
    unresolved: BTreeSet<FindingId>,
}

impl ReviewHistory {
    /// An empty history: no review has started.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many cycles have been started, in order. The ordinal of a cycle is its position here, starting at 1.
    pub fn cycle_count(&self) -> usize {
        self.cycles.len()
    }

    /// How many comprehensive cycles have been started.
    pub fn comprehensive_count(&self) -> usize {
        self.cycles
            .iter()
            .filter(|c| c.stage == CycleStage::Comprehensive)
            .count()
    }

    /// Blocking findings that still need a verified fix.
    pub fn unresolved(&self) -> &BTreeSet<FindingId> {
        &self.unresolved
    }

    /// Whether the last verification escalated and no user decision has continued the review.
    pub fn is_stalled(&self) -> bool {
        self.stalled
    }

    /// Whether the review currently stands at a pass. A pass is retracted by any later fix set.
    pub fn is_passed(&self) -> bool {
        self.passed
    }

    fn in_flight(&self) -> Option<(usize, CycleRecord)> {
        self.cycles
            .last()
            .filter(|c| !c.completed)
            .map(|c| (self.cycles.len(), *c))
    }

    /// Start a cycle and return its ordinal (starting at 1).
    ///
    /// A start while a cycle of the same stage is in flight and the origin is a retry or a recovery re-runs that cycle
    /// (same ordinal); it never creates another. A second `COMPREHENSIVE` cycle needs `Cause::UserDecision`, whatever
    /// the origin. A `VERIFICATION` cycle needs a recorded fix set.
    pub fn begin(
        &mut self,
        stage: CycleStage,
        origin: StartOrigin,
        cause: Cause,
    ) -> Result<usize, ReviewError> {
        if let Some((ordinal, record)) = self.in_flight() {
            let rerun = record.stage == stage
                && matches!(origin, StartOrigin::Retry | StartOrigin::Recovery);
            return if rerun {
                Ok(ordinal)
            } else {
                Err(ReviewError::CycleInFlight)
            };
        }
        match stage {
            CycleStage::Comprehensive => {
                if self.comprehensive_count() > 0 && cause != Cause::UserDecision {
                    return Err(ReviewError::SecondComprehensiveNeedsUserDecision);
                }
            }
            CycleStage::Verification => {
                if self.pending_fix.is_none() {
                    return Err(ReviewError::VerificationNeedsFixSet);
                }
            }
        }
        self.cycles.push(CycleRecord {
            stage,
            completed: false,
        });
        Ok(self.cycles.len())
    }

    /// Record that a fix set was committed, so a verification may follow. After a stall only a user decision may
    /// direct a fix.
    pub fn record_fix_set(&mut self, kind: FixSetKind, cause: Cause) -> Result<(), ReviewError> {
        if self.in_flight().is_some() {
            return Err(ReviewError::CycleInFlight);
        }
        if self.cycles.is_empty() {
            return Err(ReviewError::FixSetNeedsCompletedReview);
        }
        if self.stalled && cause != Cause::UserDecision {
            return Err(ReviewError::StalledNeedsUserDecision);
        }
        self.pending_fix = Some(self.pending_fix.unwrap_or(kind));
        self.stalled = false;
        self.passed = false;
        Ok(())
    }

    fn complete(&mut self, expected: CycleStage) -> Result<(), ReviewError> {
        match self.cycles.last_mut() {
            Some(record) if !record.completed && record.stage == expected => {
                record.completed = true;
                Ok(())
            }
            _ => Err(ReviewError::NoCycleInFlight { expected }),
        }
    }

    /// Finish the in-flight comprehensive cycle with the reviewer's findings. Vela's policy, not the reviewer's prose,
    /// decides which findings block. No blocking finding is a pass (there is nothing to verify); otherwise the
    /// blocking findings must be fixed and verified.
    pub fn complete_comprehensive(
        &mut self,
        findings: &[Finding],
    ) -> Result<ReviewOutcome, ReviewError> {
        self.complete(CycleStage::Comprehensive)?;
        self.unresolved = findings
            .iter()
            .filter(|f| f.is_blocking())
            .map(|f| f.id().clone())
            .collect();
        self.pending_fix = None;
        self.stalled = false;
        if self.unresolved.is_empty() {
            self.passed = true;
            Ok(ReviewOutcome::Pass)
        } else {
            self.passed = false;
            Ok(ReviewOutcome::FixRequired {
                blocking: self.unresolved.iter().cloned().collect(),
            })
        }
    }

    /// Finish the in-flight verification cycle. The result is `Pass` only when [`super::evaluate_verification`] says
    /// so over the blocking findings still open; every other result is an escalation, and the history is then stalled.
    pub fn complete_verification(
        &mut self,
        verdicts: &[Verdict],
        fix_diff_findings: &[Finding],
        observations: &[Finding],
    ) -> Result<ReviewOutcome, ReviewError> {
        self.complete(CycleStage::Verification)?;
        let (outcome, still_open) = assess(VerificationInput {
            unresolved: &self.unresolved,
            verdicts,
            fix_diff_findings,
            observations,
        });
        self.unresolved = still_open;
        self.pending_fix = None;
        match outcome {
            ReviewOutcome::Pass => {
                self.passed = true;
                self.stalled = false;
            }
            _ => {
                self.passed = false;
                self.stalled = true;
            }
        }
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::ids::ReviewSeverity;
    use crate::review::{ReviewAxis, ReviewCategory, VerdictKind};

    const ORIGINS: [StartOrigin; 4] = [
        StartOrigin::New,
        StartOrigin::FixedPointAdvance,
        StartOrigin::Retry,
        StartOrigin::Recovery,
    ];
    const CAUSES: [Cause; 2] = [Cause::System, Cause::UserDecision];

    fn finding(n: u32, severity: ReviewSeverity) -> Finding {
        Finding::new(
            ReviewAxis::Spec,
            severity,
            ReviewCategory::Correctness,
            format!("src/f{n}.rs"),
            "FR-013",
            "e",
            "f",
        )
    }

    fn resolved(f: &Finding) -> Verdict {
        Verdict::new(f.id().clone(), VerdictKind::Resolved, "fix and test")
    }

    /// A history with a completed comprehensive review that reported the given findings.
    fn reviewed(findings: &[Finding]) -> ReviewHistory {
        let mut h = ReviewHistory::new();
        h.begin(CycleStage::Comprehensive, StartOrigin::New, Cause::System)
            .unwrap();
        h.complete_comprehensive(findings).unwrap();
        h
    }

    #[test]
    fn the_first_comprehensive_review_is_allowed_for_every_origin_and_cause() {
        for origin in ORIGINS {
            for cause in CAUSES {
                let mut h = ReviewHistory::new();
                assert_eq!(h.begin(CycleStage::Comprehensive, origin, cause), Ok(1));
            }
        }
    }

    #[test]
    fn a_second_comprehensive_review_is_rejected_without_a_user_decision_whatever_the_origin() {
        for blocking in [false, true] {
            let findings = if blocking {
                vec![finding(1, ReviewSeverity::High)]
            } else {
                vec![]
            };
            for origin in ORIGINS {
                let mut h = reviewed(&findings);
                let before = h.cycle_count();
                assert_eq!(
                    h.begin(CycleStage::Comprehensive, origin, Cause::System),
                    Err(ReviewError::SecondComprehensiveNeedsUserDecision),
                    "{origin:?}"
                );
                assert_eq!(h.cycle_count(), before);
                assert_eq!(h.comprehensive_count(), 1);
            }
        }
    }

    #[test]
    fn a_second_comprehensive_review_is_rejected_after_a_fix_set_a_stall_and_a_pass() {
        // After a fix set (fixed-point advance, retry, recovery all included).
        let f = finding(1, ReviewSeverity::High);
        let mut h = reviewed(std::slice::from_ref(&f));
        h.record_fix_set(FixSetKind::ReviewFix, Cause::System)
            .unwrap();
        for origin in ORIGINS {
            assert_eq!(
                h.begin(CycleStage::Comprehensive, origin, Cause::System),
                Err(ReviewError::SecondComprehensiveNeedsUserDecision)
            );
        }
        // After a stall.
        h.begin(CycleStage::Verification, StartOrigin::New, Cause::System)
            .unwrap();
        h.complete_verification(&[], &[], &[]).unwrap();
        assert!(h.is_stalled());
        for origin in ORIGINS {
            assert_eq!(
                h.begin(CycleStage::Comprehensive, origin, Cause::System),
                Err(ReviewError::SecondComprehensiveNeedsUserDecision)
            );
        }
        // After a pass.
        let mut passed = reviewed(&[]);
        assert!(passed.is_passed());
        assert_eq!(
            passed.begin(
                CycleStage::Comprehensive,
                StartOrigin::Recovery,
                Cause::System
            ),
            Err(ReviewError::SecondComprehensiveNeedsUserDecision)
        );
    }

    #[test]
    fn a_user_decision_allows_another_comprehensive_review_and_replaces_the_findings() {
        let f = finding(1, ReviewSeverity::High);
        let mut h = reviewed(std::slice::from_ref(&f));
        assert_eq!(
            h.begin(
                CycleStage::Comprehensive,
                StartOrigin::New,
                Cause::UserDecision
            ),
            Ok(2)
        );
        assert_eq!(
            h.complete_comprehensive(&[]).unwrap(),
            ReviewOutcome::Pass,
            "the new full review supersedes the old findings"
        );
        assert_eq!(h.comprehensive_count(), 2);
    }

    #[test]
    fn a_retry_or_recovery_of_an_in_flight_cycle_reruns_the_same_cycle() {
        let mut h = ReviewHistory::new();
        assert_eq!(
            h.begin(CycleStage::Comprehensive, StartOrigin::New, Cause::System),
            Ok(1)
        );
        for origin in [StartOrigin::Retry, StartOrigin::Recovery] {
            assert_eq!(
                h.begin(CycleStage::Comprehensive, origin, Cause::System),
                Ok(1)
            );
        }
        assert_eq!(h.cycle_count(), 1);
        for origin in [StartOrigin::New, StartOrigin::FixedPointAdvance] {
            assert_eq!(
                h.begin(CycleStage::Comprehensive, origin, Cause::UserDecision),
                Err(ReviewError::CycleInFlight)
            );
        }
        assert_eq!(
            h.begin(
                CycleStage::Verification,
                StartOrigin::Recovery,
                Cause::System
            ),
            Err(ReviewError::CycleInFlight)
        );
    }

    #[test]
    fn a_verification_needs_a_fix_set() {
        let f = finding(1, ReviewSeverity::High);
        let mut h = reviewed(std::slice::from_ref(&f));
        for origin in ORIGINS {
            for cause in CAUSES {
                assert_eq!(
                    h.begin(CycleStage::Verification, origin, cause),
                    Err(ReviewError::VerificationNeedsFixSet)
                );
            }
        }
        assert_eq!(
            ReviewHistory::new().begin(CycleStage::Verification, StartOrigin::New, Cause::System),
            Err(ReviewError::VerificationNeedsFixSet)
        );
        h.record_fix_set(FixSetKind::ReviewFix, Cause::System)
            .unwrap();
        assert_eq!(
            h.begin(CycleStage::Verification, StartOrigin::New, Cause::System),
            Ok(2)
        );
    }

    #[test]
    fn a_fix_set_needs_a_completed_review_and_no_cycle_in_flight() {
        let mut h = ReviewHistory::new();
        assert_eq!(
            h.record_fix_set(FixSetKind::ReviewFix, Cause::System),
            Err(ReviewError::FixSetNeedsCompletedReview)
        );
        h.begin(CycleStage::Comprehensive, StartOrigin::New, Cause::System)
            .unwrap();
        assert_eq!(
            h.record_fix_set(FixSetKind::ReviewFix, Cause::System),
            Err(ReviewError::CycleInFlight)
        );
    }

    #[test]
    fn no_blocking_finding_means_nothing_to_verify() {
        let mut h = reviewed(&[
            finding(1, ReviewSeverity::Low),
            finding(2, ReviewSeverity::Info),
        ]);
        assert!(h.is_passed());
        assert!(h.unresolved().is_empty());
        // The verification is skipped: it can only follow a fix set.
        assert_eq!(
            h.begin(CycleStage::Verification, StartOrigin::New, Cause::System),
            Err(ReviewError::VerificationNeedsFixSet)
        );
    }

    #[test]
    fn blocking_findings_require_a_fix_then_a_verification_that_passes_when_all_resolved() {
        let a = finding(1, ReviewSeverity::High);
        let b = finding(2, ReviewSeverity::Medium);
        let c = finding(3, ReviewSeverity::Low);
        let mut h = ReviewHistory::new();
        h.begin(CycleStage::Comprehensive, StartOrigin::New, Cause::System)
            .unwrap();
        let outcome = h
            .complete_comprehensive(&[a.clone(), b.clone(), c.clone()])
            .unwrap();
        assert_eq!(
            outcome,
            ReviewOutcome::FixRequired {
                blocking: {
                    let mut ids = vec![a.id().clone(), b.id().clone()];
                    ids.sort();
                    ids
                }
            }
        );
        assert!(!h.is_passed());
        h.record_fix_set(FixSetKind::ReviewFix, Cause::System)
            .unwrap();
        h.begin(CycleStage::Verification, StartOrigin::New, Cause::System)
            .unwrap();
        // The verification refers to the same ids the comprehensive review produced.
        let outcome = h
            .complete_verification(&[resolved(&a), resolved(&b)], &[], &[])
            .unwrap();
        assert_eq!(outcome, ReviewOutcome::Pass);
        assert!(h.is_passed() && h.unresolved().is_empty());
    }

    #[test]
    fn every_failed_verification_stalls_with_review_stalled() {
        use crate::state::InterventionKind;
        let a = finding(1, ReviewSeverity::High);
        let cases: Vec<(Vec<Verdict>, Vec<Finding>, Vec<Finding>)> = vec![
            (vec![], vec![], vec![]),
            (
                vec![Verdict::new(
                    a.id().clone(),
                    VerdictKind::NotResolved,
                    "still broken",
                )],
                vec![],
                vec![],
            ),
            (
                vec![resolved(&a)],
                vec![finding(9, ReviewSeverity::Medium)],
                vec![],
            ),
            (
                vec![resolved(&a)],
                vec![],
                vec![finding(8, ReviewSeverity::High)],
            ),
        ];
        for (verdicts, fix, obs) in cases {
            let mut h = reviewed(std::slice::from_ref(&a));
            h.record_fix_set(FixSetKind::ReviewFix, Cause::System)
                .unwrap();
            h.begin(CycleStage::Verification, StartOrigin::New, Cause::System)
                .unwrap();
            match h.complete_verification(&verdicts, &fix, &obs).unwrap() {
                ReviewOutcome::Escalate(e) => {
                    assert_eq!(e.kind(), InterventionKind::ReviewStalled)
                }
                other => panic!("expected escalation, got {other:?}"),
            }
            assert!(h.is_stalled());
            assert!(!h.is_passed());
        }
    }

    #[test]
    fn a_stalled_review_continues_only_through_a_user_decision_and_still_needs_verified_resolution()
    {
        let a = finding(1, ReviewSeverity::High);
        let mut h = reviewed(std::slice::from_ref(&a));
        h.record_fix_set(FixSetKind::ReviewFix, Cause::System)
            .unwrap();
        h.begin(CycleStage::Verification, StartOrigin::New, Cause::System)
            .unwrap();
        h.complete_verification(&[], &[], &[]).unwrap();
        assert!(h.is_stalled());
        // Neither the system nor a retry can continue it.
        assert_eq!(
            h.record_fix_set(FixSetKind::ReviewFix, Cause::System),
            Err(ReviewError::StalledNeedsUserDecision)
        );
        assert_eq!(
            h.begin(CycleStage::Verification, StartOrigin::Retry, Cause::System),
            Err(ReviewError::VerificationNeedsFixSet)
        );
        // A user-directed fix and verification: the blocking finding is still open and still needs RESOLVED.
        h.record_fix_set(FixSetKind::ReviewFix, Cause::UserDecision)
            .unwrap();
        assert!(!h.is_stalled());
        assert!(h.unresolved().contains(a.id()));
        h.begin(
            CycleStage::Verification,
            StartOrigin::New,
            Cause::UserDecision,
        )
        .unwrap();
        let outcome = h.complete_verification(&[], &[], &[]).unwrap();
        assert!(matches!(outcome, ReviewOutcome::Escalate(_)));
        h.record_fix_set(FixSetKind::ReviewFix, Cause::UserDecision)
            .unwrap();
        h.begin(
            CycleStage::Verification,
            StartOrigin::New,
            Cause::UserDecision,
        )
        .unwrap();
        let outcome = h.complete_verification(&[resolved(&a)], &[], &[]).unwrap();
        assert_eq!(outcome, ReviewOutcome::Pass);
    }

    #[test]
    fn a_fix_diff_defect_stays_open_until_a_later_verification_resolves_it() {
        let a = finding(1, ReviewSeverity::High);
        let d = finding(2, ReviewSeverity::Medium);
        let mut h = reviewed(std::slice::from_ref(&a));
        h.record_fix_set(FixSetKind::ReviewFix, Cause::System)
            .unwrap();
        h.begin(CycleStage::Verification, StartOrigin::New, Cause::System)
            .unwrap();
        h.complete_verification(&[resolved(&a)], std::slice::from_ref(&d), &[])
            .unwrap();
        assert_eq!(h.unresolved().len(), 1);
        assert!(h.unresolved().contains(d.id()));
        h.record_fix_set(FixSetKind::ReviewFix, Cause::UserDecision)
            .unwrap();
        h.begin(
            CycleStage::Verification,
            StartOrigin::New,
            Cause::UserDecision,
        )
        .unwrap();
        let outcome = h.complete_verification(&[], &[], &[]).unwrap();
        assert!(matches!(outcome, ReviewOutcome::Escalate(_)));
        h.record_fix_set(FixSetKind::ReviewFix, Cause::UserDecision)
            .unwrap();
        h.begin(
            CycleStage::Verification,
            StartOrigin::New,
            Cause::UserDecision,
        )
        .unwrap();
        let outcome = h.complete_verification(&[resolved(&d)], &[], &[]).unwrap();
        assert_eq!(outcome, ReviewOutcome::Pass);
    }

    #[test]
    fn a_fix_after_a_pass_needs_its_own_verification() {
        // Integration validation failed after a pass: the fix is verified, not reviewed again.
        let mut h = reviewed(&[]);
        h.record_fix_set(FixSetKind::IntegrationValidationFix, Cause::System)
            .unwrap();
        assert!(!h.is_passed());
        h.begin(CycleStage::Verification, StartOrigin::New, Cause::System)
            .unwrap();
        let bad = [finding(4, ReviewSeverity::High)];
        let outcome = h.complete_verification(&[], &bad, &[]).unwrap();
        assert!(matches!(outcome, ReviewOutcome::Escalate(_)));
    }

    #[test]
    fn completing_the_wrong_stage_or_nothing_is_rejected() {
        let mut h = ReviewHistory::new();
        assert_eq!(
            h.complete_comprehensive(&[]),
            Err(ReviewError::NoCycleInFlight {
                expected: CycleStage::Comprehensive
            })
        );
        h.begin(CycleStage::Comprehensive, StartOrigin::New, Cause::System)
            .unwrap();
        assert_eq!(
            h.complete_verification(&[], &[], &[]),
            Err(ReviewError::NoCycleInFlight {
                expected: CycleStage::Verification
            })
        );
    }

    #[derive(Debug, Clone)]
    enum Op {
        Begin(bool, usize, bool),
        Fix(usize, bool),
        Comprehensive(Vec<(u32, usize)>),
        Verification(Vec<(u32, bool, bool)>, Vec<(u32, usize)>, Vec<(u32, usize)>),
    }

    fn severity(i: usize) -> ReviewSeverity {
        [
            ReviewSeverity::Info,
            ReviewSeverity::Low,
            ReviewSeverity::Medium,
            ReviewSeverity::High,
        ][i % 4]
    }

    fn op_strategy() -> impl Strategy<Value = Op> {
        let findings = proptest::collection::vec((0u32..4, 0usize..4), 0..4);
        prop_oneof![
            (any::<bool>(), 0usize..4, any::<bool>()).prop_map(|(c, o, u)| Op::Begin(c, o, u)),
            (0usize..3, any::<bool>()).prop_map(|(k, u)| Op::Fix(k, u)),
            findings.clone().prop_map(Op::Comprehensive),
            (
                proptest::collection::vec((0u32..4, any::<bool>(), any::<bool>()), 0..6),
                findings.clone(),
                findings
            )
                .prop_map(|(v, f, o)| Op::Verification(v, f, o)),
        ]
    }

    proptest! {
        /// No sequence of operations, with any origin or cause, ever yields a pass while a blocking finding is
        /// unresolved, and a second comprehensive review is only ever started on a user decision. The model below is
        /// written independently of the implementation.
        #[test]
        fn success_is_unreachable_with_an_unresolved_blocking_finding(
            ops in proptest::collection::vec(op_strategy(), 0..40)
        ) {
            let mut h = ReviewHistory::new();
            let mut open: BTreeSet<FindingId> = BTreeSet::new();
            for op in ops {
                match op {
                    Op::Begin(comprehensive, origin, user) => {
                        let stage = if comprehensive {
                            CycleStage::Comprehensive
                        } else {
                            CycleStage::Verification
                        };
                        let cause = if user { Cause::UserDecision } else { Cause::System };
                        let comps_before = h.comprehensive_count();
                        let cycles_before = h.cycle_count();
                        let result = h.begin(stage, ORIGINS[origin], cause);
                        if result.is_ok()
                            && h.cycle_count() > cycles_before
                            && stage == CycleStage::Comprehensive
                            && comps_before > 0
                        {
                            prop_assert!(user, "a second comprehensive review started without a user decision");
                        }
                    }
                    Op::Fix(kind, user) => {
                        let kind = [
                            FixSetKind::ReviewFix,
                            FixSetKind::IntegrationValidationFix,
                            FixSetKind::ConflictResolution,
                        ][kind];
                        let cause = if user { Cause::UserDecision } else { Cause::System };
                        let _ = h.record_fix_set(kind, cause);
                    }
                    Op::Comprehensive(list) => {
                        let findings: Vec<Finding> =
                            list.iter().map(|(n, s)| finding(*n, severity(*s))).collect();
                        if let Ok(outcome) = h.complete_comprehensive(&findings) {
                            open = findings
                                .iter()
                                .filter(|f| f.is_blocking())
                                .map(|f| f.id().clone())
                                .collect();
                            if outcome == ReviewOutcome::Pass {
                                prop_assert!(open.is_empty());
                            }
                        }
                    }
                    Op::Verification(verdicts, fix, obs) => {
                        let verdicts: Vec<Verdict> = verdicts
                            .iter()
                            .map(|(n, ok, ev)| {
                                Verdict::new(
                                    finding(*n, ReviewSeverity::High).id().clone(),
                                    if *ok { VerdictKind::Resolved } else { VerdictKind::NotResolved },
                                    if *ev { "evidence" } else { "" },
                                )
                            })
                            .collect();
                        let fix: Vec<Finding> =
                            fix.iter().map(|(n, s)| finding(*n + 10, severity(*s))).collect();
                        let obs: Vec<Finding> =
                            obs.iter().map(|(n, s)| finding(*n + 20, severity(*s))).collect();
                        if let Ok(outcome) = h.complete_verification(&verdicts, &fix, &obs) {
                            let mut next: BTreeSet<FindingId> = BTreeSet::new();
                            for id in &open {
                                let about: Vec<&Verdict> =
                                    verdicts.iter().filter(|v| &v.finding_id == id).collect();
                                let good = !about.is_empty()
                                    && about.iter().all(|v| {
                                        v.kind == VerdictKind::Resolved
                                            && !v.evidence.trim().is_empty()
                                    });
                                if !good {
                                    next.insert(id.clone());
                                }
                            }
                            next.extend(fix.iter().filter(|f| f.is_blocking()).map(|f| f.id().clone()));
                            next.extend(
                                obs.iter().filter(|f| f.blocks_as_observation()).map(|f| f.id().clone()),
                            );
                            open = next;
                            if outcome == ReviewOutcome::Pass {
                                prop_assert!(open.is_empty());
                            } else {
                                prop_assert!(matches!(outcome, ReviewOutcome::Escalate(_)));
                                prop_assert!(h.is_stalled());
                            }
                        }
                    }
                }
                if h.is_passed() {
                    prop_assert!(open.is_empty());
                    prop_assert!(h.unresolved().is_empty());
                }
                prop_assert_eq!(h.unresolved(), &open);
            }
        }
    }
}
