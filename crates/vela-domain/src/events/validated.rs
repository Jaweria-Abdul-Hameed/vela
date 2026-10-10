//! Validated value types used inside event payloads and the envelope.
//!
//! Each type can only be built through a checked constructor and is checked again when read, so a journal row that
//! was written can always be replayed (everything constructible serializes to something that deserializes to the same
//! value) and a forged or corrupt row is rejected instead of being replayed with a meaning it does not have.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::ids::{CommitSha, GateResult, GateStatus};

use super::EVENT_SCHEMA_VERSION;

/// Why a validated event value was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventValueError {
    /// A gate in a list of passed gates did not pass.
    GateDidNotPass {
        /// The offending gate.
        gate: String,
    },
    /// The gate recorded as the failing integration gate did not fail.
    GateDidNotFail {
        /// The offending gate.
        gate: String,
    },
    /// The commit that was merged is not the commit that was reviewed.
    MergedSourceIsNotTheReviewedHead,
    /// The schema version is zero or newer than this build understands.
    UnsupportedSchemaVersion(u32),
}

impl fmt::Display for EventValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GateDidNotPass { gate } => write!(f, "gate {gate:?} is listed as passed but did not pass"),
            Self::GateDidNotFail { gate } => write!(f, "gate {gate:?} is recorded as the failing gate but did not fail"),
            Self::MergedSourceIsNotTheReviewedHead => {
                f.write_str("the merged source commit differs from the reviewed head")
            }
            Self::UnsupportedSchemaVersion(version) => {
                write!(f, "unsupported event schema version {version} (this build reads 1..={EVENT_SCHEMA_VERSION})")
            }
        }
    }
}

impl std::error::Error for EventValueError {}

/// Gate results in which every gate passed (`WorkerCompleted`: a worker completes only with green gates).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "full", derive(ts_rs::TS))]
pub struct PassedGates(Vec<GateResult>);

impl PassedGates {
    /// Accepts the gates only if every one passed (an empty list is allowed: a ticket may have no gate).
    pub fn new(gates: Vec<GateResult>) -> Result<Self, EventValueError> {
        match gates.iter().find(|gate| !gate.is_pass()) {
            Some(gate) => Err(EventValueError::GateDidNotPass {
                gate: gate.gate().to_owned(),
            }),
            None => Ok(Self(gates)),
        }
    }

    /// The gates.
    pub fn as_slice(&self) -> &[GateResult] {
        &self.0
    }
}

