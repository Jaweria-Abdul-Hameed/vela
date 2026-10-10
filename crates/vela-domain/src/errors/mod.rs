//! Typed errors (F02; `docs/operations/ERROR_HANDLING.md`).
//!
//! Every error carries one of eight classes. Specific typed errors (`ErrorCode`) map onto a class, and every surfaced
//! `VelaError` answers the five questions of `ERROR_HANDLING.md`: what failed, what state is preserved, whether anything
//! was committed, pushed or merged, whether Vela will retry, and what the user can do.
//!
//! The code-to-class mapping (`ErrorCode::class`) is the F02 interpretation of "each maps to the typed classes above":
//! it is exhaustive (a new code does not compile until it has a class) and covered by tests.

#![warn(missing_docs)]

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

    /// The specification name (also the wire form).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UserActionRequired => "USER_ACTION_REQUIRED",
            Self::ExternalTemporary => "EXTERNAL_TEMPORARY",
            Self::ExternalAuth => "EXTERNAL_AUTH",
            Self::PolicyBlock => "POLICY_BLOCK",
            Self::RepositoryConflict => "REPOSITORY_CONFLICT",
            Self::ValidationFailure => "VALIDATION_FAILURE",
            Self::AgentFailure => "AGENT_FAILURE",
            Self::InternalBug => "INTERNAL_BUG",
        }
    }

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

    /// The specification name (also the wire form).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ApprovalSurfaceNotFound => "APPROVAL_SURFACE_NOT_FOUND",
            Self::ApprovalTargetAmbiguous => "APPROVAL_TARGET_AMBIGUOUS",
            Self::ApprovalPolicyUnknown => "APPROVAL_POLICY_UNKNOWN",
            Self::ApprovalDeliveryFailed => "APPROVAL_DELIVERY_FAILED",
            Self::ApprovalStalled => "APPROVAL_STALLED",
            Self::ApprovalWindowMismatch => "APPROVAL_WINDOW_MISMATCH",
            Self::ApprovalSurfaceUnavailable => "APPROVAL_SURFACE_UNAVAILABLE",
            Self::ApprovalUndeliverable => "APPROVAL_UNDELIVERABLE",
            Self::PolicyDenied => "POLICY_DENIED",
            Self::PolicyViolation => "POLICY_VIOLATION",
            Self::TrustRequired => "TRUST_REQUIRED",
            Self::CapabilityMissing => "CAPABILITY_MISSING",
            Self::PostureNotMet => "POSTURE_NOT_MET",
            Self::StateStoreCorrupt => "STATE_STORE_CORRUPT",
            Self::StateStoreUnrecoverable => "STATE_STORE_UNRECOVERABLE",
            Self::MigrationFailed => "MIGRATION_FAILED",
            Self::ProviderPolicyBlock => "PROVIDER_POLICY_BLOCK",
            Self::AuthRequired => "AUTH_REQUIRED",
            Self::GraphInvalid => "GRAPH_INVALID",
        }
    }

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

    /// Whether Vela retries. Every approval error is never retried by this stored disposition (`ERROR_HANDLING.md`: never
    /// respond to approval errors by clicking repeatedly; a surface that is unavailable is reconciled on return), and a
    /// provider policy block is never retried whatever its class says. No other code is retried either, so `Bounded` is
    /// currently reachable only through [`VelaError::of_class`] with `EXTERNAL_TEMPORARY`. Whether the approval codes
    /// should allow *bounded re-detection* (the document also speaks of "bounded recovery") is an open specification
    /// question, recorded as DR-F02-3; the conservative reading is implemented.
    pub fn retry(self) -> RetryDisposition {
        RetryDisposition::Never
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
    /// The action may have taken effect but Vela cannot tell (for example a timeout or a crash after the request was
    /// sent). Such an outcome must be reconciled against the real state before anything is retried.
    pub uncertain: bool,
}

impl SideEffects {
    /// True when the failed action committed, pushed or merged, or when its outcome is uncertain.
    pub fn any(self) -> bool {
        self.committed || self.pushed || self.merged || self.uncertain
    }
}

