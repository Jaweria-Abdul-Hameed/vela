//! The provider-policy-block signal table (A13, FR-024, `CAPACITY_AND_PROFILES.md` "Provider Policy Blocks").
//!
//! Detection is **unverified `[U]`**: a real block can and must not be provoked, so nothing here is claimed to be what Antigravity
//! actually prints. The table is small and data-driven: each [`Signal`] says where its wording came from, and recorded evidence
//! extends it (and is written up in `docs/research/`). A signal needs *all* of its phrases, so a vague word alone never matches.
//! When a signal fires the caller emits the neutral `ProviderPolicyBlock` event and nothing else: no retry, no cooldown, no
//! profile switch.

use super::excerpt;

/// Where a signal's wording comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Taken from the official changelog or documentation.
    Documented,
    /// Copied from a real occurrence recorded in `docs/research/`.
    Recorded,
    /// Invented to exercise the mapping; never to be mistaken for evidence.
    Synthetic,
}

/// What the block covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// The user's account.
    Account,
    /// The service as a whole.
    Service,
    /// The text does not say.
    Unknown,
}

/// One row of the table.
#[derive(Debug, Clone, Copy)]
pub struct Signal {
    /// A stable identifier for evidence references.
    pub id: &'static str,
    /// Lower-case phrases that must all appear.
    pub all_of: &'static [&'static str],
    /// The scope the wording implies.
    pub scope: Scope,
    /// Where the wording comes from.
    pub source: Source,
}

/// The table. The changelog (S7 in `docs/research/H01_google_position.md`, 2026-07-31) records a distinct sign-in error with an
/// appeal link for accounts blocked for a Terms violation; its exact text is not recorded, so the documented row asks only for
/// the two ideas the changelog states together (a terms violation and an appeal). The synthetic rows exist for tests.
pub const SIGNALS: &[Signal] = &[
    Signal {
        id: "documented-terms-block-with-appeal",
        all_of: &["terms", "violation", "appeal"],
        scope: Scope::Account,
        source: Source::Documented,
    },
    Signal {
        id: "documented-account-blocked-appeal",
        all_of: &["account", "blocked", "appeal"],
        scope: Scope::Account,
        source: Source::Documented,
    },
    Signal {
        id: "synthetic-account-suspended",
        all_of: &["account has been suspended"],
        scope: Scope::Account,
        source: Source::Synthetic,
    },
    Signal {
        id: "synthetic-account-disabled",
        all_of: &["account has been disabled"],
        scope: Scope::Account,
        source: Source::Synthetic,
    },
    Signal {
        id: "synthetic-terms-violation",
        all_of: &["violation of the terms"],
        scope: Scope::Account,
        source: Source::Synthetic,
    },
    Signal {
        id: "synthetic-violates-terms",
        all_of: &["violates the terms of service"],
        scope: Scope::Account,
        source: Source::Synthetic,
    },
    Signal {
        id: "synthetic-service-blocked",
        all_of: &["access to this service has been blocked"],
        scope: Scope::Service,
        source: Source::Synthetic,
    },
];

/// A signal that fired.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detected {
    /// The signal's identifier.
    pub signal: &'static str,
    /// The scope.
    pub scope: Scope,
    /// Where the matching row's wording came from. Synthetic means no real block was seen.
    pub source: Source,
    /// Redacted excerpt of the provider's text.
    pub evidence: String,
}

/// The first signal whose phrases all appear in `text`.
pub fn detect(text: &str) -> Option<Detected> {
    let lower = text.to_ascii_lowercase();
    SIGNALS
        .iter()
        .find(|s| s.all_of.iter().all(|p| lower.contains(p)))
        .map(|s| Detected {
            signal: s.id,
            scope: s.scope,
            source: s.source,
            evidence: excerpt(text),
        })
}
