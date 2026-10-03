//! Typed errors (F02; `docs/operations/ERROR_HANDLING.md`).
//!
//! Every error carries one of eight classes. Specific typed errors (`ErrorCode`) map onto a class, and every surfaced
//! `VelaError` answers the five questions of `ERROR_HANDLING.md`: what failed, what state is preserved, whether anything
//! was committed, pushed or merged, whether Vela will retry, and what the user can do.
//!
//! The code-to-class mapping (`ErrorCode::class`) is the F02 interpretation of "each maps to the typed classes above":
//! it is exhaustive (a new code does not compile until it has a class) and covered by tests.

use std::fmt;

use serde::{Deserialize, Serialize};

/// The eight error classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "full", derive(ts_rs::TS))]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorClass {
    /// The user must act (decide, install, fix configuration) before work can continue.
    UserActionRequired,
    /// A transient external condition; bounded retry with backoff and jitter is appropriate.
    ExternalTemporary,
    /// Authentication with an external service is missing or expired.
    ExternalAuth,
    /// A policy forbids the action; never retried and never worked around.
    PolicyBlock,
    /// A repository-level conflict (for example a textual merge conflict).
    RepositoryConflict,
    /// An input, artifact or gate failed validation.
    ValidationFailure,
    /// The coding agent failed to do the work.
    AgentFailure,
    /// A defect in Vela or in its stored state.
    InternalBug,
}

impl ErrorClass {
    /// Every class, in specification order.
    pub const ALL: [Self; 8] = [
        Self::UserActionRequired,
        Self::ExternalTemporary,
        Self::ExternalAuth,
        Self::PolicyBlock,
        Self::RepositoryConflict,
        Self::ValidationFailure,
        Self::AgentFailure,
        Self::InternalBug,
    ];

    /// Only transient external failures are candidates for automatic retry (and then only for idempotent actions).
    pub fn is_retryable(self) -> bool {
        self == Self::ExternalTemporary
    }
}

/// Who the error originates from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "full", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ErrorOrigin {
    /// Detected by Vela itself.
    Vela,
    /// Reported by the coding-agent provider (provider-neutral: no provider is named).
    Provider,
}

/// Whether Vela will retry the failed action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "full", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum RetryDisposition {
    /// Vela will not retry, ever.
    Never,
    /// Vela retries with bounded exponential backoff and jitter.
    Bounded,
}

/// Specific typed errors. The wire form is the specification's `SCREAMING_SNAKE_CASE` name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "full", derive(ts_rs::TS))]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    /// No approval surface was found for the request.
    ApprovalSurfaceNotFound,
    /// More than one candidate approval surface matched.
    ApprovalTargetAmbiguous,
    /// No policy decision exists for the approval request (it defaults to ASK).
    ApprovalPolicyUnknown,
    /// Delivery of an approval decision failed.
    ApprovalDeliveryFailed,
    /// The same approval fingerprint repeated without progress.
    ApprovalStalled,
    /// The foreground window does not match the correlated session.
    ApprovalWindowMismatch,
    /// The interactive session is unavailable (locked, disconnected, secure desktop). Not a failure.
    ApprovalSurfaceUnavailable,
    /// No permitted delivery tier can deliver a decision.
    ApprovalUndeliverable,
    /// A DENY was repeated or is required by the ticket.
    PolicyDenied,
    /// A detective control fired.
    PolicyViolation,
    /// Build or repository-controlled execution was attempted in an untrusted repository.
    TrustRequired,
    /// A required provider capability is absent.
    CapabilityMissing,
    /// The native permission posture check is `DOES_NOT_MEET` or `UNKNOWN`.
    PostureNotMet,
    /// The state store is corrupt.
    StateStoreCorrupt,
    /// The state store is corrupt and cannot be recovered.
    StateStoreUnrecoverable,
    /// A schema migration failed.
    MigrationFailed,
    /// The provider indicates the account or service is blocked, suspended, disabled or denied for terms or policy
    /// reasons. Class `POLICY_BLOCK`, origin provider. Never retried, never worked around, never cleared automatically.
    ProviderPolicyBlock,
    /// Provider authentication is required (determined from error text and the capability probe, not the exit code).
    AuthRequired,
    /// The ticket graph is invalid (for example it contains a cycle).
    GraphInvalid,
}

