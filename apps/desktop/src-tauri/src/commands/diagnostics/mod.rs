//! The `diagnostics` command group. F04 delivers its first command, `diagnostics.ping`, as the walking-skeleton
//! example: a typed request and result between React and the Rust core, one event on the way, and a typed
//! `VelaError` for invalid arguments. Later tickets add the real diagnostics commands (`diagnostics.export`) here.
//!
//! Handlers are thin: they validate arguments and delegate. [`PingRequest::validate`] and [`ping`] are plain
//! functions with no Tauri dependency, so the argument rules are unit-tested without a window.

use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Runtime, ipc::Invoke};
use vela_domain::errors::{ErrorClass, VelaError};

/// Dotted names of this group's commands (the wire name replaces `.` with `_`).
pub const COMMANDS: &[&str] = &["diagnostics.ping"];

/// The event the example command emits. TypeScript: `PING_EVENT` in `packages/contracts/src/commands/diagnostics.ts`.
pub const PING_EVENT: &str = "vela:ping";

/// The longest message the example command accepts, in bytes.
pub const MAX_MESSAGE_BYTES: usize = 256;

/// The argument of `diagnostics.ping`. TypeScript: `PingRequest`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PingRequest {
    /// A short text the core echoes back.
    pub message: String,
}

/// The result of `diagnostics.ping`. TypeScript: `PingResult`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PingResult {
    /// Always `pong`.
    pub reply: String,
    /// The request's message, echoed.
    pub message: String,
}

/// The payload of [`PING_EVENT`]. TypeScript: `PingEvent`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PingEvent {
    /// The request's message, echoed.
    pub message: String,
    /// One more than the previous ping event of this process; the first is 1.
    pub sequence: u64,
}

impl PingRequest {
    /// Checks the argument: the message must be non-blank, at most [`MAX_MESSAGE_BYTES`] bytes, and free of control
    /// characters. Violations are `VALIDATION_FAILURE` errors that name the field and the rule.
    pub fn validate(&self) -> Result<(), VelaError> {
        let invalid = |rule: &str| {
            VelaError::of_class(
                ErrorClass::ValidationFailure,
                format!("diagnostics.ping: `message` {rule}"),
            )
            .with_state_preserved("nothing was changed")
            .with_user_action("send a short plain-text message")
        };
        if self.message.trim().is_empty() {
            return Err(invalid("must not be blank"));
        }
        if self.message.len() > MAX_MESSAGE_BYTES {
            return Err(invalid(&format!(
                "must be at most {MAX_MESSAGE_BYTES} bytes"
            )));
        }
        if self.message.chars().any(char::is_control) {
            return Err(invalid("must not contain control characters"));
        }
        Ok(())
    }
}

/// The core's answer to a valid request.
pub fn ping(request: &PingRequest) -> Result<PingResult, VelaError> {
    request.validate()?;
    Ok(PingResult {
        reply: "pong".to_owned(),
        message: request.message.clone(),
    })
}

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// `diagnostics.ping`: validates the request, emits [`PING_EVENT`] and returns the typed result.
#[tauri::command]
pub fn diagnostics_ping<R: Runtime>(
    app: AppHandle<R>,
    request: PingRequest,
) -> Result<PingResult, VelaError> {
    let result = ping(&request)?;
    let event = PingEvent {
        message: result.message.clone(),
        sequence: SEQUENCE.fetch_add(1, Ordering::Relaxed) + 1,
    };
    app.emit(PING_EVENT, event).map_err(|error| {
        VelaError::internal(format!(
            "diagnostics.ping could not emit its event: {error}"
        ))
    })?;
    Ok(result)
}

/// Runs the command in `invoke` if this group owns it; returns `false` when it does not.
pub fn handle<R: Runtime>(invoke: Invoke<R>) -> bool {
    run(tauri::generate_handler![diagnostics_ping], invoke)
}

