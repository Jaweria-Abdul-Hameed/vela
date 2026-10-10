//! Value objects that are not identifiers.

use serde::{Deserialize, Serialize};

use super::{IdError, TicketId};

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
/// Every value of this type is internally consistent, so everything that can be serialized into the journal can be read
/// back during replay. The fields are private; the only ways to obtain a value are [`GateResult::new`] and
/// deserialization, and both apply the same rules:
///
/// - the gate name is not empty;
/// - `Pass` has exit code 0 or none (a pass with a failing exit code is a contradiction);
/// - `Fail` has a non-zero exit code or none (a failure with exit code 0 is a contradiction);
/// - `NotApplicable` ran nothing, so it has no exit code and no duration;
/// - `Blocked` may carry whatever exit code the blocked tool reported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "full", derive(ts_rs::TS))]
pub struct GateResult {
    /// The gate or command that ran.
    gate: String,
    /// Outcome class.
    status: GateStatus,
    /// Process exit code when the gate ran a process.
    exit_code: Option<i32>,
    /// Wall-clock duration in milliseconds, when known.
    #[cfg_attr(feature = "full", ts(type = "number | null"))]
    duration_ms: Option<u64>,
    /// Short human-readable summary (never raw secrets).
    summary: String,
}

/// Why a [`GateResult`] was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateResultError {
    /// The gate name is empty.
    EmptyGate,
    /// `status` is `Pass` but the exit code is not 0.
    PassWithFailingExitCode(i32),
    /// `status` is `Fail` but the exit code is 0.
    FailWithZeroExitCode,
    /// `status` is `NotApplicable` but an exit code or duration was recorded.
    NotApplicableWithRun,
}

