//! Ordered, explainable rules and conflict reporting.
//!
//! A [`RuleSet`] is evaluated first match wins. Rules are plain data (they are persisted by the settings store and
//! serialized into the hook's rule table), and the constructors enforce the invariants the engine's guarantees rest on:
//! an `ALLOW` rule must have a basis (a confirmed-profile match, or a read or write inside the worktree) in a trusted
//! repository, so no rule can grant `ALLOW` to anything else. Conflicts between rules are reported when a rule is created.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::classify::{Facts, OperationClass};
use super::operation::TrustState;
use super::path::Placement;

/// The three outcomes (`SECURITY_AND_PERMISSIONS.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Decision {
    /// Proceed without asking.
    Allow,
    /// Needs a human decision.
    Ask,
    /// Refused; a human prompt cannot grant it.
    Deny,
}

/// A stable rule identifier: lowercase letters, digits, `.`, `-` and `_`, at most 64 characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub struct RuleId(String);

impl RuleId {
    /// Validates and wraps an identifier.
    pub fn new(value: impl Into<String>) -> Result<RuleId, RuleError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 64
            && value.chars().all(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_')
            });
        if valid {
            Ok(RuleId(value))
        } else {
            Err(RuleError::InvalidId)
        }
    }

    /// The identifier text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for RuleId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        RuleId::new(s).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for RuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The conjunction of conditions under which a rule applies. An absent condition matches anything.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RuleCondition {
    /// The operation class.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<OperationClass>,
    /// The executable lookup name (basename without extension), lowercase.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable: Option<String>,
    /// Where the operation's paths lie relative to the worktree root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<Placement>,
    /// The repository trust state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust: Option<TrustState>,
    /// Whether the command equals a confirmed profile entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_match: Option<bool>,
}

impl RuleCondition {
    pub(crate) fn matches(&self, facts: &Facts, trust: TrustState) -> bool {
        self.class.is_none_or(|c| c == facts.class)
            && self
                .executable
                .as_ref()
                .is_none_or(|e| facts.executable.as_deref() == Some(e.as_str()))
            && self.placement.is_none_or(|p| p == facts.placement)
            && self.trust.is_none_or(|t| t == trust)
            && self.profile_match.is_none_or(|m| m == facts.profile_match)
    }

    /// True when every operation matching `other` also matches `self`.
    fn covers(&self, other: &RuleCondition) -> bool {
        fn dim<T: PartialEq>(a: &Option<T>, b: &Option<T>) -> bool {
            a.is_none() || a == b
        }
        dim(&self.class, &other.class)
            && dim(&self.executable, &other.executable)
            && dim(&self.placement, &other.placement)
            && dim(&self.trust, &other.trust)
            && dim(&self.profile_match, &other.profile_match)
    }

    /// True when some operation can match both.
    fn overlaps(&self, other: &RuleCondition) -> bool {
        fn dim<T: PartialEq>(a: &Option<T>, b: &Option<T>) -> bool {
            match (a, b) {
                (Some(x), Some(y)) => x == y,
                _ => true,
            }
        }
        dim(&self.class, &other.class)
            && dim(&self.executable, &other.executable)
            && dim(&self.placement, &other.placement)
            && dim(&self.trust, &other.trust)
            && dim(&self.profile_match, &other.profile_match)
    }

    /// True when the condition ties an `ALLOW` to a trusted repository and a permitted basis.
    fn has_allow_basis(&self) -> bool {
        let trusted = self.trust == Some(TrustState::Trusted);
        let profile = self.profile_match == Some(true);
        let worktree_io = matches!(
            self.class,
            Some(OperationClass::FileRead | OperationClass::FileWrite)
        ) && self.placement == Some(Placement::InsideRoot);
        trusted && (profile || worktree_io)
    }
}

/// One rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    /// Stable identifier, reported in every decision the rule makes.
    pub id: RuleId,
    /// Fixed, argument-free explanation shown with the decision.
    pub reason: String,
    /// The outcome.
    pub effect: Decision,
    /// When the rule applies.
    pub when: RuleCondition,
}

