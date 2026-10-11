//! The central redaction layer (F07; component C17, `OBSERVABILITY.md` "Redaction").
//!
//! Everything Vela persists or shows that did not originate inside Vela passes through one [`Redactor`] first: process
//! output ([`StreamRedactor`]), log records ([`RedactingMakeWriter`], [`json_layer`]) and journal payloads (serialized
//! JSON given to [`Redactor::redact`]). Redaction happens at the ingestion point, so nothing downstream ever holds the
//! unredacted text.
//!
//! The [`Redactor`] is a trait object ([`SharedRedactor`]) so the composition root builds one instance and injects it
//! into the process runner, the diagnostics bundle and the Orchestrator journal path. [`PatternRedactor`] is the
//! production implementation; it covers tokens, authorization headers, credential-like environment values, cookies
//! and known secret patterns.
//!
//! Redaction is deliberately biased: where the end of a secret cannot be known the redactor removes more rather than
//! less. It is a safety net, not a guarantee; callers still keep secrets out of arguments and messages
//! (`SECURITY_AND_PERMISSIONS.md` "Secret handling"). Decisions the specification leaves open are recorded in
//! `docs/project/F07_DECISION_REQUESTS.md`.

#![warn(missing_docs)]

mod json;
mod log;
mod names;
mod pattern;
mod stream;

#[cfg(test)]
mod corpus;

use std::borrow::Cow;
use std::sync::Arc;

pub use log::{RedactingMakeWriter, RedactingWriter, json_layer};
pub use names::is_credential_name;
pub use pattern::{MIN_SECRET_LEN, PatternRedactor};
pub use stream::{DEFAULT_MAX_LINE_BYTES, StreamRedactor, TRUNCATED};

/// What a redacted value is replaced with.
pub const REDACTED: &str = "[REDACTED]";

/// Removes secrets from text before it is logged, journaled, streamed or exported.
///
/// Implementations are pure and idempotent: redacting already redacted text changes nothing.
pub trait Redactor: Send + Sync {
    /// Redacts free text: one line, many lines, or a serialized JSON document (whose structure is preserved).
    fn redact<'a>(&self, text: &'a str) -> Cow<'a, str>;

    /// Redacts a value whose name is known (an environment variable, a header, a JSON member). A credential-like
    /// `name` redacts the whole value; any other value is redacted as free text.
    fn redact_value<'a>(&self, name: &str, value: &'a str) -> Cow<'a, str>;
}

/// The injectable form of a [`Redactor`].
pub type SharedRedactor = Arc<dyn Redactor>;