/// Gives the handler closure its parameter type from the bound, as `Builder::invoke_handler` does.
fn run<R: Runtime>(handler: impl Fn(Invoke<R>) -> bool, invoke: Invoke<R>) -> bool {
    handler(invoke)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands;
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_context, noop_assets};
    use tauri::webview::InvokeRequest;
    use tauri::{Listener, WebviewWindow};

    fn request(message: &str) -> PingRequest {
        PingRequest {
            message: message.to_owned(),
        }
    }

    #[test]
    fn a_valid_request_returns_pong_and_echoes_the_message() {
        let result = ping(&request("hello")).expect("valid");
        assert_eq!(
            result,
            PingResult {
                reply: "pong".to_owned(),
                message: "hello".to_owned()
            }
        );
    }

    #[test]
    fn blank_overlong_and_control_character_messages_are_rejected_with_a_typed_error() {
        let too_long = "x".repeat(MAX_MESSAGE_BYTES + 1);
        for message in [
            "",
            "   ",
            "\t\n",
            too_long.as_str(),
            "bell\u{7}",
            "line\nbreak",
        ] {
            let error = ping(&request(message)).expect_err(message);
            assert_eq!(error.class(), ErrorClass::ValidationFailure, "{message:?}");
            assert!(error.what_failed().contains("`message`"));
            error
                .validate()
                .expect("the error is internally consistent");
        }
    }

    #[test]
    fn the_longest_allowed_message_is_accepted() {
        let longest = "x".repeat(MAX_MESSAGE_BYTES);
        assert!(ping(&request(&longest)).is_ok());
    }

    #[test]
    fn requests_with_unknown_fields_do_not_deserialize() {
        let parsed: Result<PingRequest, _> =
            serde_json::from_value(json!({ "message": "hi", "extra": 1 }));
        assert!(parsed.is_err());
    }

    #[test]
    fn the_json_shapes_match_the_typescript_contract() {
        assert_eq!(
            serde_json::to_value(PingResult {
                reply: "pong".into(),
                message: "hi".into()
            })
            .unwrap(),
            json!({ "reply": "pong", "message": "hi" })
        );
        assert_eq!(
            serde_json::to_value(PingEvent {
                message: "hi".into(),
                sequence: 3
            })
            .unwrap(),
            json!({ "message": "hi", "sequence": 3 })
        );
        assert_eq!(PING_EVENT, "vela:ping");
    }

    fn app() -> (tauri::App<MockRuntime>, WebviewWindow<MockRuntime>) {
        let app = commands::install(tauri::test::mock_builder())
            .build(mock_context(noop_assets()))
            .expect("build the mock application");
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("create the main window");
        (app, window)
    }

    fn invoke(window: &WebviewWindow<MockRuntime>, cmd: &str, body: Value) -> Result<Value, Value> {
        get_ipc_response(
            window,
            InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "http://tauri.localhost".parse().unwrap(),
                body: InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .map(|body| body.deserialize::<Value>().expect("a JSON response"))
    }

    #[test]
    fn ping_round_trips_through_the_ipc_layer_and_emits_the_event() {
        let (app, window) = app();
        let events: Arc<Mutex<Vec<String>>> = Arc::default();
        let sink = Arc::clone(&events);
        app.listen(PING_EVENT, move |event| {
            sink.lock().unwrap().push(event.payload().to_owned());
        });

        let result = invoke(
            &window,
            "diagnostics_ping",
            json!({ "request": { "message": "hello" } }),
        )
        .expect("ping succeeds");
        assert_eq!(result, json!({ "reply": "pong", "message": "hello" }));

        let deadline = Instant::now() + Duration::from_secs(5);
        while events.lock().unwrap().is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let received = events.lock().unwrap().clone();
        assert_eq!(received.len(), 1, "exactly one event: {received:?}");
        let payload: PingEvent = serde_json::from_str(&received[0]).expect("a PingEvent payload");
        assert_eq!(payload.message, "hello");
        assert!(payload.sequence >= 1);
    }

    #[test]
    fn invalid_arguments_are_rejected_with_a_typed_error_through_the_ipc_layer() {
        let (_app, window) = app();
        let error = invoke(
            &window,
            "diagnostics_ping",
            json!({ "request": { "message": "  " } }),
        )
        .expect_err("a blank message is rejected");
        assert_eq!(error["class"], "VALIDATION_FAILURE");
        assert!(error["what_failed"].as_str().unwrap().contains("blank"));

        for body in [
            json!({}),
            json!({ "request": { "message": 5 } }),
            json!({ "request": { "message": "hi", "extra": true } }),
        ] {
            assert!(
                invoke(&window, "diagnostics_ping", body.clone()).is_err(),
                "{body}"
            );
        }
    }

    #[test]
    fn an_unregistered_command_is_blocked() {
        let (_app, window) = app();
        for cmd in [
            "diagnostics_export",
            "diagnostics_ping_all",
            "project_explode",
            "ping",
            "shell_execute",
        ] {
            let error = invoke(&window, cmd, json!({})).expect_err(cmd);
            assert!(
                error
                    .as_str()
                    .is_some_and(|text| text.contains("not found")),
                "{cmd}: {error}"
            );
        }
    }
}
