//! Scoped allow rules (K09): a rule is created from an intervention only after the user was shown its exact scope. An allow-once
//! never becomes a permanent rule. A rule can only turn an `ASK` into an allow; it can never override a `DENY`, and some classes
//! of operation can never be covered by a standing rule at all.

use std::sync::Arc;

use vela_domain::errors::{ErrorClass, VelaError};
use vela_persistence::repositories::k01_approvals::{self, RuleRow};

use crate::orchestrator::Services;

/// How long an allow-once rule lives.
pub const ONCE_TTL_MS: i64 = 10 * 60 * 1000;

/// Classes (by their `Debug` name) that no standing rule may cover: the policy keeps asking, or refusing, whatever the user clicked
/// before.
pub const NEVER_RULE_CLASSES: &[&str] = &[
    "DestructiveDelete",
    "ProtectedPathChange",
    "GitForcePush",
    "GitDestructive",
    "GitConfigOverride",
    "CredentialAccess",
    "SecurityControlChange",
    "SystemConfiguration",
    "NestedShell",
    "InfrastructureDestructive",
    "Unparseable",
];

/// The exact words the user sees before a rule exists.
pub fn scope_text(class: &str, executable: &str, kind: &str) -> String {
    let how = if kind == "ONCE" {
        "once, for the next 10 minutes".to_owned()
    } else {
        "every time, until you revoke it".to_owned()
    };
    format!("Allow `{executable}` ({class}) in this project {how}. Anything else still asks.")
}

/// Rules over the shared services.
pub struct RuleService {
    services: Arc<Services>,
}

impl RuleService {
    /// Rules over `services`.
    pub fn new(services: Arc<Services>) -> Self {
        Self { services }
    }

    fn approval(&self, id: &str) -> Result<k01_approvals::ApprovalRow, VelaError> {
        let id = id.to_owned();
        self.services
            .store
            .read(|c| k01_approvals::get_request(c, &id))?
            .ok_or_else(|| {
                VelaError::of_class(
                    ErrorClass::ValidationFailure,
                    "there is no such approval request",
                )
            })
    }

    fn insert(
        &self,
        project_id: &str,
        a: &k01_approvals::ApprovalRow,
        kind: &str,
        expires: Option<i64>,
    ) -> Result<RuleRow, VelaError> {
        let now = self.services.now_ms();
        let rule = RuleRow {
            id: format!("rule-{}", self.services.ids.new_token()),
            project_id: project_id.to_owned(),
            kind: kind.to_owned(),
            class: a.class.clone(),
            executable: a.executable.clone(),
            scope_text: scope_text(&a.class, &a.executable, kind),
            source_approval: Some(a.id.clone()),
            created_ms: now,
            expires_ms: expires,
            revoked_ms: None,
        };
        let r = rule.clone();
        self.services
            .store
            .write(move |tx| k01_approvals::insert_rule(tx.conn(), &r))?;
        Ok(rule)
    }

    /// The scope text for an approval, for the UI to show before the user confirms.
    pub fn preview(&self, approval_id: &str, kind: &str) -> Result<String, VelaError> {
        let a = self.approval(approval_id)?;
        Ok(scope_text(&a.class, &a.executable, kind))
    }

    /// Allow once: a rule that expires. Created from an `ASK` that is waiting on the user.
    pub fn allow_once(&self, project_id: &str, approval_id: &str) -> Result<RuleRow, VelaError> {
        let a = self.approval(approval_id)?;
        if a.state != "ASKED" {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "only a request that is waiting for a decision can be allowed once",
            ));
        }
        if a.decision.as_deref() == Some("DENY") {
            return Err(VelaError::of_class(
                ErrorClass::PolicyBlock,
                "a denied operation cannot be allowed",
            ));
        }
        let expires = self.services.now_ms() + ONCE_TTL_MS;
        self.insert(project_id, &a, "ONCE", Some(expires))
    }

    /// A standing rule. `shown_scope` must be exactly the text [`RuleService::preview`] returned: the user confirms what they saw.
    pub fn create_permanent(
        &self,
        project_id: &str,
        approval_id: &str,
        shown_scope: &str,
    ) -> Result<RuleRow, VelaError> {
        let a = self.approval(approval_id)?;
        let expected = scope_text(&a.class, &a.executable, "PERMANENT");
        if shown_scope != expected {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "the confirmed scope is not the scope of this request",
            )
            .with_user_action("review the scope again and confirm it"));
        }
        if a.state != "ASKED" {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "a standing rule is created from a request that is waiting for a decision",
            ));
        }
        if NEVER_RULE_CLASSES.contains(&a.class.as_str()) {
            return Err(VelaError::of_class(
                ErrorClass::PolicyBlock,
                format!(
                    "{} operations can never be covered by a standing rule",
                    a.class
                ),
            ));
        }
        self.insert(project_id, &a, "PERMANENT", None)
    }

    /// Rules in force now.
    pub fn active(&self, project_id: &str) -> Result<Vec<RuleRow>, VelaError> {
        let (p, now) = (project_id.to_owned(), self.services.now_ms());
        self.services
            .store
            .read(|c| k01_approvals::active_rules(c, &p, now))
    }

    /// The rule that covers `class` and `executable`, if one is in force.
    pub fn matching(
        &self,
        project_id: &str,
        class: &str,
        executable: &str,
    ) -> Result<Option<RuleRow>, VelaError> {
        Ok(self
            .active(project_id)?
            .into_iter()
            .find(|r| r.class == class && r.executable == executable))
    }

    /// Revokes a rule. Revoking an unknown or already revoked rule is an error.
    pub fn revoke(&self, rule_id: &str) -> Result<(), VelaError> {
        let (id, now) = (rule_id.to_owned(), self.services.now_ms());
        if self
            .services
            .store
            .write(move |tx| k01_approvals::revoke_rule(tx.conn(), &id, now))?
        {
            Ok(())
        } else {
            Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "there is no such active rule",
            ))
        }
    }
}
