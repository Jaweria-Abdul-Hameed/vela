//! Generated artifacts: the rule table the `vela-hook` binary evaluates, and the provider-neutral allow rules that the
//! Antigravity adapter renders into the isolated profile's native settings (ADR-018 section 3).
//!
//! Both are derived from the same [`PolicyEngine`], so the hook and the native layer cannot disagree with the engine. The
//! hook table is serde-only (no `serde_json`), so it builds in the `hook-table` configuration; the hook binary owns the
//! file encoding and the fail-closed behavior on a missing or malformed table (A04).

use serde::{Deserialize, Serialize};

use super::classify::{WriteExclusions, command_facts_for_parts, write_exclusions};
use super::engine::PolicyEngine;
use super::normalizer::Dialect;
use super::operation::{
    CommandProfile, ContextError, PolicyContext, ProfileEntry, RunMode, TrustState,
};
use super::rule::{Decision, RuleError, RuleSet};

/// The rule-table format version. The reader refuses any other version.
pub const HOOK_TABLE_FORMAT_VERSION: u32 = 1;

/// The complete input of a policy decision, in a form the hook can load. Loading re-validates every part, so a table
/// edited into an inconsistent state is refused rather than partly applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookRuleTable {
    /// Must equal [`HOOK_TABLE_FORMAT_VERSION`].
    pub format_version: u32,
    /// The assigned worktree root.
    pub worktree_root: String,
    /// The repository trust state.
    pub trust: TrustState,
    /// The run mode the table was generated for.
    pub run_mode: RunMode,
    /// The confirmed command profile.
    pub profile: Vec<ProfileEntry>,
    /// The ordered rules.
    pub rules: RuleSet,
}

/// Why a table could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableError {
    /// The format version is not supported.
    UnsupportedVersion(u32),
    /// The context is invalid.
    Context(ContextError),
    /// The rule set is invalid.
    Rules(RuleError),
}

impl std::fmt::Display for TableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TableError::UnsupportedVersion(v) => write!(f, "unsupported rule table version {v}"),
            TableError::Context(e) => write!(f, "invalid rule table context: {e}"),
            TableError::Rules(e) => write!(f, "invalid rule table rules: {e}"),
        }
    }
}

impl std::error::Error for TableError {}

impl HookRuleTable {
    /// The table for `engine`.
    pub fn from_engine(engine: &PolicyEngine) -> HookRuleTable {
        let ctx = engine.context();
        HookRuleTable {
            format_version: HOOK_TABLE_FORMAT_VERSION,
            worktree_root: ctx.worktree_root(),
            trust: ctx.trust(),
            run_mode: ctx.run_mode(),
            profile: ctx.profile().entries().to_vec(),
            rules: engine.rules().clone(),
        }
    }

    /// Validates the table and builds the engine it describes.
    pub fn into_engine(self) -> Result<PolicyEngine, TableError> {
        if self.format_version != HOOK_TABLE_FORMAT_VERSION {
            return Err(TableError::UnsupportedVersion(self.format_version));
        }
        let profile = CommandProfile::new(self.profile).map_err(TableError::Context)?;
        let context = PolicyContext::new(&self.worktree_root, self.trust, self.run_mode, profile)
            .map_err(TableError::Context)?;
        // Re-validate the rules: a deserialized set was validated, a constructed one may not have been.
        let rules = RuleSet::from_rules(
            self.rules.rules().to_vec(),
            self.rules.user_start(),
            self.rules.user_len(),
        )
        .map_err(TableError::Rules)?;
        Ok(PolicyEngine::with_rules(context, rules))
    }
}

/// A native allow entry, independent of any provider's rule syntax. The adapter renders these (exact command patterns and
/// a path-scoped write rule) and owns the provider-specific format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NativeAllowRule {
    /// Allow exactly this command, with exactly these arguments, in this directory.
    Command {
        /// The normalized executable.
        executable: String,
        /// The literal arguments.
        args: Vec<String>,
        /// The canonical absolute working directory.
        cwd: String,
    },
    /// Allow writes below this directory. Policy denies protected locations and credential material inside it (the hook
    /// enforces that); the adapter adds path-scoped deny rules for `exclusions` where the native syntax allows, so the
    /// native envelope stays within what policy allows (ADR-009 NPP-2).
    WriteWithin {
        /// The canonical worktree root.
        root: String,
        /// What is never writable below `root`.
        exclusions: WriteExclusions,
    },
}

/// The native allow rules for `engine`: nothing for an untrusted repository; otherwise one exact entry per profile
/// command the engine itself would `ALLOW` (a profile entry that classifies as `DENY` or `ASK` is never emitted), and the
/// worktree write rule.
pub fn native_allow_rules(engine: &PolicyEngine) -> Vec<NativeAllowRule> {
    let ctx = engine.context();
    if ctx.trust() != TrustState::Trusted {
        return Vec::new();
    }
    let mut out = Vec::new();
    for entry in ctx.profile().entries() {
        // A stored entry has no dialect, and a few classifications depend on it, so emit only what the engine allows
        // under each.
        let allowed_everywhere = [Dialect::PowerShell, Dialect::Cmd]
            .into_iter()
            .all(|dialect| {
                let facts = command_facts_for_parts(
                    entry.executable(),
                    entry.args(),
                    entry.cwd(),
                    ctx,
                    dialect,
                );
                engine.decide(&facts).decision == Decision::Allow
            });
        if allowed_everywhere {
            out.push(NativeAllowRule::Command {
                executable: entry.executable().to_string(),
                args: entry.args().to_vec(),
                cwd: entry.cwd().to_string(),
            });
        }
    }
    out.push(NativeAllowRule::WriteWithin {
        root: ctx.worktree_root(),
        exclusions: write_exclusions(),
    });
    out
}
