//! The default rule set (`SECURITY_AND_PERMISSIONS.md`, ADR-009, ADR-013).
//!
//! Order is the policy. Tiers, in evaluation order:
//!
//! 1. **`DENY`** classes that no prompt may grant: credential access, disabling security controls, force pushes,
//!    destructive Git, destructive deletes, writes to protected locations.
//! 2. **User rules** (tighten only: `ASK` or `DENY`), inserted at [`RuleSet::user_start`].
//! 3. **Untrusted floor**: in an `UNTRUSTED` repository nothing below `DENY` is better than `ASK` (ADR-013).
//! 4. **`ALLOW`** in a trusted repository, and only for a confirmed-profile command or a read or write inside the
//!    worktree.
//! 5. **`ASK`** per class, so every decision names the reason, then a catch-all `ASK`.

use super::classify::OperationClass::{self, *};
use super::operation::TrustState;
use super::path::Placement;
use super::rule::{Decision, Rule, RuleCondition, RuleId, RuleSet};

fn rule(id: &str, effect: Decision, reason: &str, when: RuleCondition) -> Rule {
    Rule {
        id: RuleId::new(id).expect("built-in rule id"),
        reason: reason.to_string(),
        effect,
        when,
    }
}

fn class(c: OperationClass) -> RuleCondition {
    RuleCondition {
        class: Some(c),
        ..RuleCondition::default()
    }
}

/// The default rules with no user rules.
pub fn default_rules() -> RuleSet {
    default_rules_with(Vec::new())
        .expect("the built-in rule set is valid")
        .0
}

/// The default rules with `user_rules` inserted in order at the user tier. Returns the set and the reported
/// interactions; a user rule that could never apply, or that would allow, is refused.
pub fn default_rules_with(
    user_rules: Vec<Rule>,
) -> Result<(RuleSet, Vec<super::rule::RuleConflict>), super::rule::RuleError> {
    use Decision::{Allow, Ask, Deny};
    let mut rules = vec![
        rule(
            "vela.deny.credential-access",
            Deny,
            "reads credentials, tokens, key material or the environment",
            class(CredentialAccess),
        ),
        rule(
            "vela.deny.security-control",
            Deny,
            "disables or weakens a security control",
            class(SecurityControlChange),
        ),
        rule(
            "vela.deny.force-push",
            Deny,
            "force push, mirror push or remote branch deletion",
            class(GitForcePush),
        ),
        rule(
            "vela.deny.git-destructive",
            Deny,
            "discards uncommitted work or history",
            class(GitDestructive),
        ),
        rule(
            "vela.deny.destructive-delete",
            Deny,
            "deletes outside the worktree, the worktree itself, or destroys a disk",
            class(DestructiveDelete),
        ),
        rule(
            "vela.deny.protected-path-change",
            Deny,
            "writes or deletes Git internals or agent and Vela policy locations",
            class(ProtectedPathChange),
        ),
    ];
    let user_start = rules.len();
    rules.extend([
        rule(
            "vela.ask.untrusted-repository",
            Ask,
            "the repository is not trusted; only data inspection is permitted without a decision",
            RuleCondition {
                trust: Some(TrustState::Untrusted),
                ..RuleCondition::default()
            },
        ),
        rule(
            "vela.allow.confirmed-profile",
            Allow,
            "matches the user-confirmed command profile of a trusted repository",
            RuleCondition {
                trust: Some(TrustState::Trusted),
                profile_match: Some(true),
                ..RuleCondition::default()
            },
        ),
        rule(
            "vela.allow.worktree-read",
            Allow,
            "reads inside the assigned worktree of a trusted repository",
            RuleCondition {
                trust: Some(TrustState::Trusted),
                class: Some(FileRead),
                placement: Some(Placement::InsideRoot),
                ..RuleCondition::default()
            },
        ),
        rule(
            "vela.allow.worktree-write",
            Allow,
            "writes inside the assigned worktree of a trusted repository",
            RuleCondition {
                trust: Some(TrustState::Trusted),
                class: Some(FileWrite),
                placement: Some(Placement::InsideRoot),
                ..RuleCondition::default()
            },
        ),
        rule(
            "vela.ask.file-read",
            Ask,
            "reads a path outside the worktree or one that cannot be resolved",
            class(FileRead),
        ),
        rule(
            "vela.ask.file-write",
            Ask,
            "writes a path outside the worktree or one that cannot be resolved",
            class(FileWrite),
        ),
        rule("vela.ask.delete", Ask, "deletes files", class(Delete)),
        rule(
            "vela.ask.recursive-delete",
            Ask,
            "deletes a directory tree inside the worktree",
            class(RecursiveDelete),
        ),
        rule(
            "vela.ask.file-mutate",
            Ask,
            "creates, copies, moves or modifies files",
            class(FileMutate),
        ),
        rule(
            "vela.ask.git-inspect",
            Ask,
            "Git inspection that is not in the confirmed profile",
            class(GitInspect),
        ),
        rule(
            "vela.ask.git-mutate",
            Ask,
            "changes Git state",
            class(GitMutate),
        ),
        rule(
            "vela.ask.git-push",
            Ask,
            "pushes to a remote",
            class(GitPush),
        ),
        rule(
            "vela.ask.git-config-override",
            Ask,
            "Git with a configuration or location override",
            class(GitConfigOverride),
        ),
        rule(
            "vela.ask.system-configuration",
            Ask,
            "changes system configuration, installs software or elevates",
            class(SystemConfiguration),
        ),
        rule(
            "vela.ask.network-access",
            Ask,
            "network or remote access",
            class(NetworkAccess),
        ),
        rule(
            "vela.ask.nested-shell",
            Ask,
            "runs a shell or launches further commands",
            class(NestedShell),
        ),
        rule(
            "vela.ask.infrastructure-destructive",
            Ask,
            "destructive database, cloud or cluster operation",
            class(InfrastructureDestructive),
        ),
        rule(
            "vela.ask.unknown-executable",
            Ask,
            "executable is not in the confirmed command profile",
            class(UnknownExecutable),
        ),
        rule(
            "vela.ask.unparseable",
            Ask,
            "not a single simple command that can be normalized",
            class(Unparseable),
        ),
        rule(
            "vela.ask.fallback",
            Ask,
            "no rule matched; unknown operations are asked",
            RuleCondition::default(),
        ),
    ]);
    let mut set = RuleSet::from_rules(rules, user_start, 0)?;
    let mut report = Vec::new();
    for user in user_rules {
        report.extend(set.insert_user_rule(user)?);
    }
    Ok((set, report))
}
