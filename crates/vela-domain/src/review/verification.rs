//! Verification outcome evaluation (`REVIEW_PROTOCOL.md` steps 6 to 8 and "Exit"; `PROMPT_CONTRACTS.md`, verifier
//! output contract).
//!
//! The outcome is a pure function of the verdicts, the fix-diff findings and the observations. It takes no cause, no
//! clock and no counter: there is nothing a timeout, a default, a retry or a human decision could pass in to turn an
//! unresolved blocking finding into a pass.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::finding::Finding;
use super::fingerprint::FindingId;
use crate::state::InterventionKind;

/// The verifier's verdict on one blocking finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VerdictKind {
    /// The fix resolves the finding (only with evidence).
    Resolved,
    /// The finding is not resolved.
    NotResolved,
}

/// One verdict as returned by the verifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verdict {
    /// The finding the verdict is about.
    pub finding_id: FindingId,
    /// `RESOLVED` or `NOT_RESOLVED`.
    pub kind: VerdictKind,
    /// The evidence (the fix and its regression test). `RESOLVED` without evidence counts as `NOT_RESOLVED`.
    pub evidence: String,
}

impl Verdict {
    /// A verdict.
    pub fn new(finding_id: FindingId, kind: VerdictKind, evidence: impl Into<String>) -> Self {
        Self {
            finding_id,
            kind,
            evidence: evidence.into(),
        }
    }
}

/// Why a verification failed. Each variant names the finding responsible.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StallReason {
    /// The verifier reported the blocking finding `NOT_RESOLVED` (or `RESOLVED` without evidence, or contradicted
    /// itself).
    NotResolved(FindingId),
    /// The verifier gave no verdict for the blocking finding; a missing verdict is `NOT_RESOLVED`.
    MissingVerdict(FindingId),
    /// The fix diff itself contains a blocking defect.
    BlockingDefectInFixDiff(FindingId),
    /// A high-severity defect noticed outside the fix diff is open.
    OpenHighSeverity(FindingId),
}

/// Escalation to a human: `NEEDS_HUMAN` with kind `REVIEW_STALLED`. It always carries at least one reason, and its
/// kind cannot be anything else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Escalation {
    reasons: Vec<StallReason>,
}

impl Escalation {
    fn new(reasons: Vec<StallReason>) -> Self {
        debug_assert!(!reasons.is_empty());
        Self { reasons }
    }

    /// The intervention kind: always `REVIEW_STALLED`.
    pub const fn kind(&self) -> InterventionKind {
        InterventionKind::ReviewStalled
    }

    /// Why the verification failed.
    pub fn reasons(&self) -> &[StallReason] {
        &self.reasons
    }
}

/// The outcome of finishing a review cycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewOutcome {
    /// Nothing blocking remains.
    Pass,
    /// The comprehensive review reported blocking findings: they are fixed, then verified. Not a failure.
    FixRequired {
        /// The blocking findings to fix.
        blocking: Vec<FindingId>,
    },
    /// The worker is `NEEDS_HUMAN` with kind `REVIEW_STALLED`.
    Escalate(Escalation),
}

/// What the verifier and the fix diff reported.
#[derive(Debug, Clone, Copy)]
pub struct VerificationInput<'a> {
    /// Blocking findings that still need a `RESOLVED` verdict: those of the comprehensive review, plus anything a
    /// previous failed verification left open.
    pub unresolved: &'a BTreeSet<FindingId>,
    /// The verifier's verdicts.
    pub verdicts: &'a [Verdict],
    /// Findings in the fix diff (`review_head_sha..fix_head_sha`).
    pub fix_diff_findings: &'a [Finding],
    /// Observations outside the fix diff.
    pub observations: &'a [Finding],
}

/// Evaluate a verification. `Pass` only when every unresolved blocking finding has a `RESOLVED` verdict with
/// evidence, the fix diff has no blocking finding, and no high-severity observation is open. Every other input is an
/// escalation to `NEEDS_HUMAN` (`REVIEW_STALLED`); there is no other result.
pub fn evaluate_verification(input: VerificationInput<'_>) -> ReviewOutcome {
    assess(input).0
}

