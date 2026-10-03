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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl GateResult {
    /// True only for a gate that ran and passed. `Blocked` and `NotApplicable` are never a pass, and a recorded exit
    /// code other than 0 contradicts a pass, so such a record is not one.
    pub fn is_pass(&self) -> bool {
        self.status == GateStatus::Pass && matches!(self.exit_code, None | Some(0))
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
