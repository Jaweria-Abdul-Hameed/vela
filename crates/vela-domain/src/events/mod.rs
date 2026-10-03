//! Events and the IPC command registry (F02; `SYSTEM_ARCHITECTURE.md` section 4, `IMPLEMENTATION_ARCHITECTURE.md`
//! section 6).
//!
//! - `vocabulary`: the closed event-kind vocabulary and one typed payload DTO per kind.
//! - this module: `EventEnvelope`, the one wire shape for every journaled and streamed event.
//! - `command_registry`: the Rust-side command registry type and the Rust/TypeScript contract comparison.
//! - `contract_export` (tests only): generates the TypeScript contracts with `ts-rs` and checks them against the committed
//!   files in `packages/contracts/src/generated`.

#![warn(missing_docs)]

pub mod command_registry;
#[cfg(all(test, feature = "full"))]
mod contract_export;
#[cfg(test)]
mod sample;
mod vocabulary;

pub use vocabulary::*;

use serde::{Deserialize, Serialize};

use crate::ids::{RunId, TicketId, WorkerId};

/// Schema version stamped on every envelope this build produces.
pub const EVENT_SCHEMA_VERSION: u32 = 1;

/// `EventEnvelope { seq, ts, run_id?, ticket_id?, worker_id?, kind, payload, schema_version }`
/// (`IMPLEMENTATION_ARCHITECTURE.md` section 6).
///
/// `kind` and `payload` are flattened out of [`Event`], so the wire form is flat and the payload type always matches
/// the kind. `seq` is assigned by the journal writer, never by the producer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "full", derive(ts_rs::TS))]
pub struct EventEnvelope {
    /// Position in the journal (monotonic per journal).
    #[cfg_attr(feature = "full", ts(type = "number"))]
    pub seq: u64,
    /// UTC timestamp in epoch milliseconds.
    #[cfg_attr(feature = "full", ts(type = "number"))]
    pub ts: i64,
    /// The run the event belongs to, if any.
    pub run_id: Option<RunId>,
    /// The ticket the event concerns, if any.
    pub ticket_id: Option<TicketId>,
    /// The worker the event concerns, if any.
    pub worker_id: Option<WorkerId>,
    /// Kind and typed payload.
    #[serde(flatten)]
    pub event: Event,
    /// Schema version of the payload.
    pub schema_version: u32,
}

impl EventEnvelope {
    /// The kind of the carried event.
    pub fn kind(&self) -> EventKind {
        self.event.kind()
    }
}

#[cfg(all(test, feature = "full"))]
mod tests {
    use super::sample::Sample;
    use super::*;
    use std::collections::HashSet;

    fn envelope(event: Event) -> EventEnvelope {
        EventEnvelope {
            seq: 42,
            ts: 1_700_000_000_000,
            run_id: Some(RunId::sample()),
            ticket_id: Some(TicketId::sample()),
            worker_id: None,
            event,
            schema_version: EVENT_SCHEMA_VERSION,
        }
    }

    /// The names in section 4 of the specification, in order: every backticked CamelCase token in that section.
    fn specified_kinds() -> Vec<String> {
        const SPEC: &str = include_str!("../../../../docs/architecture/SYSTEM_ARCHITECTURE.md");
        let section = SPEC
            .split("## 4. Event model")
            .nth(1)
            .and_then(|rest| rest.split("## 5. IPC").next())
            .expect("section 4 of SYSTEM_ARCHITECTURE.md");
        section
            .split('`')
            .skip(1)
            .step_by(2)
            .filter(|t| {
                t.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                    && t.chars().all(|c| c.is_ascii_alphanumeric())
            })
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn vocabulary_matches_the_specification_exactly() {
        let specified = specified_kinds();
        let modelled: Vec<&str> = EventKind::ALL.iter().map(|k| k.as_str()).collect();
        assert_eq!(
            modelled, specified,
            "EventKind::ALL must equal section 4, in order"
        );
        assert_eq!(modelled.len(), 53);
        let unique: HashSet<_> = modelled.iter().collect();
        assert_eq!(unique.len(), modelled.len(), "duplicate kind");
    }

    #[test]
    fn every_kind_has_one_sample_event_and_provider_policy_block_kinds_are_neutral() {
        let samples = sample_events();
        let kinds: Vec<EventKind> = samples.iter().map(Event::kind).collect();
        assert_eq!(kinds, EventKind::ALL);
        for needed in ["ProviderPolicyBlockRecorded", "ProviderPolicyBlockCleared"] {
            assert!(EventKind::ALL.iter().any(|k| k.as_str() == needed));
        }
    }

    #[test]
    fn kind_wire_name_is_the_specification_name() {
        for kind in EventKind::ALL {
            let wire = serde_json::to_string(kind).unwrap();
            assert_eq!(wire, format!("\"{}\"", kind.as_str()));
            assert_eq!(serde_json::from_str::<EventKind>(&wire).unwrap(), *kind);
        }
        assert!(serde_json::from_str::<EventKind>("\"NotAnEvent\"").is_err());
    }

    #[test]
    fn every_event_kind_round_trips_with_a_typed_payload() {
        for event in sample_events() {
            let json = serde_json::to_string(&event).unwrap();
            let value: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(value["kind"], event.kind().as_str(), "{json}");
            assert!(value["payload"].is_object(), "{json}");
            let back: Event = serde_json::from_str(&json).unwrap();
            assert_eq!(back, event, "{json}");
        }
    }

    #[test]
    fn every_envelope_round_trips_with_a_flat_wire_shape() {
        for event in sample_events() {
            let env = envelope(event);
            let json = serde_json::to_string(&env).unwrap();
            let value: serde_json::Value = serde_json::from_str(&json).unwrap();
            for key in [
                "seq",
                "ts",
                "run_id",
                "ticket_id",
                "worker_id",
                "kind",
                "payload",
                "schema_version",
            ] {
                assert!(value.get(key).is_some(), "missing {key} in {json}");
            }
            assert_eq!(value["kind"], env.kind().as_str());
            assert_eq!(value["worker_id"], serde_json::Value::Null);
            assert_eq!(serde_json::from_str::<EventEnvelope>(&json).unwrap(), env);
        }
    }

    #[test]
    fn an_unknown_kind_or_a_payload_of_the_wrong_shape_is_rejected() {
        let ok = serde_json::to_value(envelope(Event::RunPaused(RunPausedPayload {
            reason: "x".into(),
        })))
        .unwrap();

        let mut unknown = ok.clone();
        unknown["kind"] = "NotAnEvent".into();
        assert!(serde_json::from_value::<EventEnvelope>(unknown).is_err());

        let mut wrong_shape = ok;
        wrong_shape["kind"] = "RunCreated".into();
        assert!(serde_json::from_value::<EventEnvelope>(wrong_shape).is_err());
    }

    #[test]
    fn identifiers_inside_an_envelope_are_validated_on_read() {
        let mut value = serde_json::to_value(envelope(Event::TicketReady(TicketReadyPayload {
            frontier_size: 1,
        })))
        .unwrap();
        value["run_id"] = "has space".into();
        assert!(serde_json::from_value::<EventEnvelope>(value).is_err());
    }
}
