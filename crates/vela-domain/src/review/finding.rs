//! Findings, the Engineering exit policy and disposition tracking (`REVIEW_PROTOCOL.md`, "Finding schema" and
//! "Default Engineering policy").

use std::fmt;

use serde::{Deserialize, Serialize};

use super::fingerprint::{FindingId, fingerprint};
use super::verification::VerdictKind;
use crate::ids::ReviewSeverity;
use crate::state::Cause;

/// The review axis a finding came from (`PROMPT_CONTRACTS.md`, reviewer output contract).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReviewAxis {
    /// Repository engineering standards.
    Standards,
    /// The originating spec or ticket.
    Spec,
}

/// What kind of finding this is. The policy blocks on the defect classes the Engineering policy lists and never on
/// the preference classes it names as normally non-blocking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewCategory {
    /// A security vulnerability introduced by the diff.
    Security,
    /// Incorrect behavior.
    Correctness,
    /// A missing acceptance criterion or a deviation from the specification.
    Spec,
    /// Broken tests, build or typecheck.
    BuildOrTest,
    /// An architecture boundary violation.
    Architecture,
    /// Data loss, a race or an error-path bug.
    DataIntegrity,
    /// A change to an execution-defining file (ADR-013) the ticket scope does not explain.
    ExecutionDefiningFile,
    /// Subjective naming.
    Naming,
    /// Style already accepted by the formatter or linter.
    Style,
    /// An optional abstraction.
    Abstraction,
    /// Speculative future-proofing.
    SpeculativeFuture,
    /// A micro-optimization without measured need.
    MicroOptimization,
    /// Anything the reviewer could not place in a class above. It blocks only at high severity (DR-D06-1).
    Other,
}

/// How the policy treats a category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PolicyClass {
    /// Blocks at medium severity or higher.
    Defect,
    /// Never blocks (normally non-blocking per the protocol).
    Preference,
    /// Blocks only at high severity.
    Unclassified,
}

impl ReviewCategory {
    /// The canonical name used in the fingerprint.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Security => "security",
            Self::Correctness => "correctness",
            Self::Spec => "spec",
            Self::BuildOrTest => "build_or_test",
            Self::Architecture => "architecture",
            Self::DataIntegrity => "data_integrity",
            Self::ExecutionDefiningFile => "execution_defining_file",
            Self::Naming => "naming",
            Self::Style => "style",
            Self::Abstraction => "abstraction",
            Self::SpeculativeFuture => "speculative_future",
            Self::MicroOptimization => "micro_optimization",
            Self::Other => "other",
        }
    }

    const fn policy_class(self) -> PolicyClass {
        match self {
            Self::Security
            | Self::Correctness
            | Self::Spec
            | Self::BuildOrTest
            | Self::Architecture
            | Self::DataIntegrity
            | Self::ExecutionDefiningFile => PolicyClass::Defect,
            Self::Naming
            | Self::Style
            | Self::Abstraction
            | Self::SpeculativeFuture
            | Self::MicroOptimization => PolicyClass::Preference,
            Self::Other => PolicyClass::Unclassified,
        }
    }

    /// The Engineering policy: does a finding of this category and severity block? The reviewer's severity is used
    /// as reported and is never lowered here.
    pub fn blocks_at(self, severity: ReviewSeverity) -> bool {
        match self.policy_class() {
            PolicyClass::Defect => severity.is_blocking_candidate(),
            PolicyClass::Preference => false,
            PolicyClass::Unclassified => severity == ReviewSeverity::High,
        }
    }

    /// Whether a defect noticed outside the fix diff blocks wherever it is found: high severity only, and never a
    /// preference-class finding.
    pub fn blocks_as_observation(self, severity: ReviewSeverity) -> bool {
        severity == ReviewSeverity::High && self.policy_class() != PolicyClass::Preference
    }
}

/// Where a finding stands. `Resolved` is reachable only through a verifier verdict: the worker's own statement is
/// not evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Disposition {
    /// Reported, not yet addressed.
    Open,
    /// The worker says it fixed it. Not evidence; it still needs the verifier.
    FixClaimed,
    /// The verifier reported `RESOLVED` with evidence.
    Resolved,
    /// The verifier reported `NOT_RESOLVED` (or gave no verdict).
    NotResolved,
    /// Non-blocking, recorded as a follow-up.
    FollowUp,
    /// Non-blocking, risk accepted by a recorded user decision.
    AcceptedRisk,
}

