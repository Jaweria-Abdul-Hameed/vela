//! Redaction of journal payloads (`IMPLEMENTATION_ARCHITECTURE.md` section 8: "payloads are redacted before they
//! reach the writer thread").
//!
//! A payload is redacted as a JSON value, not as text: every string is redacted on its own, and a member whose name
//! is credential-like loses its whole value. The shape of the payload never changes, so a redacted payload still
//! deserializes into its typed form.

use std::sync::Arc;

use serde_json::Value;

/// The redactor as the core sees it.
///
/// The production implementation is `vela-process::redact::PatternRedactor`, which the core cannot name
/// (`vela-core` does not depend on `vela-process`). The composition root adapts the one shared redactor to this port;
/// both methods map one to one onto `vela_process::redact::Redactor`.
pub trait RedactionPort: Send + Sync {
    /// Redacts free text.
    fn redact_text(&self, text: &str) -> String;

    /// Redacts a value whose name is known. A credential-like `name` redacts the whole value.
    fn redact_value(&self, name: &str, value: &str) -> String;
}

/// The injectable form of a [`RedactionPort`].
pub type SharedRedaction = Arc<dyn RedactionPort>;

/// Redacts a journal payload in place.
///
/// Strings are redacted by member name when they have one (array elements take the name of the member that holds the
/// array) and as free text otherwise. A number under a credential-like name becomes the redacted string. Member names
/// are redacted as free text. Booleans and `null` are never secrets and are left alone.
pub fn redact_payload(redactor: &dyn RedactionPort, payload: &mut Value) {
    redact_node(redactor, None, payload);
}

/// Redacts a serialized journal payload. Text that is not JSON is redacted as free text, so nothing passes through
/// unredacted because it failed to parse.
pub fn redact_payload_text(redactor: &dyn RedactionPort, payload: &str) -> String {
    match serde_json::from_str::<Value>(payload) {
        Ok(mut value) => {
            redact_payload(redactor, &mut value);
            value.to_string()
        }
        Err(_) => redactor.redact_text(payload),
    }
}