impl ErrorCode {
    /// Every code.
    pub const ALL: [Self; 19] = [
        Self::ApprovalSurfaceNotFound,
        Self::ApprovalTargetAmbiguous,
        Self::ApprovalPolicyUnknown,
        Self::ApprovalDeliveryFailed,
        Self::ApprovalStalled,
        Self::ApprovalWindowMismatch,
        Self::ApprovalSurfaceUnavailable,
        Self::ApprovalUndeliverable,
        Self::PolicyDenied,
        Self::PolicyViolation,
        Self::TrustRequired,
        Self::CapabilityMissing,
        Self::PostureNotMet,
        Self::StateStoreCorrupt,
        Self::StateStoreUnrecoverable,
        Self::MigrationFailed,
        Self::ProviderPolicyBlock,
        Self::AuthRequired,
        Self::GraphInvalid,
    ];

    /// The class this code belongs to.
    pub fn class(self) -> ErrorClass {
        match self {
            Self::ApprovalSurfaceNotFound
            | Self::ApprovalTargetAmbiguous
            | Self::ApprovalDeliveryFailed
            | Self::ApprovalWindowMismatch
            | Self::ApprovalSurfaceUnavailable => ErrorClass::ExternalTemporary,
            Self::ApprovalPolicyUnknown
            | Self::ApprovalStalled
            | Self::ApprovalUndeliverable
            | Self::TrustRequired
            | Self::CapabilityMissing
            | Self::PostureNotMet
            | Self::StateStoreUnrecoverable => ErrorClass::UserActionRequired,
            Self::PolicyDenied | Self::PolicyViolation | Self::ProviderPolicyBlock => {
                ErrorClass::PolicyBlock
            }
            Self::StateStoreCorrupt | Self::MigrationFailed => ErrorClass::InternalBug,
            Self::AuthRequired => ErrorClass::ExternalAuth,
            Self::GraphInvalid => ErrorClass::ValidationFailure,
        }
    }

    /// Where the error originates. Only a provider policy block originates from the provider.
    pub fn origin(self) -> ErrorOrigin {
        match self {
            Self::ProviderPolicyBlock => ErrorOrigin::Provider,
            _ => ErrorOrigin::Vela,
        }
    }

    /// Whether Vela retries. Approval surface problems are never retried by clicking again (`ERROR_HANDLING.md`), and a
    /// provider policy block is never retried, whatever its class says.
    pub fn retry(self) -> RetryDisposition {
        match self {
            Self::ApprovalSurfaceNotFound
            | Self::ApprovalTargetAmbiguous
            | Self::ApprovalWindowMismatch
            | Self::ProviderPolicyBlock => RetryDisposition::Never,
            _ if self.class().is_retryable() => RetryDisposition::Bounded,
            _ => RetryDisposition::Never,
        }
    }
}

/// What a failed action had already done to the repository.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "full", derive(ts_rs::TS))]
pub struct SideEffects {
    /// A commit was created.
    pub committed: bool,
    /// Something was pushed to a remote.
    pub pushed: bool,
    /// A merge was completed.
    pub merged: bool,
}

/// The typed error every port returns. It answers the five questions of `ERROR_HANDLING.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "full", derive(ts_rs::TS))]
pub struct VelaError {
    /// The error class.
    pub class: ErrorClass,
    /// The specific typed error, when there is one.
    pub code: Option<ErrorCode>,
    /// Who the error originates from.
    pub origin: ErrorOrigin,
    /// 1. What failed.
    pub what_failed: String,
    /// 2. What state is preserved.
    pub state_preserved: String,
    /// 3. Whether anything was committed, pushed or merged.
    pub side_effects: SideEffects,
    /// 4. Whether Vela will retry.
    pub retry: RetryDisposition,
    /// 5. What the user can do.
    pub user_action: String,
}

