//! Value objects that are not identifiers.

use serde::{Deserialize, Serialize};

use super::TicketId;

/// Risk class of a ticket (the `risk:*` label).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "full", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum RiskClass {
    /// Contained change with a narrow blast radius.
    Low,
    /// Default class.
    Medium,
    /// Touches security, persistence integrity, process control or other hard-to-reverse behavior.
    High,
}

/// Severity of a review finding. Ordered from least to most severe.
///
/// Whether a finding blocks is decided by the review policy (`REVIEW_PROTOCOL.md`), not by the severity alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "full", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ReviewSeverity {
    /// Observation with no required action.
    Info,
    /// Minor issue; normally non-blocking.
    Low,
    /// Medium correctness or specification finding; blocks under the Engineering policy.
    Medium,
    /// High-severity correctness, security or specification finding; always blocks.
    High,
}

impl ReviewSeverity {
    /// True for `Medium` and above, the severities the Engineering policy can block on.
    pub fn is_blocking_candidate(self) -> bool {
        self >= Self::Medium
    }
}

/// Outcome class of one validation gate. The wording matches the completion-report classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "full", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum GateStatus {
    /// The gate ran and passed.
    Pass,
    /// The gate ran and failed.
    Fail,
    /// The gate could not run in this environment; it must not be reported as a pass.
    Blocked,
    /// The gate does not apply to this change.
    NotApplicable,
}

/// The recorded result of one gate (`AGENTS.md`: command, exit code, duration, summary).
///
/// A `Pass` with a non-zero exit code is a contradiction: it is rejected by [`GateResult::new`] and when deserializing
/// (so a journaled record cannot claim it), and [`GateResult::is_pass`] never treats such a value as a pass even if the
/// public fields were assigned directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "full", derive(ts_rs::TS))]
pub struct GateResult {
    /// The gate or command that ran.
    pub gate: String,
    /// Outcome class.
    pub status: GateStatus,
    /// Process exit code when the gate ran a process.
    pub exit_code: Option<i32>,
    /// Wall-clock duration in milliseconds, when known.
    #[cfg_attr(feature = "full", ts(type = "number | null"))]
    pub duration_ms: Option<u64>,
    /// Short human-readable summary (never raw secrets).
    pub summary: String,
}

/// Why a [`GateResult`] was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateResultError {
    /// The gate name is empty.
    EmptyGate,
    /// `status` is `Pass` but the exit code is not 0.
    PassWithFailingExitCode(i32),
}

impl std::fmt::Display for GateResultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyGate => f.write_str("a gate result needs a gate name"),
            Self::PassWithFailingExitCode(code) => {
                write!(f, "a gate recorded as pass has exit code {code}")
            }
        }
    }
}

impl std::error::Error for GateResultError {}

impl GateResult {
    /// Builds a result, rejecting contradictory records.
    pub fn new(
        gate: impl Into<String>,
        status: GateStatus,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
        summary: impl Into<String>,
    ) -> Result<Self, GateResultError> {
        let result = Self {
            gate: gate.into(),
            status,
            exit_code,
            duration_ms,
            summary: summary.into(),
        };
        result.validate()?;
        Ok(result)
    }

    fn validate(&self) -> Result<(), GateResultError> {
        if self.gate.trim().is_empty() {
            return Err(GateResultError::EmptyGate);
        }
        match (self.status, self.exit_code) {
            (GateStatus::Pass, Some(code)) if code != 0 => {
                Err(GateResultError::PassWithFailingExitCode(code))
            }
            _ => Ok(()),
        }
    }

    /// True only for a gate that ran and passed. `Blocked` and `NotApplicable` are never a pass, and a recorded exit
    /// code other than 0 contradicts a pass, so such a record is not one.
    pub fn is_pass(&self) -> bool {
        self.status == GateStatus::Pass && matches!(self.exit_code, None | Some(0))
    }
}

impl<'de> Deserialize<'de> for GateResult {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            gate: String,
            status: GateStatus,
            exit_code: Option<i32>,
            duration_ms: Option<u64>,
            summary: String,
        }
        let raw = Raw::deserialize(deserializer)?;
        Self::new(raw.gate, raw.status, raw.exit_code, raw.duration_ms, raw.summary)
            .map_err(serde::de::Error::custom)
    }
}