/// Why a rule or a rule set was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleError {
    /// The identifier is not valid.
    InvalidId,
    /// The reason is empty or longer than 200 characters.
    InvalidReason,
    /// The executable condition is not a lowercase bare name.
    InvalidExecutable,
    /// Another rule already has this identifier.
    DuplicateId(RuleId),
    /// An `ALLOW` rule without a permitted basis (trusted repository and a profile match or a worktree read or write).
    AllowWithoutBasis(RuleId),
    /// A rule created by a user may only tighten (`ASK` or `DENY`).
    UserAllowNotPermitted(RuleId),
    /// The rule can never apply because an earlier rule covers it with a different outcome.
    Shadowed(RuleConflict),
    /// The rule set has more than [`MAX_RULES`] rules.
    TooManyRules,
    /// The user tier does not lie within the rules, or the tiers hold the wrong outcomes (only `DENY` before the user tier,
    /// nothing that allows inside it).
    InvalidUserTier,
}

impl fmt::Display for RuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuleError::InvalidId => f.write_str("invalid rule id"),
            RuleError::InvalidReason => f.write_str("rule reason is empty or too long"),
            RuleError::InvalidExecutable => {
                f.write_str("rule executable is not a lowercase bare name")
            }
            RuleError::DuplicateId(id) => write!(f, "duplicate rule id {id}"),
            RuleError::AllowWithoutBasis(id) => write!(f, "ALLOW rule {id} has no permitted basis"),
            RuleError::UserAllowNotPermitted(id) => {
                write!(f, "rule {id}: user rules may only ASK or DENY")
            }
            RuleError::Shadowed(c) => write!(
                f,
                "rule {} can never apply: rule {} covers it",
                c.later, c.earlier
            ),
            RuleError::TooManyRules => f.write_str("too many rules"),
            RuleError::InvalidUserTier => f.write_str("user tier is outside the rules"),
        }
    }
}

impl std::error::Error for RuleError {}

/// How two rules interact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictKind {
    /// The later rule is wholly covered by the earlier one and has a different outcome: it can never apply.
    Shadowed,
    /// The later rule is wholly covered by the earlier one with the same outcome: redundant.
    Redundant,
    /// The rules overlap with different outcomes and neither covers the other: the earlier one wins on the overlap.
    Overlap,
}

/// A reported interaction between an earlier and a later rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleConflict {
    /// The rule that is evaluated first.
    pub earlier: RuleId,
    /// The rule that is evaluated later.
    pub later: RuleId,
    /// The kind of interaction.
    pub kind: ConflictKind,
}

/// Most rules a set may hold.
pub const MAX_RULES: usize = 1024;

impl Rule {
    /// Checks the rule's own fields.
    pub fn validate(&self) -> Result<(), RuleError> {
        if self.reason.trim().is_empty() || self.reason.len() > 200 {
            return Err(RuleError::InvalidReason);
        }
        if let Some(exe) = &self.when.executable {
            let valid = !exe.is_empty()
                && exe.len() <= 64
                && exe.chars().all(|c| {
                    c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_' | '.')
                });
            if !valid {
                return Err(RuleError::InvalidExecutable);
            }
        }
        if self.effect == Decision::Allow && !self.when.has_allow_basis() {
            return Err(RuleError::AllowWithoutBasis(self.id.clone()));
        }
        Ok(())
    }
}

/// An ordered rule list, first match wins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleSet {
    rules: Vec<Rule>,
    /// Index of the user tier: after the built-in `DENY` tier, before everything that can allow.
    user_start: usize,
    /// Number of user rules, which occupy `user_start..user_start + user_len` in creation order.
    user_len: usize,
}