impl VelaError {
    /// An error of a specific code; class, origin and retry follow from the code.
    pub fn from_code(code: ErrorCode, what_failed: impl Into<String>) -> Self {
        Self {
            class: code.class(),
            code: Some(code),
            origin: code.origin(),
            what_failed: what_failed.into(),
            state_preserved: String::new(),
            side_effects: SideEffects::default(),
            retry: code.retry(),
            user_action: String::new(),
        }
    }

    /// An error that has a class but no more specific code.
    pub fn of_class(class: ErrorClass, what_failed: impl Into<String>) -> Self {
        Self {
            class,
            code: None,
            origin: ErrorOrigin::Vela,
            what_failed: what_failed.into(),
            state_preserved: String::new(),
            side_effects: SideEffects::default(),
            retry: if class.is_retryable() {
                RetryDisposition::Bounded
            } else {
                RetryDisposition::Never
            },
            user_action: String::new(),
        }
    }

    /// An internal-bug error, for states that should be unreachable.
    pub fn internal(what_failed: impl Into<String>) -> Self {
        Self::of_class(ErrorClass::InternalBug, what_failed)
    }

    /// Records what state is preserved (answer 2).
    #[must_use]
    pub fn with_state_preserved(mut self, state: impl Into<String>) -> Self {
        self.state_preserved = state.into();
        self
    }

    /// Records what had already been committed, pushed or merged (answer 3).
    #[must_use]
    pub fn with_side_effects(mut self, side_effects: SideEffects) -> Self {
        self.side_effects = side_effects;
        self
    }

    /// Records what the user can do (answer 5).
    #[must_use]
    pub fn with_user_action(mut self, action: impl Into<String>) -> Self {
        self.user_action = action.into();
        self
    }
}

impl fmt::Display for VelaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.code {
            Some(code) => write!(f, "{code:?} ({:?}): {}", self.class, self.what_failed),
            None => write!(f, "{:?}: {}", self.class, self.what_failed),
        }
    }
}

