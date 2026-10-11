//! Logging setup and diagnostics (F07; component C17, `OBSERVABILITY.md`).
//!
//! Three pieces of the logging setup live here, because they are the ones the core owns:
//!
//! - **Correlation ids** ([`CorrelationIds`]): the run, ticket, worker and operation a piece of work belongs to,
//!   carried by a `tracing` span so that every record emitted inside it, in any crate and on any task, has them.
//! - **Log files** ([`LogFile`], [`RotationLimits`], [`LogStream`]): size-capped rotation, so the logs of each stream
//!   never occupy more than a configured number of bytes.
//! - **Journal payload redaction** ([`redact_payload`], [`RedactionPort`]): every string of a journal payload passes
//!   through the redactor before the payload reaches the persistence writer.
//!
//! The redactor itself and the JSON log layer are `vela-process::redact`. `vela-core` does not depend on
//! `vela-process` (`IMPLEMENTATION_ARCHITECTURE.md` section 3, rule 3), so the composition root joins the two:
//!
//! ```text
//! let redactor: SharedRedactor = Arc::new(PatternRedactor::new());
//! let file = LogFile::open(log_dir, LogStream::Core, RotationLimits::default())?;
//! let layer = vela_process::redact::json_layer(move || file.clone(), redactor.clone());
//! tracing::subscriber::set_global_default(tracing_subscriber::registry().with(layer))?;
//! ```
//!
//! and hands the same redactor to the core behind [`RedactionPort`]. Why the port exists, and the other decisions the
//! specification leaves open, are recorded in `docs/project/F07_DECISION_REQUESTS.md`.

#![warn(missing_docs)]

mod correlation;
mod payload;
mod rotation;

pub use correlation::{
    CORRELATION_SPAN, CorrelationIds, OPERATION_ID_FIELD, RUN_ID_FIELD, TICKET_ID_FIELD,
    WORKER_ID_FIELD, spawn_correlated,
};
pub use payload::{RedactionPort, SharedRedaction, redact_payload, redact_payload_text};
pub use rotation::{LogFile, LogStream, RotationLimits};