impl RuleSet {
    /// Builds a set from rules in order, validating each and refusing duplicates and shadowed rules. The user tier is
    /// `user_len` rules starting at `user_start`; [`insert_user_rule`](Self::insert_user_rule) appends to it.
    pub fn from_rules(
        rules: Vec<Rule>,
        user_start: usize,
        user_len: usize,
    ) -> Result<RuleSet, RuleError> {
        if rules.len() > MAX_RULES {
            return Err(RuleError::TooManyRules);
        }
        if user_start
            .checked_add(user_len)
            .is_none_or(|end| end > rules.len())
        {
            return Err(RuleError::InvalidUserTier);
        }
        // The tiers: only `DENY` before the user tier, and nothing that allows inside it.
        let tiers_hold = rules[..user_start]
            .iter()
            .all(|r| r.effect == Decision::Deny)
            && rules[user_start..user_start + user_len]
                .iter()
                .all(|r| r.effect != Decision::Allow);
        if !tiers_hold {
            return Err(RuleError::InvalidUserTier);
        }
        let mut set = RuleSet {
            rules: Vec::with_capacity(rules.len()),
            user_start,
            user_len,
        };
        for rule in rules {
            rule.validate()?;
            set.check_new(&rule, set.rules.len())?;
            set.rules.push(rule);
        }
        Ok(set)
    }

    /// The rules in evaluation order.
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// The index of the user tier.
    pub fn user_start(&self) -> usize {
        self.user_start
    }

    /// The number of user rules.
    pub fn user_len(&self) -> usize {
        self.user_len
    }

    /// Appends a user rule that can only tighten (`ASK` or `DENY`), ahead of every rule that can allow. Returns the
    /// non-fatal interactions (redundancy, overlaps) so the settings UI can show them; a rule that could never apply is
    /// refused with [`RuleError::Shadowed`].
    pub fn insert_user_rule(&mut self, rule: Rule) -> Result<Vec<RuleConflict>, RuleError> {
        if rule.effect == Decision::Allow {
            return Err(RuleError::UserAllowNotPermitted(rule.id));
        }
        rule.validate()?;
        if self.rules.len() >= MAX_RULES {
            return Err(RuleError::TooManyRules);
        }
        let at = self.user_start + self.user_len;
        let conflicts = self.check_new(&rule, at)?;
        self.rules.insert(at, rule);
        self.user_len += 1;
        Ok(conflicts)
    }

    /// Checks `rule` against the rules before position `at` (shadowing) and reports the interactions with all rules.
    fn check_new(&self, rule: &Rule, at: usize) -> Result<Vec<RuleConflict>, RuleError> {
        if self.rules.iter().any(|r| r.id == rule.id) {
            return Err(RuleError::DuplicateId(rule.id.clone()));
        }
        let mut report = Vec::new();
        for (index, other) in self.rules.iter().enumerate() {
            let (earlier, later, earlier_rule, later_rule) = if index < at {
                (&other.id, &rule.id, other, rule)
            } else {
                (&rule.id, &other.id, rule, other)
            };
            if !earlier_rule.when.overlaps(&later_rule.when) {
                continue;
            }
            let covers = earlier_rule.when.covers(&later_rule.when);
            let same = earlier_rule.effect == later_rule.effect;
            let kind = match (covers, same) {
                (true, false) => ConflictKind::Shadowed,
                (true, true) => ConflictKind::Redundant,
                (false, false) => ConflictKind::Overlap,
                (false, true) => continue,
            };
            let conflict = RuleConflict {
                earlier: earlier.clone(),
                later: later.clone(),
                kind,
            };
            // Only the new rule being unreachable is fatal; a new rule that shadows an existing one is reported.
            if kind == ConflictKind::Shadowed && index < at {
                return Err(RuleError::Shadowed(conflict));
            }
            report.push(conflict);
        }
        Ok(report)
    }

    /// The first rule matching `facts` in a repository with the given trust state.
    pub(crate) fn first_match(&self, facts: &Facts, trust: TrustState) -> Option<&Rule> {
        self.rules.iter().find(|r| r.when.matches(facts, trust))
    }
}

impl<'de> Deserialize<'de> for RuleSet {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            rules: Vec<Rule>,
            user_start: usize,
            user_len: usize,
        }
        let raw = Raw::deserialize(d)?;
        RuleSet::from_rules(raw.rules, raw.user_start, raw.user_len)
            .map_err(serde::de::Error::custom)
    }
}
