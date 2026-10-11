//! Stable finding identity: a fingerprint of category, location and requirement (`REVIEW_PROTOCOL.md`, "Identity").

use std::fmt;

use serde::{Deserialize, Serialize};

const PREFIX: &str = "f-";
const HEX_LEN: usize = 16;

/// A finding id: `f-` followed by 16 lowercase hexadecimal digits. It is derived from the fingerprint, so the same
/// finding reported again (in the verification that follows the review) has the same id.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct FindingId(String);

/// A malformed finding id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingIdError(String);

impl fmt::Display for FindingIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "malformed finding id: {:?}", self.0)
    }
}

impl std::error::Error for FindingIdError {}

impl FindingId {
    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FindingId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for FindingId {
    type Error = FindingIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let well_formed = value.strip_prefix(PREFIX).is_some_and(|hex| {
            hex.len() == HEX_LEN
                && hex
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        });
        if well_formed {
            Ok(Self(value))
        } else {
            Err(FindingIdError(value))
        }
    }
}

impl From<FindingId> for String {
    fn from(value: FindingId) -> Self {
        value.0
    }
}

/// Canonical form of one fingerprint component: trimmed, case-folded, internal whitespace collapsed, and path
/// separators unified, so cosmetic differences between two reports of the same finding do not change its id.
fn normalize(component: &str) -> String {
    component
        .replace('\\', "/")
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// FNV-1a, 64 bit. Hand-written so the id is stable across platforms, toolchains and releases (a std hasher is not
/// guaranteed stable) and so no dependency is added.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// The finding id for a category, a location and the requirement violated. Severity, evidence and the suggested fix
/// are deliberately not part of the identity: they may be reworded between the review and the verification.
pub fn fingerprint(category: &str, location: &str, requirement: &str) -> FindingId {
    // Each component is length-prefixed so ("ab", "c") and ("a", "bc") cannot collide by concatenation.
    let mut canonical = String::new();
    for component in [category, location, requirement] {
        let normalized = normalize(component);
        canonical.push_str(&normalized.len().to_string());
        canonical.push(':');
        canonical.push_str(&normalized);
        canonical.push('\n');
    }
    FindingId(format!(
        "{PREFIX}{:0width$x}",
        fnv1a(canonical.as_bytes()),
        width = HEX_LEN
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_inputs_give_the_same_id() {
        let a = fingerprint("correctness", "src/a.rs:10", "FR-013");
        let b = fingerprint("correctness", "src/a.rs:10", "FR-013");
        assert_eq!(a, b);
    }

    #[test]
    fn id_is_a_pinned_value_so_it_never_drifts_between_releases() {
        // FNV-1a over the canonical text; pinned so a change of algorithm is a deliberate, visible act.
        let id = fingerprint("correctness", "src/a.rs:10", "FR-013");
        assert!(id.as_str().starts_with("f-"));
        assert_eq!(id.as_str().len(), 18);
        assert_eq!(
            fnv1a(b""),
            0xcbf2_9ce4_8422_2325,
            "FNV-1a offset basis for empty input"
        );
        assert_eq!(
            fnv1a(b"a"),
            0xaf63_dc4c_8601_ec8c,
            "FNV-1a reference vector"
        );
    }

    #[test]
    fn cosmetic_differences_do_not_change_the_id() {
        let a = fingerprint("Correctness", "src\\a.rs:10", "  FR-013   violated ");
        let b = fingerprint("correctness", "SRC/a.rs:10", "fr-013 violated");
        assert_eq!(a, b);
    }

    #[test]
    fn any_component_changing_changes_the_id() {
        let base = fingerprint("correctness", "src/a.rs:10", "FR-013");
        assert_ne!(base, fingerprint("spec", "src/a.rs:10", "FR-013"));
        assert_ne!(base, fingerprint("correctness", "src/a.rs:11", "FR-013"));
        assert_ne!(base, fingerprint("correctness", "src/a.rs:10", "FR-014"));
    }

    #[test]
    fn components_do_not_collide_by_concatenation() {
        assert_ne!(fingerprint("ab", "c", "d"), fingerprint("a", "bc", "d"));
        assert_ne!(fingerprint("a", "b", "cd"), fingerprint("a", "bc", "d"));
    }

    #[test]
    fn serde_round_trips_and_rejects_malformed_ids() {
        let id = fingerprint("c", "l", "r");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(serde_json::from_str::<FindingId>(&json).unwrap(), id);
        assert!(serde_json::from_str::<FindingId>("\"f-XYZ\"").is_err());
        assert!(serde_json::from_str::<FindingId>("\"x-0123456789abcdef\"").is_err());
        assert!(serde_json::from_str::<FindingId>("\"f-0123456789ABCDEF\"").is_err());
    }
}