/// Where a dependency edge came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "full", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum EdgeOrigin {
    /// Stated explicitly by the ticket or tracker (a hard blocker).
    Authored,
    /// Proposed by the dependency analyst and validated by Vela.
    Analyst,
}

/// A "blocker must finish before blocked" edge between two tickets.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "full", derive(ts_rs::TS))]
pub struct DependencyEdge {
    /// The ticket that must be `DONE` first.
    pub blocker: TicketId,
    /// The ticket that waits.
    pub blocked: TicketId,
    /// Where the edge came from.
    pub origin: EdgeOrigin,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_is_ordered_and_medium_is_the_blocking_threshold() {
        assert!(ReviewSeverity::High > ReviewSeverity::Medium);
        assert!(ReviewSeverity::Medium > ReviewSeverity::Low);
        assert!(!ReviewSeverity::Low.is_blocking_candidate());
        assert!(ReviewSeverity::Medium.is_blocking_candidate());
        assert!(ReviewSeverity::High.is_blocking_candidate());
    }

    #[test]
    fn only_a_gate_that_ran_and_passed_is_a_pass() {
        let gate = |status| GateResult {
            gate: "cargo test".into(),
            status,
            exit_code: None,
            duration_ms: None,
            summary: String::new(),
        };
        assert!(gate(GateStatus::Pass).is_pass());
        assert!(!gate(GateStatus::Fail).is_pass());
        assert!(!gate(GateStatus::Blocked).is_pass());
        assert!(!gate(GateStatus::NotApplicable).is_pass());
        let mut contradictory = gate(GateStatus::Pass);
        contradictory.exit_code = Some(1);
        assert!(!contradictory.is_pass());
        contradictory.exit_code = Some(0);
        assert!(contradictory.is_pass());
    }

    #[test]
    fn contradictory_gate_records_are_rejected_at_construction() {
        assert!(GateResult::new("cargo test", GateStatus::Pass, Some(0), None, "ok").is_ok());
        assert!(GateResult::new("cargo test", GateStatus::Pass, None, None, "ok").is_ok());
        assert_eq!(
            GateResult::new("cargo test", GateStatus::Pass, Some(1), None, "ok"),
            Err(GateResultError::PassWithFailingExitCode(1))
        );
        assert!(GateResult::new("cargo test", GateStatus::Fail, Some(1), None, "no").is_ok());
        assert_eq!(
            GateResult::new(" ", GateStatus::Fail, None, None, ""),
            Err(GateResultError::EmptyGate)
        );
    }

    #[cfg(feature = "full")]
    #[test]
    fn a_pass_with_a_failing_exit_code_cannot_be_deserialized() {
        let forged = r#"{"gate":"cargo test","status":"pass","exit_code":1,"duration_ms":null,"summary":""}"#;
        assert!(serde_json::from_str::<GateResult>(forged).is_err());
        let honest = r#"{"gate":"cargo test","status":"fail","exit_code":1,"duration_ms":null,"summary":""}"#;
        assert!(serde_json::from_str::<GateResult>(honest).is_ok());
    }

    #[cfg(feature = "full")]
    #[test]
    fn value_objects_round_trip_through_json() {
        let edge = DependencyEdge {
            blocker: TicketId::new("F01").unwrap(),
            blocked: TicketId::new("F02").unwrap(),
            origin: EdgeOrigin::Authored,
        };
        let json = serde_json::to_string(&edge).unwrap();
        assert_eq!(
            json,
            r#"{"blocker":"F01","blocked":"F02","origin":"authored"}"#
        );
        assert_eq!(serde_json::from_str::<DependencyEdge>(&json).unwrap(), edge);

        let gate = GateResult {
            gate: "cargo clippy".into(),
            status: GateStatus::Blocked,
            exit_code: Some(101),
            duration_ms: Some(1500),
            summary: "os error 4551".into(),
        };
        let json = serde_json::to_string(&gate).unwrap();
        assert_eq!(serde_json::from_str::<GateResult>(&json).unwrap(), gate);
        assert_eq!(
            serde_json::to_string(&ReviewSeverity::High).unwrap(),
            "\"high\""
        );
        assert_eq!(serde_json::to_string(&RiskClass::Low).unwrap(), "\"low\"");
    }
}