impl Disposition {
    const fn name(self) -> &'static str {
        match self {
            Self::Open => "OPEN",
            Self::FixClaimed => "FIX_CLAIMED",
            Self::Resolved => "RESOLVED",
            Self::NotResolved => "NOT_RESOLVED",
            Self::FollowUp => "FOLLOW_UP",
            Self::AcceptedRisk => "ACCEPTED_RISK",
        }
    }
}

/// A rejected disposition change. Rejection never changes the finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispositionError {
    /// A blocking finding is never deferred or accepted; only a verified fix closes it.
    BlockingFindingCannotBeDispositioned {
        /// The target disposition that was refused.
        to: Disposition,
    },
    /// Accepting a risk needs a recorded user decision.
    RiskAcceptanceNeedsUserDecision,
    /// The change is not defined from the current disposition.
    Illegal {
        /// Current disposition.
        from: Disposition,
        /// Requested disposition.
        to: Disposition,
    },
}

impl fmt::Display for DispositionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BlockingFindingCannotBeDispositioned { to } => write!(
                f,
                "a blocking finding cannot become {}; only a verified fix closes it",
                to.name()
            ),
            Self::RiskAcceptanceNeedsUserDecision => {
                f.write_str("accepting a risk needs a recorded user decision")
            }
            Self::Illegal { from, to } => {
                write!(
                    f,
                    "illegal disposition change {} to {}",
                    from.name(),
                    to.name()
                )
            }
        }
    }
}

impl std::error::Error for DispositionError {}

/// One review finding. Whether it blocks is decided by [`Finding::is_blocking`] from the policy, never stored from
/// the reviewer's prose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    id: FindingId,
    axis: ReviewAxis,
    severity: ReviewSeverity,
    category: ReviewCategory,
    location: String,
    requirement: String,
    evidence: String,
    suggested_fix: String,
    disposition: Disposition,
}

impl Finding {
    /// A new, open finding. The id is the fingerprint of category, location and requirement.
    pub fn new(
        axis: ReviewAxis,
        severity: ReviewSeverity,
        category: ReviewCategory,
        location: impl Into<String>,
        requirement: impl Into<String>,
        evidence: impl Into<String>,
        suggested_fix: impl Into<String>,
    ) -> Self {
        let location = location.into();
        let requirement = requirement.into();
        Self {
            id: fingerprint(category.as_str(), &location, &requirement),
            axis,
            severity,
            category,
            location,
            requirement,
            evidence: evidence.into(),
            suggested_fix: suggested_fix.into(),
            disposition: Disposition::Open,
        }
    }

    /// The stable id.
    pub fn id(&self) -> &FindingId {
        &self.id
    }

    /// The axis.
    pub fn axis(&self) -> ReviewAxis {
        self.axis
    }

    /// The severity as reported by the reviewer.
    pub fn severity(&self) -> ReviewSeverity {
        self.severity
    }

    /// The category.
    pub fn category(&self) -> ReviewCategory {
        self.category
    }

    /// The affected location.
    pub fn location(&self) -> &str {
        &self.location
    }

    /// The requirement violated.
    pub fn requirement(&self) -> &str {
        &self.requirement
    }

    /// The evidence.
    pub fn evidence(&self) -> &str {
        &self.evidence
    }

    /// The actionable fix.
    pub fn suggested_fix(&self) -> &str {
        &self.suggested_fix
    }

    /// The current disposition.
    pub fn disposition(&self) -> Disposition {
        self.disposition
    }

    /// The Engineering exit policy: does this finding block?
    pub fn is_blocking(&self) -> bool {
        self.category.blocks_at(self.severity)
    }

    /// Whether this finding, reported as an observation outside the fix diff, blocks.
    pub fn blocks_as_observation(&self) -> bool {
        self.category.blocks_as_observation(self.severity)
    }

