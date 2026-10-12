//! Review domain (D06): finding identity, the Engineering exit policy, review stages and cycle rules, and the
//! verification outcome (ADR-022, `REVIEW_PROTOCOL.md`, `ORCHESTRATION_ENGINE.md` section 4).
//!
//! There is exactly one comprehensive review per ticket, then targeted verification of the fixes, then a human if
//! anything blocking remains. Everything here is pure: no I/O, no clock, no provider-specific concept (ADR-006). No
//! function in this module can return success while a blocking finding is unresolved, and no cause (timeout, default,
//! human decision) turns an unresolved blocking finding into a pass.
//!
//! Interpretations the specification leaves open are recorded in `docs/project/D06_DECISION_REQUESTS.md`.

#![warn(missing_docs)]

mod cycle;
mod finding;
mod fingerprint;
mod verification;

pub use cycle::{CycleStage, FixSetKind, ReviewError, ReviewHistory, StartOrigin};
pub use finding::{Disposition, DispositionError, Finding, ReviewAxis, ReviewCategory};
pub use fingerprint::{FindingId, FindingIdError, fingerprint};
pub use verification::{
    Escalation, ReviewOutcome, StallReason, Verdict, VerdictKind, VerificationInput,
    evaluate_verification,
};