fn redact_node(redactor: &dyn RedactionPort, name: Option<&str>, node: &mut Value) {
    match node {
        Value::Null | Value::Bool(_) => {}
        Value::Number(number) => {
            let Some(name) = name else {
                return;
            };
            let shown = number.to_string();
            let redacted = redactor.redact_value(name, &shown);
            if redacted != shown {
                *node = Value::String(redacted);
            }
        }
        Value::String(text) => {
            let redacted = match name {
                Some(name) => redactor.redact_value(name, text),
                None => redactor.redact_text(text),
            };
            if redacted != *text {
                *text = redacted;
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_node(redactor, name, item);
            }
        }
        Value::Object(members) => {
            for (member, value) in members.iter_mut() {
                redact_node(redactor, Some(member), value);
            }
            let renamed: Vec<(String, String)> = members
                .keys()
                .filter_map(|member| {
                    let redacted = redactor.redact_text(member);
                    (redacted != *member).then(|| (member.clone(), redacted))
                })
                .collect();
            for (member, redacted) in renamed {
                let Some(value) = members.remove(&member) else {
                    continue;
                };
                // Two names can redact to the same text; neither member may replace the other.
                let mut name = redacted.clone();
                let mut copy = 2;
                while members.contains_key(&name) {
                    name = format!("{redacted} ({copy})");
                    copy += 1;
                }
                members.insert(name, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// A stand-in for the production redactor: `SECRET` is the known secret pattern, and names containing `password`
    /// or `token` are credential-like.
    struct Fake;

    impl RedactionPort for Fake {
        fn redact_text(&self, text: &str) -> String {
            text.replace("SECRET", "[REDACTED]")
        }

        fn redact_value(&self, name: &str, value: &str) -> String {
            let name = name.to_ascii_lowercase();
            if name.contains("password") || name.contains("token") {
                "[REDACTED]".to_owned()
            } else {
                self.redact_text(value)
            }
        }
    }

    #[test]
    fn every_string_of_a_payload_passes_through_the_redactor() {
        let mut payload = json!({
            "kind": "CommandFinished",
            "command": "deploy --key SECRET",
            "exit_code": 0,
            "ok": true,
            "nothing": null,
            "env": { "DB_PASSWORD": "hunter2", "PATH": "/usr/bin", "PIN_TOKEN": 1234 },
            "lines": ["plain", "has SECRET inside", ["nested SECRET"]],
            "tokens": ["a", "b", 3],
            "SECRET-named": { "deep": [{ "password": "x", "note": "SECRET" }] },
        });
        redact_payload(&Fake, &mut payload);
        assert_eq!(
            payload,
            json!({
                "kind": "CommandFinished",
                "command": "deploy --key [REDACTED]",
                "exit_code": 0,
                "ok": true,
                "nothing": null,
                "env": { "DB_PASSWORD": "[REDACTED]", "PATH": "/usr/bin", "PIN_TOKEN": "[REDACTED]" },
                "lines": ["plain", "has [REDACTED] inside", ["nested [REDACTED]"]],
                "tokens": ["[REDACTED]", "[REDACTED]", "[REDACTED]"],
                "[REDACTED]-named": { "deep": [{ "password": "[REDACTED]", "note": "[REDACTED]" }] },
            })
        );
    }

    #[test]
    fn redaction_is_idempotent_and_keeps_clean_payloads_identical() {
        let clean =
            json!({ "run_id": "run-1", "seq": 7, "items": [1, 2.5, "text"], "flag": false });
        let mut payload = clean.clone();
        redact_payload(&Fake, &mut payload);
        assert_eq!(payload, clean);

        let mut once = json!({ "password": "x", "note": "a SECRET", "list": ["SECRET"] });
        redact_payload(&Fake, &mut once);
        let mut twice = once.clone();
        redact_payload(&Fake, &mut twice);
        assert_eq!(once, twice);
    }

    #[test]
    fn member_names_that_redact_to_the_same_text_are_all_kept() {
        let mut payload = json!({ "SECRET": 1, "SECRET ": 2, "[REDACTED]": 3 });
        redact_payload(&Fake, &mut payload);
        let members = payload.as_object().unwrap();
        assert_eq!(members.len(), 3, "{payload}");
        let mut values: Vec<i64> = members
            .values()
            .map(|value| value.as_i64().unwrap())
            .collect();
        values.sort_unstable();
        assert_eq!(values, [1, 2, 3]);
        assert!(
            members.keys().all(|name| name.starts_with("[REDACTED]")),
            "{payload}"
        );
    }

    #[test]
    fn scalar_payloads_are_redacted_too() {
        let mut text = json!("a SECRET");
        redact_payload(&Fake, &mut text);
        assert_eq!(text, json!("a [REDACTED]"));
        let mut number = json!(42);
        redact_payload(&Fake, &mut number);
        assert_eq!(number, json!(42));
    }

    #[test]
    fn serialized_payloads_and_text_that_is_not_json() {
        let redacted = redact_payload_text(&Fake, r#"{"password":"hunter2","note":"SECRET"}"#);
        assert_eq!(
            serde_json::from_str::<Value>(&redacted).unwrap(),
            json!({ "password": "[REDACTED]", "note": "[REDACTED]" })
        );
        assert_eq!(
            redact_payload_text(&Fake, r#"{"note":"SECRET" truncated"#),
            r#"{"note":"[REDACTED]" truncated"#
        );
        assert_eq!(redact_payload_text(&Fake, ""), "");
    }

    #[test]
    fn the_port_is_injectable_as_a_trait_object() {
        let shared: SharedRedaction = Arc::new(Fake);
        let other = Arc::clone(&shared);
        let handle = std::thread::spawn(move || other.redact_text("SECRET"));
        assert_eq!(handle.join().unwrap(), "[REDACTED]");
        assert_eq!(shared.redact_value("api_token", "v"), "[REDACTED]");
    }
}