    /// The worker says it fixed the finding. Recorded, never treated as resolution.
    pub fn claim_fix(&mut self) -> Result<(), DispositionError> {
        match self.disposition {
            Disposition::Open | Disposition::FixClaimed | Disposition::NotResolved => {
                self.disposition = Disposition::FixClaimed;
                Ok(())
            }
            from => Err(DispositionError::Illegal {
                from,
                to: Disposition::FixClaimed,
            }),
        }
    }

    /// Apply the verifier's verdict. This is the only way to `Resolved`.
    pub fn apply_verdict(&mut self, verdict: VerdictKind) -> Result<(), DispositionError> {
        let to = match verdict {
            VerdictKind::Resolved => Disposition::Resolved,
            VerdictKind::NotResolved => Disposition::NotResolved,
        };
        match self.disposition {
            Disposition::Open
            | Disposition::FixClaimed
            | Disposition::NotResolved
            | Disposition::Resolved => {
                self.disposition = to;
                Ok(())
            }
            from => Err(DispositionError::Illegal { from, to }),
        }
    }

    /// Record a non-blocking finding as a follow-up.
    pub fn defer_as_follow_up(&mut self) -> Result<(), DispositionError> {
        if self.is_blocking() {
            return Err(DispositionError::BlockingFindingCannotBeDispositioned {
                to: Disposition::FollowUp,
            });
        }
        match self.disposition {
            Disposition::Open | Disposition::FollowUp => {
                self.disposition = Disposition::FollowUp;
                Ok(())
            }
            from => Err(DispositionError::Illegal {
                from,
                to: Disposition::FollowUp,
            }),
        }
    }