impl std::error::Error for VelaError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_maps_to_a_class_and_the_mapping_is_the_documented_one() {
        use ErrorClass::*;
        use ErrorCode::*;
        let expected = [
            (ApprovalSurfaceNotFound, ExternalTemporary),
            (ApprovalTargetAmbiguous, ExternalTemporary),
            (ApprovalPolicyUnknown, UserActionRequired),
            (ApprovalDeliveryFailed, ExternalTemporary),
            (ApprovalStalled, UserActionRequired),
            (ApprovalWindowMismatch, ExternalTemporary),
            (ApprovalSurfaceUnavailable, ExternalTemporary),
            (ApprovalUndeliverable, UserActionRequired),
            (PolicyDenied, PolicyBlock),
            (PolicyViolation, PolicyBlock),
            (TrustRequired, UserActionRequired),
            (CapabilityMissing, UserActionRequired),
            (PostureNotMet, UserActionRequired),
            (StateStoreCorrupt, InternalBug),
            (StateStoreUnrecoverable, UserActionRequired),
            (MigrationFailed, InternalBug),
            (ProviderPolicyBlock, PolicyBlock),
            (AuthRequired, ExternalAuth),
            (GraphInvalid, ValidationFailure),
        ];
        assert_eq!(expected.len(), ErrorCode::ALL.len());
        for (code, class) in expected {
            assert_eq!(code.class(), class, "{code:?}");
            assert!(ErrorCode::ALL.contains(&code));
        }
    }

    #[test]
    fn provider_policy_block_is_a_provider_originated_policy_block_that_is_never_retried() {
        let code = ErrorCode::ProviderPolicyBlock;
        assert_eq!(code.class(), ErrorClass::PolicyBlock);
        assert_eq!(code.origin(), ErrorOrigin::Provider);
        assert_eq!(code.retry(), RetryDisposition::Never);
        let err = VelaError::from_code(code, "account blocked");
        assert_eq!(err.class, ErrorClass::PolicyBlock);
        assert_eq!(err.origin, ErrorOrigin::Provider);
        assert_eq!(err.retry, RetryDisposition::Never);
        for other in ErrorCode::ALL {
            if other != code {
                assert_eq!(other.origin(), ErrorOrigin::Vela, "{other:?}");
            }
        }
    }

    #[test]
    fn only_transient_external_classes_retry_and_only_when_the_code_allows_it() {
        for class in ErrorClass::ALL {
            assert_eq!(class.is_retryable(), class == ErrorClass::ExternalTemporary);
        }
        assert_eq!(
            VelaError::of_class(ErrorClass::ExternalTemporary, "x").retry,
            RetryDisposition::Bounded
        );
        assert_eq!(
            VelaError::of_class(ErrorClass::InternalBug, "x").retry,
            RetryDisposition::Never
        );
        assert_eq!(
            ErrorCode::ApprovalDeliveryFailed.retry(),
            RetryDisposition::Bounded
        );
        assert_eq!(
            ErrorCode::ApprovalTargetAmbiguous.retry(),
            RetryDisposition::Never
        );
    }

    #[test]
    fn errors_answer_the_five_questions() {
        let err = VelaError::from_code(ErrorCode::TrustRequired, "build refused")
            .with_state_preserved("worker paused, worktree untouched")
            .with_side_effects(SideEffects::default())
            .with_user_action("trust the project");
        assert_eq!(err.what_failed, "build refused");
        assert_eq!(err.state_preserved, "worker paused, worktree untouched");
        assert_eq!(err.side_effects, SideEffects::default());
        assert_eq!(err.retry, RetryDisposition::Never);
        assert_eq!(err.user_action, "trust the project");
        assert!(err.to_string().contains("TrustRequired"));
    }

    #[cfg(feature = "full")]
    #[test]
    fn wire_names_are_the_specification_names() {
        let wire = |code: ErrorCode| serde_json::to_string(&code).unwrap();
        assert_eq!(
            wire(ErrorCode::ProviderPolicyBlock),
            "\"PROVIDER_POLICY_BLOCK\""
        );
        assert_eq!(
            wire(ErrorCode::ApprovalSurfaceUnavailable),
            "\"APPROVAL_SURFACE_UNAVAILABLE\""
        );
        assert_eq!(
            serde_json::to_string(&ErrorClass::UserActionRequired).unwrap(),
            "\"USER_ACTION_REQUIRED\""
        );
        let err = VelaError::from_code(ErrorCode::PolicyViolation, "hook fired");
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(serde_json::from_str::<VelaError>(&json).unwrap(), err);
    }

    /// Every typed name that `ERROR_HANDLING.md` lists (before its result-interpretation section) is a class or a code.
    #[cfg(feature = "full")]
    #[test]
    fn every_error_name_in_the_specification_is_modelled() {
        const SPEC: &str = include_str!("../../../../docs/operations/ERROR_HANDLING.md");
        let typed = SPEC
            .split("# Antigravity Result Interpretation")
            .next()
            .unwrap();
        let mut seen = 0;
        for token in typed.split('`').skip(1).step_by(2) {
            let is_name = token.len() > 3
                && token.chars().all(|c| c.is_ascii_uppercase() || c == '_')
                && token.contains('_');
            // `DOES_NOT_MEET` is a posture verdict quoted in a description, not an error name.
            if !is_name || token == "DOES_NOT_MEET" {
                continue;
            }
            seen += 1;
            let wire = format!("\"{token}\"");
            let as_class = serde_json::from_str::<ErrorClass>(&wire).is_ok();
            let as_code = serde_json::from_str::<ErrorCode>(&wire).is_ok();
            assert!(as_class || as_code, "{token} is not modelled");
        }
        assert!(
            seen >= 18,
            "expected to find the specified names, saw {seen}"
        );
    }
}