/// The outcome plus the findings still open afterwards (carried into a later, human-authorized verification).
pub(super) fn assess(input: VerificationInput<'_>) -> (ReviewOutcome, BTreeSet<FindingId>) {
    // A finding is resolved only when every verdict about it says so, with evidence. Contradictory or evidence-free
    // verdicts fail closed.
    let mut verdicts_by_id: BTreeMap<&FindingId, bool> = BTreeMap::new();
    for verdict in input.verdicts {
        let accepted = verdict.kind == VerdictKind::Resolved && !verdict.evidence.trim().is_empty();
        let entry = verdicts_by_id.entry(&verdict.finding_id).or_insert(true);
        *entry = *entry && accepted;
    }

    let mut reasons = Vec::new();
    let mut still_open = BTreeSet::new();
    for id in input.unresolved {
        match verdicts_by_id.get(id) {
            Some(true) => {}
            Some(false) => {
                reasons.push(StallReason::NotResolved(id.clone()));
                still_open.insert(id.clone());
            }
            None => {
                reasons.push(StallReason::MissingVerdict(id.clone()));
                still_open.insert(id.clone());
            }
        }
    }
    for finding in input.fix_diff_findings {
        if finding.is_blocking() {
            reasons.push(StallReason::BlockingDefectInFixDiff(finding.id().clone()));
            still_open.insert(finding.id().clone());
        }
    }
    for finding in input.observations {
        if finding.blocks_as_observation() {
            reasons.push(StallReason::OpenHighSeverity(finding.id().clone()));
            still_open.insert(finding.id().clone());
        }
    }

    if reasons.is_empty() {
        (ReviewOutcome::Pass, still_open)
    } else {
        (
            ReviewOutcome::Escalate(Escalation::new(reasons)),
            still_open,
        )
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::ids::ReviewSeverity;
    use crate::review::{ReviewAxis, ReviewCategory};

    fn id(n: u32) -> FindingId {
        crate::review::fingerprint("correctness", &format!("src/f{n}.rs"), "FR-013")
    }

    fn defect(n: u32, severity: ReviewSeverity) -> Finding {
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

    fn resolved(n: u32) -> Verdict {
        Verdict::new(id(n), VerdictKind::Resolved, "fix and regression test")
    }

    fn not_resolved(n: u32) -> Verdict {
        Verdict::new(id(n), VerdictKind::NotResolved, "still broken")
    }

    fn run(
        unresolved: &[u32],
        verdicts: &[Verdict],
        fix_diff: &[Finding],
        observations: &[Finding],
    ) -> ReviewOutcome {
        let set: BTreeSet<FindingId> = unresolved.iter().map(|n| id(*n)).collect();
        evaluate_verification(VerificationInput {
            unresolved: &set,
            verdicts,
            fix_diff_findings: fix_diff,
            observations,
        })
    }

    fn reasons(outcome: &ReviewOutcome) -> Vec<StallReason> {
        match outcome {
            ReviewOutcome::Escalate(e) => {
                assert_eq!(e.kind(), InterventionKind::ReviewStalled);
                e.reasons().to_vec()
            }
            other => panic!("expected escalation, got {other:?}"),
        }
    }

    #[test]
    fn every_blocking_finding_resolved_and_nothing_else_open_passes() {
        assert_eq!(
            run(&[1, 2], &[resolved(1), resolved(2)], &[], &[]),
            ReviewOutcome::Pass
        );
    }

    #[test]
    fn non_blocking_fix_diff_findings_and_observations_do_not_block() {
        let fix = [
            defect(7, ReviewSeverity::Low),
            defect(8, ReviewSeverity::Info),
        ];
        let obs = [defect(9, ReviewSeverity::Medium)];
        assert_eq!(run(&[1], &[resolved(1)], &fix, &obs), ReviewOutcome::Pass);
    }

    #[test]
    fn nothing_to_verify_with_a_clean_fix_diff_passes() {
        assert_eq!(run(&[], &[], &[], &[]), ReviewOutcome::Pass);
    }

    #[test]
    fn not_resolved_escalates() {
        let outcome = run(&[1, 2], &[resolved(1), not_resolved(2)], &[], &[]);
        assert_eq!(reasons(&outcome), vec![StallReason::NotResolved(id(2))]);
    }

    #[test]
    fn a_missing_verdict_is_not_resolved_and_escalates() {
        let outcome = run(&[1, 2], &[resolved(1)], &[], &[]);
        assert_eq!(reasons(&outcome), vec![StallReason::MissingVerdict(id(2))]);
        let none = run(&[1], &[], &[], &[]);
        assert_eq!(reasons(&none), vec![StallReason::MissingVerdict(id(1))]);
    }

    #[test]
    fn resolved_without_evidence_or_contradicted_fails_closed() {
        let bare = Verdict::new(id(1), VerdictKind::Resolved, "   ");
        assert_eq!(
            reasons(&run(&[1], &[bare], &[], &[])),
            vec![StallReason::NotResolved(id(1))]
        );
        let both = [resolved(1), not_resolved(1)];
        assert_eq!(
            reasons(&run(&[1], &both, &[], &[])),
            vec![StallReason::NotResolved(id(1))]
        );
        let both_reversed = [not_resolved(1), resolved(1)];
        assert_eq!(
            reasons(&run(&[1], &both_reversed, &[], &[])),
            vec![StallReason::NotResolved(id(1))]
        );
    }

    #[test]
    fn a_blocking_defect_in_the_fix_diff_escalates() {
        let fix = [defect(5, ReviewSeverity::Medium)];
        let outcome = run(&[1], &[resolved(1)], &fix, &[]);
        assert_eq!(
            reasons(&outcome),
            vec![StallReason::BlockingDefectInFixDiff(id(5))]
        );
    }

    #[test]
    fn an_open_high_severity_observation_escalates_even_with_nothing_to_verify() {
        let obs = [defect(6, ReviewSeverity::High)];
        let outcome = run(&[], &[], &[], &obs);
        assert_eq!(
            reasons(&outcome),
            vec![StallReason::OpenHighSeverity(id(6))]
        );
    }

    #[test]
    fn a_verdict_for_an_unknown_finding_resolves_nothing() {
        let outcome = run(&[1], &[resolved(2)], &[], &[]);
        assert_eq!(reasons(&outcome), vec![StallReason::MissingVerdict(id(1))]);
    }

    /// The full outcome table: every combination of verdict state for one blocking finding, fix-diff state and
    /// observation state, against an independent oracle.
    #[test]
    fn outcome_table_over_every_combination() {
        #[derive(Clone, Copy, Debug)]
        enum V {
            Resolved,
            NotResolved,
            Missing,
        }
        #[derive(Clone, Copy, Debug)]
        enum Sev {
            None,
            NonBlocking,
            Blocking,
        }
        for v in [V::Resolved, V::NotResolved, V::Missing] {
            for fix in [Sev::None, Sev::NonBlocking, Sev::Blocking] {
                for obs in [Sev::None, Sev::NonBlocking, Sev::Blocking] {
                    let verdicts: Vec<Verdict> = match v {
                        V::Resolved => vec![resolved(1)],
                        V::NotResolved => vec![not_resolved(1)],
                        V::Missing => vec![],
                    };
                    let fixes: Vec<Finding> = match fix {
                        Sev::None => vec![],
                        Sev::NonBlocking => vec![defect(2, ReviewSeverity::Low)],
                        Sev::Blocking => vec![defect(2, ReviewSeverity::High)],
                    };
                    let observations: Vec<Finding> = match obs {
                        Sev::None => vec![],
                        Sev::NonBlocking => vec![defect(3, ReviewSeverity::Medium)],
                        Sev::Blocking => vec![defect(3, ReviewSeverity::High)],
                    };
                    let expect_pass = matches!(v, V::Resolved)
                        && !matches!(fix, Sev::Blocking)
                        && !matches!(obs, Sev::Blocking);
                    let outcome = run(&[1], &verdicts, &fixes, &observations);
                    if expect_pass {
                        assert_eq!(outcome, ReviewOutcome::Pass, "{v:?} {fix:?} {obs:?}");
                    } else {
                        let r = reasons(&outcome);
                        assert_eq!(
                            r.len(),
                            usize::from(!matches!(v, V::Resolved))
                                + usize::from(matches!(fix, Sev::Blocking))
                                + usize::from(matches!(obs, Sev::Blocking)),
                            "{v:?} {fix:?} {obs:?}"
                        );
                    }
                }
            }
        }
    }

    fn verdict_strategy() -> impl Strategy<Value = Verdict> {
        (0u32..6, any::<bool>(), any::<bool>()).prop_map(|(n, resolved, evidence)| {
            Verdict::new(
                id(n),
                if resolved {
                    VerdictKind::Resolved
                } else {
                    VerdictKind::NotResolved
                },
                if evidence { "evidence" } else { "" },
            )
        })
    }

    fn finding_strategy() -> impl Strategy<Value = Finding> {
        (10u32..16, 0usize..4).prop_map(|(n, s)| {
            defect(
                n,
                [
                    ReviewSeverity::Info,
                    ReviewSeverity::Low,
                    ReviewSeverity::Medium,
                    ReviewSeverity::High,
                ][s],
            )
        })
    }

    proptest! {
        /// Success is unreachable with an unresolved blocking finding: whatever the inputs, a pass implies every
        /// unresolved id has a verdict that says RESOLVED with evidence, and nothing contradicts it.
        #[test]
        fn pass_implies_every_blocking_finding_resolved(
            unresolved in proptest::collection::btree_set(0u32..6, 0..6),
            verdicts in proptest::collection::vec(verdict_strategy(), 0..12),
            fix in proptest::collection::vec(finding_strategy(), 0..4),
            obs in proptest::collection::vec(finding_strategy(), 0..4),
        ) {
            let set: BTreeSet<FindingId> = unresolved.iter().map(|n| id(*n)).collect();
            let outcome = evaluate_verification(VerificationInput {
                unresolved: &set,
                verdicts: &verdicts,
                fix_diff_findings: &fix,
                observations: &obs,
            });
            if outcome == ReviewOutcome::Pass {
                for finding_id in &set {
                    let about: Vec<&Verdict> =
                        verdicts.iter().filter(|v| &v.finding_id == finding_id).collect();
                    prop_assert!(!about.is_empty());
                    prop_assert!(about.iter().all(|v| {
                        v.kind == VerdictKind::Resolved && !v.evidence.trim().is_empty()
                    }));
                }
                prop_assert!(fix.iter().all(|f| !f.is_blocking()));
                prop_assert!(obs.iter().all(|f| !f.blocks_as_observation()));
            }
        }
    }
}