/// The typed error every port returns. It answers the five questions of `ERROR_HANDLING.md`.
///
/// Values are internally consistent: built through [`VelaError::from_code`] or [`VelaError::of_class`] (the fields are
/// private) and re-checked when deserialized, so a journaled error cannot claim, for example, to be a retryable
/// transient Vela error while carrying the `PROVIDER_POLICY_BLOCK` code. The rules ([`VelaError::validate`]): with a
/// code, `class` and `origin` are the code's; without one the origin is Vela; `retry` is `Bounded` only for a retryable
/// class whose code allows it and when the action had no side effects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "full", derive(ts_rs::TS))]
pub struct VelaError {
    /// The error class.
    class: ErrorClass,
    /// The specific typed error, when there is one.
    code: Option<ErrorCode>,
    /// Who the error originates from.
    origin: ErrorOrigin,
    /// 1. What failed.
    what_failed: String,
    /// 2. What state is preserved.
    state_preserved: String,
    /// 3. Whether anything was committed, pushed or merged.
    side_effects: SideEffects,
    /// 4. Whether Vela will retry.
    retry: RetryDisposition,
    /// 5. What the user can do.
    user_action: String,
}

/// Why a stored [`VelaError`] was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InconsistentError(pub &'static str);

impl fmt::Display for InconsistentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for InconsistentError {}