    /// Accept the risk of a non-blocking finding. Needs a recorded user decision; a blocking finding is never
    /// accepted by any cause.
    pub fn accept_risk(&mut self, cause: Cause) -> Result<(), DispositionError> {
        if self.is_blocking() {
            return Err(DispositionError::BlockingFindingCannotBeDispositioned {
                to: Disposition::AcceptedRisk,
            });
        }
        if cause != Cause::UserDecision {
            return Err(DispositionError::RiskAcceptanceNeedsUserDecision);
        }
        match self.disposition {
            Disposition::Open | Disposition::FollowUp | Disposition::AcceptedRisk => {
                self.disposition = Disposition::AcceptedRisk;
                Ok(())
            }
            from => Err(DispositionError::Illegal {
                from,
                to: Disposition::AcceptedRisk,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_CATEGORIES: [ReviewCategory; 13] = [
        ReviewCategory::Security,
        ReviewCategory::Correctness,
        ReviewCategory::Spec,
        ReviewCategory::BuildOrTest,
        ReviewCategory::Architecture,
        ReviewCategory::DataIntegrity,
        ReviewCategory::ExecutionDefiningFile,
        ReviewCategory::Naming,
        ReviewCategory::Style,
        ReviewCategory::Abstraction,
        ReviewCategory::SpeculativeFuture,
        ReviewCategory::MicroOptimization,
        ReviewCategory::Other,
    ];
    const ALL_SEVERITIES: [ReviewSeverity; 4] = [
        ReviewSeverity::Info,
        ReviewSeverity::Low,
        ReviewSeverity::Medium,
        ReviewSeverity::High,
    ];

    fn finding(category: ReviewCategory, severity: ReviewSeverity) -> Finding {
        Finding::new(
            ReviewAxis::Spec,
            severity,
            category,
            "src/a.rs:1",
            "FR-013",
            "evidence",
            "fix",
        )
    }

    /// Independent oracle of the REVIEW_PROTOCOL.md "Default Engineering policy" table.
    fn oracle(category: ReviewCategory, severity: ReviewSeverity) -> bool {
        use ReviewCategory as C;
        use ReviewSeverity as S;
        match category {
            C::Security
            | C::Correctness
            | C::Spec
            | C::BuildOrTest
            | C::Architecture
            | C::DataIntegrity
            | C::ExecutionDefiningFile => matches!(severity, S::Medium | S::High),
            C::Naming | C::Style | C::Abstraction | C::SpeculativeFuture | C::MicroOptimization => {
                false
            }
            C::Other => matches!(severity, S::High),
        }
    }

    #[test]
    fn blocking_matches_the_engineering_policy_for_every_category_and_severity() {
        for category in ALL_CATEGORIES {
            for severity in ALL_SEVERITIES {
                assert_eq!(
                    finding(category, severity).is_blocking(),
                    oracle(category, severity),
                    "{category:?} at {severity:?}"
                );
            }
        }
    }

    #[test]
    fn high_severity_correctness_security_and_spec_always_block() {
        for category in [
            ReviewCategory::Correctness,
            ReviewCategory::Security,
            ReviewCategory::Spec,
        ] {
            assert!(finding(category, ReviewSeverity::High).is_blocking());
            assert!(finding(category, ReviewSeverity::Medium).is_blocking());
            assert!(!finding(category, ReviewSeverity::Low).is_blocking());
        }
    }

    #[test]
    fn observations_block_only_at_high_severity_and_never_for_preferences() {
        for category in ALL_CATEGORIES {
            for severity in ALL_SEVERITIES {
                let preference = matches!(
                    category,
                    ReviewCategory::Naming
                        | ReviewCategory::Style
                        | ReviewCategory::Abstraction
                        | ReviewCategory::SpeculativeFuture
                        | ReviewCategory::MicroOptimization
                );
                let expected = severity == ReviewSeverity::High && !preference;
                assert_eq!(
                    finding(category, severity).blocks_as_observation(),
                    expected,
                    "{category:?} at {severity:?}"
                );
            }
        }
    }

    #[test]
    fn id_ignores_severity_evidence_and_fix_wording() {
        let a = Finding::new(
            ReviewAxis::Spec,
            ReviewSeverity::High,
            ReviewCategory::Spec,
            "src/a.rs:1",
            "FR-013",
            "first wording",
            "fix one",
        );
        let b = Finding::new(
            ReviewAxis::Standards,
            ReviewSeverity::Medium,
            ReviewCategory::Spec,
            "src/a.rs:1",
            "FR-013",
            "second wording",
            "fix two",
        );
        assert_eq!(a.id(), b.id());
    }

    #[test]
    fn a_fix_claim_is_not_resolution() {
        let mut f = finding(ReviewCategory::Correctness, ReviewSeverity::High);
        f.claim_fix().unwrap();
        assert_eq!(f.disposition(), Disposition::FixClaimed);
        f.apply_verdict(VerdictKind::Resolved).unwrap();
        assert_eq!(f.disposition(), Disposition::Resolved);
        assert!(f.claim_fix().is_err());
    }

    #[test]
    fn a_blocking_finding_is_never_deferred_or_accepted() {
        let mut f = finding(ReviewCategory::Correctness, ReviewSeverity::Medium);
        assert_eq!(
            f.defer_as_follow_up(),
            Err(DispositionError::BlockingFindingCannotBeDispositioned {
                to: Disposition::FollowUp
            })
        );
        for cause in [Cause::System, Cause::UserDecision] {
            assert_eq!(
                f.accept_risk(cause),
                Err(DispositionError::BlockingFindingCannotBeDispositioned {
                    to: Disposition::AcceptedRisk
                })
            );
        }
        assert_eq!(f.disposition(), Disposition::Open);
    }

    #[test]
    fn a_non_blocking_finding_can_be_deferred_and_a_risk_needs_a_user_decision() {
        let mut f = finding(ReviewCategory::Naming, ReviewSeverity::Low);
        f.defer_as_follow_up().unwrap();
        assert_eq!(f.disposition(), Disposition::FollowUp);
        assert_eq!(
            f.accept_risk(Cause::System),
            Err(DispositionError::RiskAcceptanceNeedsUserDecision)
        );
        f.accept_risk(Cause::UserDecision).unwrap();
        assert_eq!(f.disposition(), Disposition::AcceptedRisk);
        assert!(f.apply_verdict(VerdictKind::Resolved).is_err());
    }

    #[test]
    fn not_resolved_can_be_fixed_and_verified_again() {
        let mut f = finding(ReviewCategory::Spec, ReviewSeverity::High);
        f.apply_verdict(VerdictKind::NotResolved).unwrap();
        assert_eq!(f.disposition(), Disposition::NotResolved);
        f.claim_fix().unwrap();
        f.apply_verdict(VerdictKind::Resolved).unwrap();
        assert_eq!(f.disposition(), Disposition::Resolved);
    }
}
