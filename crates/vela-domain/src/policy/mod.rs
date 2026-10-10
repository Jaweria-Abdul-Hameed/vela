//! Policy engine core (D05; component C03, FR-030, FR-031, FR-038).
//!
//! Every operation an agent attempts is classified by ordered, explainable rules into `ALLOW`, `ASK` or `DENY`:
//!
//! - an [`Operation`] (a command line, or a file read, write or delete) and a [`PolicyContext`] (worktree root,
//!   repository trust state, run mode, confirmed command profile) go in;
//! - a small [`normalize`]r accepts exactly one simple command in PowerShell or `cmd` syntax and rejects everything
//!   richer, which becomes `ASK`;
//! - the [`default_rules`] encode `SECURITY_AND_PERMISSIONS.md` and ADR-013; first match wins, and conflicts between
//!   rules are reported when a rule is created;
//! - `ALLOW` exists only for a command in the confirmed profile of a trusted repository, or a read or write inside the
//!   worktree of one. The engine re-checks this after rule matching, so no rule set can widen it;
//! - [`HookRuleTable`] and [`native_allow_rules`] are generated from the same engine for `vela-hook` and the native
//!   settings.
//!
//! The module is pure (no I/O, no clock) and builds with only `serde`, so the hook binary can depend on it through the
//! `hook-table` feature. Repository, issue and `AGENTS.md` text never reaches policy: callers pass the operation, and the
//! trust state and profile come from the settings store (ADR-013). Decisions, errors and `Debug` output never contain
//! command arguments or paths.

#![warn(missing_docs)]

mod classify;
mod defaults;
mod engine;
mod normalizer;
mod operation;
mod path;
mod rule;
mod table;

#[cfg(test)]
mod tests;

pub use classify::{OperationClass, PROTECTED_COMPONENTS};
pub use defaults::{default_rules, default_rules_with};
pub use engine::{DecisionScope, PolicyDecision, PolicyEngine};
pub use normalizer::{
    Dialect, MAX_COMMAND_LEN, MAX_TOKENS, NormalizedCommand, Rejection, normalize,
};
pub use operation::{
    CommandProfile, ContextError, MAX_PROFILE_ENTRIES, Operation, PolicyContext, ProfileEntry,
    RunMode, TrustState,
};
pub use path::Placement;
pub use rule::{
    ConflictKind, Decision, MAX_RULES, Rule, RuleCondition, RuleConflict, RuleError, RuleId,
    RuleSet,
};
pub use table::{
    HOOK_TABLE_FORMAT_VERSION, HookRuleTable, NativeAllowRule, TableError, native_allow_rules,
};