impl std::fmt::Display for GateResultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyGate => f.write_str("a gate result needs a gate name"),
            Self::PassWithFailingExitCode(code) => {
                write!(f, "a gate recorded as pass has exit code {code}")
            }
            Self::FailWithZeroExitCode => f.write_str("a gate recorded as fail has exit code 0"),
            Self::NotApplicableWithRun => {
                f.write_str("a gate recorded as not applicable has an exit code or duration")
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
        let gate = gate.into();
        if gate.trim().is_empty() {
            return Err(GateResultError::EmptyGate);
        }
        match (status, exit_code) {
            (GateStatus::Pass, Some(code)) if code != 0 => {
                return Err(GateResultError::PassWithFailingExitCode(code));
            }
            (GateStatus::Fail, Some(0)) => return Err(GateResultError::FailWithZeroExitCode),
            (GateStatus::NotApplicable, Some(_)) => {
                return Err(GateResultError::NotApplicableWithRun);
            }
            _ => {}
        }
        if status == GateStatus::NotApplicable && duration_ms.is_some() {
            return Err(GateResultError::NotApplicableWithRun);
        }
        Ok(Self {
            gate,
            status,
            exit_code,
            duration_ms,
            summary: summary.into(),
        })
    }

    /// The gate or command that ran.
    pub fn gate(&self) -> &str {
        &self.gate
    }

    /// The outcome class.
    pub fn status(&self) -> GateStatus {
        self.status
    }

    /// The exit code, when the gate ran a process.
    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    /// The wall-clock duration in milliseconds, when known.
    pub fn duration_ms(&self) -> Option<u64> {
        self.duration_ms
    }

    /// The summary.
    pub fn summary(&self) -> &str {
        &self.summary
    }

    /// True only for a gate that ran and passed. `Blocked` and `NotApplicable` are never a pass.
    pub fn is_pass(&self) -> bool {
        self.status == GateStatus::Pass
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
        Self::new(
            raw.gate,
            raw.status,
            raw.exit_code,
            raw.duration_ms,
            raw.summary,
        )
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

/// A "blocker must finish before blocked" edge between two tickets. A ticket cannot block itself (that would be a
/// cycle of length one), so such an edge can neither be constructed nor read back.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "full", derive(ts_rs::TS))]
pub struct DependencyEdge {
    /// The ticket that must be `DONE` first.
    blocker: TicketId,
    /// The ticket that waits.
    blocked: TicketId,
    /// Where the edge came from.
    origin: EdgeOrigin,
}

impl DependencyEdge {
    /// Builds an edge, rejecting a self-edge.
    pub fn new(blocker: TicketId, blocked: TicketId, origin: EdgeOrigin) -> Result<Self, IdError> {
        if blocker == blocked {
            return Err(IdError::Malformed("a ticket cannot block itself"));
        }
        Ok(Self {
            blocker,
            blocked,
            origin,
        })
    }

    /// The ticket that must be `DONE` first.
    pub fn blocker(&self) -> &TicketId {
        &self.blocker
    }

    /// The ticket that waits.
    pub fn blocked(&self) -> &TicketId {
        &self.blocked
    }

    /// Where the edge came from.
    pub fn origin(&self) -> EdgeOrigin {
        self.origin
    }
}

impl<'de> Deserialize<'de> for DependencyEdge {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            blocker: TicketId,
            blocked: TicketId,
            origin: EdgeOrigin,
        }
        let raw = Raw::deserialize(deserializer)?;
        Self::new(raw.blocker, raw.blocked, raw.origin).map_err(serde::de::Error::custom)
    }
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

    fn gate(status: GateStatus, exit_code: Option<i32>) -> Result<GateResult, GateResultError> {
        GateResult::new("cargo test", status, exit_code, None, "")
    }

    #[test]
    fn only_a_gate_that_ran_and_passed_is_a_pass() {
        assert!(gate(GateStatus::Pass, None).unwrap().is_pass());
        assert!(gate(GateStatus::Pass, Some(0)).unwrap().is_pass());
        assert!(!gate(GateStatus::Fail, Some(1)).unwrap().is_pass());
        assert!(!gate(GateStatus::Blocked, Some(101)).unwrap().is_pass());
        assert!(!gate(GateStatus::NotApplicable, None).unwrap().is_pass());
    }

    #[test]
    fn contradictory_gate_records_cannot_be_constructed() {
        assert_eq!(
            gate(GateStatus::Pass, Some(1)),
            Err(GateResultError::PassWithFailingExitCode(1))
        );
        assert_eq!(
            gate(GateStatus::Pass, Some(-1)),
            Err(GateResultError::PassWithFailingExitCode(-1))
        );
        assert_eq!(
            gate(GateStatus::Fail, Some(0)),
            Err(GateResultError::FailWithZeroExitCode)
        );
        assert_eq!(
            gate(GateStatus::NotApplicable, Some(0)),
            Err(GateResultError::NotApplicableWithRun)
        );
        assert_eq!(
            GateResult::new("x", GateStatus::NotApplicable, None, Some(5), ""),
            Err(GateResultError::NotApplicableWithRun)
        );
        assert_eq!(
            GateResult::new(" ", GateStatus::Fail, None, None, ""),
            Err(GateResultError::EmptyGate)
        );
        // Consistent records, including a failure with no exit code (killed) and a blocked tool's own code.
        assert!(gate(GateStatus::Fail, None).is_ok());
        assert!(gate(GateStatus::Fail, Some(101)).is_ok());
        assert!(gate(GateStatus::Blocked, None).is_ok());
    }

    #[test]
    fn accessors_return_what_was_recorded() {
        let result = GateResult::new(
            "cargo clippy",
            GateStatus::Fail,
            Some(2),
            Some(1500),
            "lint",
        )
        .unwrap();
        assert_eq!(result.gate(), "cargo clippy");
        assert_eq!(result.status(), GateStatus::Fail);
        assert_eq!(result.exit_code(), Some(2));
        assert_eq!(result.duration_ms(), Some(1500));
        assert_eq!(result.summary(), "lint");
    }

    /// Everything that can be constructed serializes to a value that deserializes to the same record, so a journaled
    /// gate result can always be replayed.
    #[cfg(feature = "full")]
    #[test]
    fn every_constructible_gate_result_round_trips_and_every_invalid_combination_is_rejected_on_read()
     {
        let statuses = [
            GateStatus::Pass,
            GateStatus::Fail,
            GateStatus::Blocked,
            GateStatus::NotApplicable,
        ];
        for status in statuses {
            for exit_code in [None, Some(0), Some(1), Some(-1), Some(101)] {
                for duration in [None, Some(0), Some(9)] {
                    let built = GateResult::new("g", status, exit_code, duration, "s");
                    let json = format!(
                        r#"{{"gate":"g","status":{},"exit_code":{},"duration_ms":{},"summary":"s"}}"#,
                        serde_json::to_string(&status).unwrap(),
                        serde_json::to_string(&exit_code).unwrap(),
                        serde_json::to_string(&duration).unwrap(),
                    );
                    let read = serde_json::from_str::<GateResult>(&json);
                    match built {
                        Ok(value) => {
                            assert_eq!(
                                serde_json::from_str::<GateResult>(
                                    &serde_json::to_string(&value).unwrap()
                                )
                                .unwrap(),
                                value
                            );
                            assert_eq!(read.unwrap(), value);
                        }
                        Err(_) => assert!(read.is_err(), "{json} must be rejected on read"),
                    }
                }
            }
        }
    }

    #[cfg(feature = "full")]
    #[test]
    fn forged_gate_json_is_rejected_on_read() {
        for forged in [
            r#"{"gate":"g","status":"pass","exit_code":1,"duration_ms":null,"summary":""}"#,
            r#"{"gate":"g","status":"fail","exit_code":0,"duration_ms":null,"summary":""}"#,
            r#"{"gate":"","status":"fail","exit_code":1,"duration_ms":null,"summary":""}"#,
            r#"{"gate":"g","status":"not_applicable","exit_code":0,"duration_ms":null,"summary":""}"#,
        ] {
            assert!(
                serde_json::from_str::<GateResult>(forged).is_err(),
                "{forged}"
            );
        }
    }

    #[test]
    fn a_ticket_cannot_block_itself() {
        let f02 = || TicketId::new("F02").unwrap();
        assert!(DependencyEdge::new(TicketId::new("F01").unwrap(), f02(), EdgeOrigin::Analyst).is_ok());
        assert_eq!(
            DependencyEdge::new(f02(), f02(), EdgeOrigin::Authored),
            Err(IdError::Malformed("a ticket cannot block itself"))
        );
    }

    #[cfg(feature = "full")]
    #[test]
    fn a_self_edge_is_rejected_on_read() {
        let forged = r#"{"blocker":"F02","blocked":"F02","origin":"authored"}"#;
        assert!(serde_json::from_str::<DependencyEdge>(forged).is_err());
    }

    #[cfg(feature = "full")]
    #[test]
    fn value_objects_round_trip_through_json() {
        let edge = DependencyEdge::new(
            TicketId::new("F01").unwrap(),
            TicketId::new("F02").unwrap(),
            EdgeOrigin::Authored,
        )
        .unwrap();
        let json = serde_json::to_string(&edge).unwrap();
        assert_eq!(
            json,
            r#"{"blocker":"F01","blocked":"F02","origin":"authored"}"#
        );
        assert_eq!(serde_json::from_str::<DependencyEdge>(&json).unwrap(), edge);

        let gate = GateResult::new(
            "cargo clippy",
            GateStatus::Blocked,
            Some(101),
            Some(1500),
            "os error 4551",
        )
        .unwrap();
        let json = serde_json::to_string(&gate).unwrap();
        assert_eq!(serde_json::from_str::<GateResult>(&json).unwrap(), gate);
        assert_eq!(
            serde_json::to_string(&ReviewSeverity::High).unwrap(),
            "\"high\""
        );
        assert_eq!(serde_json::to_string(&RiskClass::Low).unwrap(), "\"low\"");
    }
}
