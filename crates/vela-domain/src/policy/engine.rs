//! The policy engine: classify, match the first rule, and enforce the `ALLOW` invariant.

use serde::{Deserialize, Serialize};

use super::classify::{Facts, OperationClass, analyze};
use super::defaults::default_rules;
use super::operation::{Operation, PolicyContext, TrustState};
use super::path::Placement;
use super::rule::{Decision, RuleId, RuleSet};

/// What a decision applies to. It never carries command arguments or paths, which can hold secrets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionScope {
    /// The operation class.
    pub class: OperationClass,
    /// Where the operation's paths lie relative to the worktree root.
    pub placement: Placement,
    /// The executable lookup name, for commands.
    pub executable: Option<String>,
}

/// The engine's answer: the outcome, the rule that produced it, why, and what it covers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyDecision {
    /// `ALLOW`, `ASK` or `DENY`.
    pub decision: Decision,
    /// The rule that decided.
    pub rule_id: RuleId,
    /// A fixed explanation, never containing arguments or paths.
    pub reason: String,
    /// What the decision covers.
    pub scope: DecisionScope,
}

/// Identifier of the invariant that downgrades an `ALLOW` that lacks its basis.
const INVARIANT_RULE: &str = "vela.invariant.allow-basis";

/// A pure policy engine over a context and an ordered rule set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyEngine {
    context: PolicyContext,
    rules: RuleSet,
}

impl PolicyEngine {
    /// An engine over the default rules.
    pub fn new(context: PolicyContext) -> PolicyEngine {
        PolicyEngine {
            context,
            rules: default_rules(),
        }
    }

    /// An engine over a specific rule set (built by [`default_rules_with`](super::default_rules_with) or a hook table).
    pub fn with_rules(context: PolicyContext, rules: RuleSet) -> PolicyEngine {
        PolicyEngine { context, rules }
    }

    /// The context.
    pub fn context(&self) -> &PolicyContext {
        &self.context
    }

    /// The rules.
    pub fn rules(&self) -> &RuleSet {
        &self.rules
    }

    /// Decides one operation. Total: every input yields a decision, and a decision other than `ASK` or `DENY` is only
    /// returned for a trusted repository and a confirmed-profile command or a read or write inside the worktree.
    pub fn evaluate(&self, operation: &Operation) -> PolicyDecision {
        self.decide(&analyze(operation, &self.context))
    }

    pub(crate) fn decide(&self, facts: &Facts) -> PolicyDecision {
        let scope = DecisionScope {
            class: facts.class,
            placement: facts.placement,
            executable: facts.executable.clone(),
        };
        let trust = self.context.trust();
        let Some(rule) = self.rules.first_match(facts, trust) else {
            // A rule set without a catch-all still fails safe.
            return PolicyDecision {
                decision: Decision::Ask,
                rule_id: RuleId::new("vela.invariant.no-match").expect("fixed id"),
                reason: "no rule matched; unknown operations are asked".to_string(),
                scope,
            };
        };
        let mut reason = rule.reason.clone();
        if let Some(rejection) = facts.rejection {
            reason = format!("{reason}: {}", rejection.describe());
        }
        if rule.effect == Decision::Allow && !allow_basis_holds(facts, trust) {
            return PolicyDecision {
                decision: Decision::Ask,
                rule_id: RuleId::new(INVARIANT_RULE).expect("fixed id"),
                reason: "allow without a confirmed-profile or worktree basis is downgraded to ask"
                    .to_string(),
                scope,
            };
        }
        PolicyDecision {
            decision: rule.effect,
            rule_id: rule.id.clone(),
            reason,
            scope,
        }
    }
}

/// The only situations in which `ALLOW` is valid, checked again after rule matching (defense in depth).
fn allow_basis_holds(facts: &Facts, trust: TrustState) -> bool {
    use OperationClass::*;
    let never_allowed = matches!(
        facts.class,
        CredentialAccess
            | SecurityControlChange
            | GitForcePush
            | GitDestructive
            | DestructiveDelete
            | ProtectedPathChange
            | Unparseable
    );
    trust == TrustState::Trusted
        && !never_allowed
        && (facts.profile_match
            || (matches!(facts.class, FileRead | FileWrite)
                && facts.placement == Placement::InsideRoot))
}
