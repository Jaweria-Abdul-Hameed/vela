//! Redaction of journal payloads (`IMPLEMENTATION_ARCHITECTURE.md` section 8: "payloads are redacted before they
//! reach the writer thread").
//!
//! A payload is serialized and given to the redactor as one JSON document. The redactor walks that document
//! (`vela-process::redact`): every string is redacted, and what only the structure reveals is redacted with it
//! (members with credential-like names, name and value pairs, the element after a credential flag in an argument
//! list). There is one such walk, in the redaction layer; this module does not repeat it.
//!
//! Containers keep their shape and member order. A redacted value is always a string, so a number stored under a
//! credential-like name becomes the marker string, and a member name that is itself a secret is replaced. Typed
//! payloads therefore keep credentials in string fields.

use std::sync::Arc;

use serde_json::Value;

/// The redactor as the core sees it.
///
/// The production implementation is `vela-process::redact::PatternRedactor`, which the core cannot name
/// (`vela-core` does not depend on `vela-process`). The composition root adapts the one shared redactor to this port;
/// both methods map one to one onto `vela_process::redact::Redactor`.
pub trait RedactionPort: Send + Sync {
    /// Redacts free text. Text that is a serialized JSON document is redacted structurally and stays a JSON document
    /// of the same shape.
    fn redact_text(&self, text: &str) -> String;

    /// Redacts a value whose name is known. A credential-like `name` redacts the whole value.
    fn redact_value(&self, name: &str, value: &str) -> String;
}

/// The injectable form of a [`RedactionPort`].
pub type SharedRedaction = Arc<dyn RedactionPort>;

/// Redacts a journal payload in place. A payload with nothing to redact is left exactly as it was.
pub fn redact_payload(redactor: &dyn RedactionPort, payload: &mut Value) {
    let text = payload.to_string();
    let redacted = redactor.redact_text(&text);
    if redacted == text {
        return;
    }
    *payload = match serde_json::from_str(&redacted) {
        Ok(value) => value,
        // A redactor that did not keep the document well formed: journal what it returned, never the original.
        Err(_) => Value::String(redacted),
    };
}

/// Redacts a serialized journal payload. Text that is not JSON is redacted as free text, so nothing passes through
/// unredacted because it failed to parse; text with nothing to redact is returned unchanged.
pub fn redact_payload_text(redactor: &dyn RedactionPort, payload: &str) -> String {
    redactor.redact_text(payload)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use serde_json::json;

    use super::*;

    /// A stand-in for the production redactor: `SECRET` is the known secret pattern. It counts what it is given.
    #[derive(Default)]
    struct Fake {
        documents: AtomicUsize,
    }

    impl RedactionPort for Fake {
        fn redact_text(&self, text: &str) -> String {
            self.documents.fetch_add(1, Ordering::Relaxed);
            text.replace("SECRET", "[REDACTED]")
        }

        fn redact_value(&self, name: &str, value: &str) -> String {
            if name.to_ascii_lowercase().contains("password") {
                "[REDACTED]".to_owned()
            } else {
                self.redact_text(value)
            }
        }
    }

    /// A redactor that damages the document it is given.
    struct Breaking;

    impl RedactionPort for Breaking {
        fn redact_text(&self, text: &str) -> String {
            text.replace("\"SECRET\"", "gone")
        }

        fn redact_value(&self, _: &str, value: &str) -> String {
            self.redact_text(value)
        }
    }

    #[test]
    fn the_whole_payload_passes_through_the_redactor_as_one_document() {
        let fake = Fake::default();
        let mut payload = json!({
            "kind": "CommandFinished",
            "command": "deploy --key SECRET",
            "exit_code": 0,
            "ok": true,
            "nothing": null,
            "env": { "PATH": "/usr/bin", "KEY": "SECRET" },
            "lines": ["plain", "has SECRET inside", ["nested SECRET"]],
            "SECRET-named": { "deep": [{ "note": "SECRET" }] },
        });
        redact_payload(&fake, &mut payload);
        assert_eq!(
            payload,
            json!({
                "kind": "CommandFinished",
                "command": "deploy --key [REDACTED]",
                "exit_code": 0,
                "ok": true,
                "nothing": null,
                "env": { "PATH": "/usr/bin", "KEY": "[REDACTED]" },
                "lines": ["plain", "has [REDACTED] inside", ["nested [REDACTED]"]],
                "[REDACTED]-named": { "deep": [{ "note": "[REDACTED]" }] },
            })
        );
        assert_eq!(fake.documents.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn redaction_is_idempotent_and_keeps_clean_payloads_identical() {
        let fake = Fake::default();
        let clean =
            json!({ "run_id": "run-1", "seq": 7, "items": [1, 2.5, "text"], "flag": false });
        let mut payload = clean.clone();
        redact_payload(&fake, &mut payload);
        assert_eq!(payload, clean);

        let mut once = json!({ "note": "a SECRET", "list": ["SECRET"] });
        redact_payload(&fake, &mut once);
        let mut twice = once.clone();
        redact_payload(&fake, &mut twice);
        assert_eq!(once, twice);
    }

    #[test]
    fn scalar_payloads_are_redacted_too() {
        let fake = Fake::default();
        let mut text = json!("a SECRET");
        redact_payload(&fake, &mut text);
        assert_eq!(text, json!("a [REDACTED]"));
        let mut number = json!(42);
        redact_payload(&fake, &mut number);
        assert_eq!(number, json!(42));
    }

    #[test]
    fn a_redactor_that_breaks_the_document_never_leaves_the_original() {
        let mut payload = json!({ "note": "SECRET", "n": 1 });
        redact_payload(&Breaking, &mut payload);
        let Value::String(text) = &payload else {
            panic!("expected the redactor's text, got {payload}");
        };
        assert!(!text.contains("SECRET") && text.contains("gone"), "{text}");
    }

    #[test]
    fn serialized_payloads_and_text_that_is_not_json() {
        let fake = Fake::default();
        let clean = "{ \"b\": 1,  \"a\": [true] }";
        assert_eq!(redact_payload_text(&fake, clean), clean);
        assert_eq!(
            redact_payload_text(&fake, r#"{"z":"SECRET","a":"x"}"#),
            r#"{"z":"[REDACTED]","a":"x"}"#
        );
        assert_eq!(
            redact_payload_text(&fake, r#"{"note":"SECRET" truncated"#),
            r#"{"note":"[REDACTED]" truncated"#
        );
        assert_eq!(redact_payload_text(&fake, ""), "");
    }

    #[test]
    fn the_port_is_injectable_as_a_trait_object() {
        let shared: SharedRedaction = Arc::new(Fake::default());
        let other = Arc::clone(&shared);
        let handle = std::thread::spawn(move || other.redact_text("SECRET"));
        assert_eq!(handle.join().unwrap(), "[REDACTED]");
        assert_eq!(shared.redact_value("db_password", "v"), "[REDACTED]");
    }
}