impl<'de> Deserialize<'de> for VelaError {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            class: ErrorClass,
            code: Option<ErrorCode>,
            origin: ErrorOrigin,
            what_failed: String,
            state_preserved: String,
            side_effects: SideEffects,
            retry: RetryDisposition,
            user_action: String,
        }
        let raw = Raw::deserialize(deserializer)?;
        let error = VelaError {
            class: raw.class,
            code: raw.code,
            origin: raw.origin,
            what_failed: raw.what_failed,
            state_preserved: raw.state_preserved,
            side_effects: raw.side_effects,
            retry: raw.retry,
            user_action: raw.user_action,
        };
        error.validate().map_err(serde::de::Error::custom)?;
        Ok(error)
    }
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

    /// The error class.
    pub fn class(&self) -> ErrorClass {
        self.class
    }

    /// The specific typed error, when there is one.
    pub fn code(&self) -> Option<ErrorCode> {
        self.code
    }

    /// Who the error originates from.
    pub fn origin(&self) -> ErrorOrigin {
        self.origin
    }

    /// 1. What failed.
    pub fn what_failed(&self) -> &str {
        &self.what_failed
    }

    /// 2. What state is preserved.
    pub fn state_preserved(&self) -> &str {
        &self.state_preserved
    }

    /// 3. What had already been committed, pushed or merged.
    pub fn side_effects(&self) -> SideEffects {
        self.side_effects
    }

    /// 4. The stored retry disposition. Retry loops use [`VelaError::may_retry`].
    pub fn retry(&self) -> RetryDisposition {
        self.retry
    }

    /// 5. What the user can do.
    pub fn user_action(&self) -> &str {
        &self.user_action
    }

    /// Checks the consistency rules; used when reading a stored error.
    pub fn validate(&self) -> Result<(), InconsistentError> {
        match self.code {
            Some(code) => {
                if self.class != code.class() {
                    return Err(InconsistentError("the class does not match the error code"));
                }
                if self.origin != code.origin() {
                    return Err(InconsistentError("the origin does not match the error code"));
                }
                if self.retry == RetryDisposition::Bounded && code.retry() != RetryDisposition::Bounded {
                    return Err(InconsistentError("this error code is never retried"));
                }
            }
            None => {
                if self.origin != ErrorOrigin::Vela {
                    return Err(InconsistentError(
                        "only a provider policy block originates from the provider",
                    ));
                }
            }
        }
        if self.retry == RetryDisposition::Bounded
            && (!self.class.is_retryable() || self.side_effects.any())
        {
            return Err(InconsistentError(
                "a retry is only allowed for a transient class and an action without side effects",
            ));
        }
        Ok(())
    }

    /// True when the failed action may already have had an effect (it committed, pushed or merged, or its outcome is
    /// uncertain). The state must then be reconciled against Git and the journal; the action is never blindly repeated.
    pub fn requires_reconciliation(&self) -> bool {
        self.side_effects.any()
    }

    /// Whether Vela may retry, derived from the invariants rather than from the stored fields: a provider policy block,
    /// a provider-originated error, any error with a code that never retries, every class except `EXTERNAL_TEMPORARY`,
    /// and any error whose action had side effects or an uncertain outcome are never blindly retried, even if a
    /// deserialized or hand-built value says otherwise ("never retry destructive or non-idempotent actions blindly").
    /// Retry loops must use this, not the `retry` field.
    pub fn may_retry(&self) -> bool {
        self.retry == RetryDisposition::Bounded
            && !self.requires_reconciliation()
            && self.class.is_retryable()
            && self.origin == ErrorOrigin::Vela
            && self
                .code
                .is_none_or(|code| code.retry() == RetryDisposition::Bounded)
    }

    /// Records what state is preserved (answer 2).
    #[must_use]
    pub fn with_state_preserved(mut self, state: impl Into<String>) -> Self {
        self.state_preserved = state.into();
        self
    }

    /// Records what had already been committed, pushed or merged, or that the outcome is uncertain (answer 3). Any
    /// recorded effect also sets the stored retry disposition to `Never`.
    #[must_use]
    pub fn with_side_effects(mut self, side_effects: SideEffects) -> Self {
        self.side_effects = side_effects;
        if side_effects.any() {
            self.retry = RetryDisposition::Never;
        }
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
            Some(code) => write!(
                f,
                "{} ({}): {}",
                code.as_str(),
                self.class.as_str(),
                self.what_failed
            ),
            None => write!(f, "{}: {}", self.class.as_str(), self.what_failed),
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
        for code in ErrorCode::ALL {
            assert_eq!(code.retry(), RetryDisposition::Never, "{code:?}");
            assert!(!VelaError::from_code(code, "x").may_retry(), "{code:?}");
        }
        assert!(VelaError::of_class(ErrorClass::ExternalTemporary, "x").may_retry());
        assert!(!VelaError::of_class(ErrorClass::InternalBug, "x").may_retry());
    }

    #[test]
    fn may_retry_ignores_inconsistent_stored_fields() {
        // A hand-built or deserialized value that claims a provider policy block may be retried is still never retried.
        let mut forged = VelaError::from_code(ErrorCode::ProviderPolicyBlock, "blocked");
        forged.class = ErrorClass::ExternalTemporary;
        forged.origin = ErrorOrigin::Vela;
        forged.retry = RetryDisposition::Bounded;
        assert!(!forged.may_retry());
        let mut provider = VelaError::of_class(ErrorClass::ExternalTemporary, "x");
        provider.origin = ErrorOrigin::Provider;
        assert!(!provider.may_retry());
    }

    #[test]
    fn an_error_after_a_side_effect_or_with_an_uncertain_outcome_is_never_blindly_retried() {
        let effects = [
            SideEffects {
                committed: true,
                ..SideEffects::default()
            },
            SideEffects {
                pushed: true,
                ..SideEffects::default()
            },
            SideEffects {
                merged: true,
                ..SideEffects::default()
            },
            SideEffects {
                uncertain: true,
                ..SideEffects::default()
            },
        ];
        for side_effects in effects {
            assert!(side_effects.any());
            let transient =
                VelaError::of_class(ErrorClass::ExternalTemporary, "timeout after send");
            assert!(transient.may_retry());
            assert!(!transient.requires_reconciliation());

            let after = transient.with_side_effects(side_effects);
            assert!(after.requires_reconciliation());
            assert!(!after.may_retry(), "{side_effects:?}");
            assert_eq!(after.retry, RetryDisposition::Never);

            // A value whose stored fields claim otherwise (hand-built or read back) is still not retried.
            let mut forged = after.clone();
            forged.retry = RetryDisposition::Bounded;
            assert!(!forged.may_retry(), "{side_effects:?}");
        }
        assert!(!SideEffects::default().any());
        let clean = VelaError::of_class(ErrorClass::ExternalTemporary, "x")
            .with_side_effects(SideEffects::default());
        assert!(clean.may_retry());
    }

    #[test]
    fn every_constructible_error_is_consistent() {
        for code in ErrorCode::ALL {
            let error = VelaError::from_code(code, "x");
            assert_eq!(error.validate(), Ok(()), "{code:?}");
        }
        for class in ErrorClass::ALL {
            let error = VelaError::of_class(class, "x");
            assert_eq!(error.validate(), Ok(()), "{class:?}");
        }
        let with_effects = VelaError::of_class(ErrorClass::ExternalTemporary, "x").with_side_effects(
            SideEffects {
                pushed: true,
                ..SideEffects::default()
            },
        );
        assert_eq!(with_effects.validate(), Ok(()));
    }

    #[test]
    fn validation_rejects_every_inconsistent_combination() {
        let base = || VelaError::from_code(ErrorCode::ProviderPolicyBlock, "blocked");
        let mut wrong_class = base();
        wrong_class.class = ErrorClass::ExternalTemporary;
        assert!(wrong_class.validate().is_err());
        let mut wrong_origin = base();
        wrong_origin.origin = ErrorOrigin::Vela;
        assert!(wrong_origin.validate().is_err());
        let mut retried = base();
        retried.retry = RetryDisposition::Bounded;
        assert!(retried.validate().is_err());

        let mut provider_without_code = VelaError::of_class(ErrorClass::PolicyBlock, "x");
        provider_without_code.origin = ErrorOrigin::Provider;
        assert!(provider_without_code.validate().is_err());

        let mut retry_on_a_permanent_class = VelaError::of_class(ErrorClass::InternalBug, "x");
        retry_on_a_permanent_class.retry = RetryDisposition::Bounded;
        assert!(retry_on_a_permanent_class.validate().is_err());

        let mut retry_after_a_push = VelaError::of_class(ErrorClass::ExternalTemporary, "x");
        retry_after_a_push.side_effects.pushed = true;
        assert!(retry_after_a_push.validate().is_err());
    }

    #[cfg(feature = "full")]
    #[test]
    fn a_forged_stored_error_is_rejected_on_read_and_a_real_one_round_trips() {
        let forged = r#"{"class":"EXTERNAL_TEMPORARY","code":"PROVIDER_POLICY_BLOCK","origin":"vela","what_failed":"x","state_preserved":"","side_effects":{"committed":false,"pushed":false,"merged":false,"uncertain":false},"retry":"bounded","user_action":""}"#;
        assert!(serde_json::from_str::<VelaError>(forged).is_err());
        for code in ErrorCode::ALL {
            let error = VelaError::from_code(code, "x");
            let json = serde_json::to_string(&error).unwrap();
            assert_eq!(serde_json::from_str::<VelaError>(&json).unwrap(), error);
        }
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
        assert!(err.to_string().contains("TRUST_REQUIRED"));
        assert_eq!(
            err.to_string(),
            "TRUST_REQUIRED (USER_ACTION_REQUIRED): build refused"
        );
        assert_eq!(
            VelaError::of_class(ErrorClass::InternalBug, "oops").to_string(),
            "INTERNAL_BUG: oops"
        );
    }

    #[cfg(feature = "full")]
    #[test]
    fn wire_names_are_the_specification_names() {
        for class in ErrorClass::ALL {
            assert_eq!(
                serde_json::to_string(&class).unwrap(),
                format!("\"{}\"", class.as_str())
            );
        }
        for code in ErrorCode::ALL {
            assert_eq!(
                serde_json::to_string(&code).unwrap(),
                format!("\"{}\"", code.as_str())
            );
        }
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