impl<'de> Deserialize<'de> for PassedGates {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(Vec::<GateResult>::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// The gate that failed integration validation (`IntegrationGateFailed`): its status is `Fail`. A gate that could not
/// run (`Blocked`) is not a failed gate: it leaves integration health unknown instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "full", derive(ts_rs::TS))]
pub struct FailedGate(GateResult);

impl FailedGate {
    /// Accepts the result only if its status is `Fail`.
    pub fn new(result: GateResult) -> Result<Self, EventValueError> {
        if result.status() == GateStatus::Fail {
            Ok(Self(result))
        } else {
            Err(EventValueError::GateDidNotFail {
                gate: result.gate().to_owned(),
            })
        }
    }

    /// The failing gate result.
    pub fn result(&self) -> &GateResult {
        &self.0
    }
}

impl<'de> Deserialize<'de> for FailedGate {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(GateResult::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// What entered integration: the commit that passed review and the commit that was actually merged. The merge port
/// refuses to merge anything else, so the two are always equal, and the journal records both so that an audit (and
/// replay) can verify it. A row in which they differ is rejected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "full", derive(ts_rs::TS))]
pub struct MergeBinding {
    reviewed_head: CommitSha,
    merged_source: CommitSha,
}

impl MergeBinding {
    /// Accepts the pair only if the merged source is the reviewed head.
    pub fn new(reviewed_head: CommitSha, merged_source: CommitSha) -> Result<Self, EventValueError> {
        if reviewed_head == merged_source {
            Ok(Self {
                reviewed_head,
                merged_source,
            })
        } else {
            Err(EventValueError::MergedSourceIsNotTheReviewedHead)
        }
    }

    /// The `review_head_sha` of the review that passed.
    pub fn reviewed_head(&self) -> &CommitSha {
        &self.reviewed_head
    }

    /// The source commit that was merged.
    pub fn merged_source(&self) -> &CommitSha {
        &self.merged_source
    }
}

impl<'de> Deserialize<'de> for MergeBinding {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            reviewed_head: CommitSha,
            merged_source: CommitSha,
        }
        let raw = Raw::deserialize(deserializer)?;
        Self::new(raw.reviewed_head, raw.merged_source).map_err(serde::de::Error::custom)
    }
}

/// The schema version of an envelope: `1..=EVENT_SCHEMA_VERSION`. Zero and versions newer than this build are rejected,
/// so a payload is never interpreted with the shape of a different version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "full", derive(ts_rs::TS))]
pub struct SchemaVersion(u32);

impl SchemaVersion {
    /// Accepts a version this build understands.
    pub fn new(version: u32) -> Result<Self, EventValueError> {
        if (1..=EVENT_SCHEMA_VERSION).contains(&version) {
            Ok(Self(version))
        } else {
            Err(EventValueError::UnsupportedSchemaVersion(version))
        }
    }

    /// The version this build writes.
    pub fn current() -> Self {
        Self(EVENT_SCHEMA_VERSION)
    }

    /// The numeric version.
    pub fn get(self) -> u32 {
        self.0
    }
}

impl<'de> Deserialize<'de> for SchemaVersion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(u32::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate(status: GateStatus, exit_code: Option<i32>) -> GateResult {
        GateResult::new("cargo test", status, exit_code, None, "").unwrap()
    }

    fn sha(digit: char) -> CommitSha {
        CommitSha::new(digit.to_string().repeat(40)).unwrap()
    }

    #[test]
    fn passed_gates_accept_only_passing_gates() {
        assert!(PassedGates::new(Vec::new()).is_ok());
        let ok = PassedGates::new(vec![gate(GateStatus::Pass, Some(0)), gate(GateStatus::Pass, None)]).unwrap();
        assert_eq!(ok.as_slice().len(), 2);
        for bad in [
            gate(GateStatus::Fail, Some(1)),
            gate(GateStatus::Blocked, Some(101)),
            gate(GateStatus::NotApplicable, None),
        ] {
            assert!(
                PassedGates::new(vec![gate(GateStatus::Pass, Some(0)), bad]).is_err(),
                "one non-passing gate must reject the list"
            );
        }
    }

    #[test]
    fn a_failed_gate_must_have_failed() {
        assert!(FailedGate::new(gate(GateStatus::Fail, Some(1))).is_ok());
        assert!(FailedGate::new(gate(GateStatus::Fail, None)).is_ok());
        for bad in [
            gate(GateStatus::Pass, Some(0)),
            gate(GateStatus::Blocked, Some(101)),
            gate(GateStatus::NotApplicable, None),
        ] {
            assert!(FailedGate::new(bad).is_err());
        }
        assert_eq!(
            FailedGate::new(gate(GateStatus::Pass, None)).unwrap_err(),
            EventValueError::GateDidNotFail {
                gate: "cargo test".into()
            }
        );
    }

    #[test]
    fn a_merge_binding_requires_the_merged_source_to_be_the_reviewed_head() {
        let binding = MergeBinding::new(sha('a'), sha('a')).unwrap();
        assert_eq!(binding.reviewed_head(), binding.merged_source());
        assert_eq!(
            MergeBinding::new(sha('a'), sha('b')),
            Err(EventValueError::MergedSourceIsNotTheReviewedHead)
        );
    }

    #[test]
    fn schema_versions_outside_the_supported_range_are_rejected() {
        assert_eq!(SchemaVersion::current().get(), EVENT_SCHEMA_VERSION);
        assert!(SchemaVersion::new(1).is_ok());
        assert_eq!(
            SchemaVersion::new(0),
            Err(EventValueError::UnsupportedSchemaVersion(0))
        );
        assert_eq!(
            SchemaVersion::new(EVENT_SCHEMA_VERSION + 1),
            Err(EventValueError::UnsupportedSchemaVersion(EVENT_SCHEMA_VERSION + 1))
        );
        assert_eq!(
            SchemaVersion::new(u32::MAX),
            Err(EventValueError::UnsupportedSchemaVersion(u32::MAX))
        );
    }

    #[cfg(feature = "full")]
    #[test]
    fn validated_types_round_trip_and_reject_forged_json() {
        let passed = PassedGates::new(vec![gate(GateStatus::Pass, Some(0))]).unwrap();
        let json = serde_json::to_string(&passed).unwrap();
        assert_eq!(serde_json::from_str::<PassedGates>(&json).unwrap(), passed);
        let forged = json.replace("\"pass\"", "\"fail\"").replace("\"exit_code\":0", "\"exit_code\":1");
        assert!(forged.contains("\"fail\""));
        assert!(serde_json::from_str::<PassedGates>(&forged).is_err());

        let failed = FailedGate::new(gate(GateStatus::Fail, Some(1))).unwrap();
        let json = serde_json::to_string(&failed).unwrap();
        assert_eq!(serde_json::from_str::<FailedGate>(&json).unwrap(), failed);
        let forged = r#"{"gate":"g","status":"pass","exit_code":0,"duration_ms":null,"summary":""}"#;
        assert!(serde_json::from_str::<FailedGate>(forged).is_err());

        let binding = MergeBinding::new(sha('a'), sha('a')).unwrap();
        let json = serde_json::to_string(&binding).unwrap();
        assert_eq!(serde_json::from_str::<MergeBinding>(&json).unwrap(), binding);
        let forged = format!(
            r#"{{"reviewed_head":"{}","merged_source":"{}"}}"#,
            sha('a'),
            sha('b')
        );
        assert!(serde_json::from_str::<MergeBinding>(&forged).is_err());

        assert_eq!(serde_json::to_string(&SchemaVersion::current()).unwrap(), "1");
        assert!(serde_json::from_str::<SchemaVersion>("0").is_err());
        assert!(serde_json::from_str::<SchemaVersion>("2").is_err());
        assert!(serde_json::from_str::<SchemaVersion>("-1").is_err());
        assert!(serde_json::from_str::<SchemaVersion>("\"1\"").is_err());
    }
}
