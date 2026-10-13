use std::path::PathBuf;

use serde_json::Value;

use super::*;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/transcripts")
}

fn load(name: &str) -> (String, String) {
    let dir = fixtures();
    let read = |suffix: &str| std::fs::read_to_string(dir.join(format!("{name}{suffix}"))).unwrap();
    (read(".jsonl"), read(".stderr.txt"))
}

fn outcome_of(name: &str, exit_code: i32) -> TurnOutcome {
    let (stdout, stderr) = load(name);
    interpret_turn(TurnInput {
        stdout: &stdout,
        stderr: &stderr,
        exit_code: Some(exit_code),
        timed_out: false,
        cancelled: false,
    })
}

fn class_name(c: &TurnClass) -> &'static str {
    match c {
        TurnClass::Completed => "completed",
        TurnClass::SoftDenied => "soft_denied",
        TurnClass::HookDenied { .. } => "hook_denied",
        TurnClass::HookFailed { .. } => "hook_failed",
        TurnClass::QuotaExhausted { .. } => "quota_exhausted",
        TurnClass::AuthRequired { .. } => "auth_required",
        TurnClass::AccessFailure { .. } => "access_failure",
        TurnClass::ProviderPolicyBlock { .. } => "provider_policy_block",
        TurnClass::Waiting => "waiting",
        TurnClass::TimedOut => "timed_out",
        TurnClass::Cancelled => "cancelled",
        TurnClass::Incomplete => "incomplete",
        TurnClass::Failed { .. } => "failed",
    }
}

/// Every transcript in the manifest is classified as its expectation says.
#[test]
fn every_recorded_and_synthetic_transcript_is_classified_as_expected() {
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(fixtures().join("manifest.json")).unwrap())
            .unwrap();
    let transcripts = manifest["transcripts"].as_array().unwrap();
    assert!(transcripts.len() >= 10);
    for t in transcripts {
        let name = t["file"].as_str().unwrap();
        let exit = i32::try_from(t["exitCode"].as_i64().unwrap()).unwrap();
        let out = outcome_of(name, exit);
        let expect = &t["expect"];
        assert_eq!(
            class_name(&out.class),
            expect["class"].as_str().unwrap(),
            "{name}: {:?}",
            out.class
        );
        if let Some(r) = expect.get("response").and_then(Value::as_str) {
            assert_eq!(out.response, r, "{name}");
        }
        if let Some(id) = expect.get("conversationId").and_then(Value::as_str) {
            assert_eq!(out.conversation_id.as_deref(), Some(id), "{name}");
        }
        if let Some(n) = expect.get("numTurns").and_then(Value::as_u64) {
            assert_eq!(u64::from(out.num_turns), n, "{name}");
        }
        if let Some(n) = expect.get("malformedLines").and_then(Value::as_u64) {
            assert_eq!(u64::from(out.malformed_lines), n, "{name}");
        }
        if let Some(reason) = expect.get("reason").and_then(Value::as_str) {
            assert!(
                matches!(&out.class, TurnClass::HookDenied { reason: r } if r == reason),
                "{name}: {:?}",
                out.class
            );
        }
        if let Some(actions) = expect.get("deniedActions").and_then(Value::as_array) {
            let got: Vec<&str> = out.denied.iter().map(|d| d.action.as_str()).collect();
            let want: Vec<&str> = actions.iter().filter_map(Value::as_str).collect();
            assert_eq!(got, want, "{name}");
        }
        if let Some(steps) = expect.get("executedTools").and_then(Value::as_array) {
            for s in steps {
                let tool = s["name"].as_str().unwrap();
                let want = match s["executed"].as_str().unwrap() {
                    "yes" => Executed::Yes,
                    "no" => Executed::No,
                    _ => Executed::Unknown,
                };
                let report = out
                    .steps
                    .iter()
                    .find(|r| r.tool == tool)
                    .unwrap_or_else(|| panic!("{name}: no step for {tool}: {:?}", out.steps));
                assert_eq!(report.executed, want, "{name}: {tool}");
            }
        }
    }
}

#[test]
fn a_tool_step_marked_done_after_a_denial_is_not_treated_as_executed() {
    let out = outcome_of("soft-denial", 0);
    let step = out.steps.iter().find(|s| s.tool == "run_command").unwrap();
    assert_eq!(step.state, StepState::Done, "the provider says DONE");
    assert_eq!(step.executed, Executed::No);
    assert!(step.reason.as_deref().unwrap().contains("auto-denied"));
    assert_eq!(out.class, TurnClass::SoftDenied);
    assert!(
        out.response.is_empty(),
        "the process 'succeeded' with an empty response"
    );
}

#[test]
fn exit_code_and_result_status_are_never_trusted() {
    // The documented bug: a quota error exits 0 after the print timeout; the soft denial also exits 0 with SUCCESS.
    assert_ne!(outcome_of("soft-denial", 0).class, TurnClass::Completed);
    assert!(matches!(
        outcome_of("quota-text", 0).class,
        TurnClass::QuotaExhausted { .. }
    ));
    // A non-zero exit with a complete, clean result is still a completed turn (the result is the evidence).
    assert_eq!(outcome_of("success-pong", 1).class, TurnClass::Completed);
}

#[test]
fn timeouts_and_cancellation_win_over_anything_the_stream_says() {
    let (stdout, stderr) = load("success-pong");
    let timed_out = interpret_turn(TurnInput {
        stdout: &stdout,
        stderr: &stderr,
        exit_code: None,
        timed_out: true,
        cancelled: false,
    });
    assert_eq!(timed_out.class, TurnClass::TimedOut);
    let cancelled = interpret_turn(TurnInput {
        stdout: &stdout,
        stderr: &stderr,
        exit_code: None,
        timed_out: false,
        cancelled: true,
    });
    assert_eq!(cancelled.class, TurnClass::Cancelled);
}

#[test]
fn a_hook_crash_fails_closed_and_a_hook_deny_carries_the_reason() {
    let crash = outcome_of("hook-crash", 0);
    assert!(matches!(crash.class, TurnClass::HookFailed { .. }));
    assert!(
        crash
            .steps
            .iter()
            .all(|s| s.executed == Executed::No || s.tool == "view_file")
    );
    let deny = outcome_of("hook-deny", 0);
    assert!(
        matches!(&deny.class, TurnClass::HookDenied { reason } if reason == "blocked by vela test policy")
    );
}

#[test]
fn authentication_and_quota_text_are_recognized_and_ordinary_text_is_not() {
    assert!(matches!(
        classify_text("You are not signed in. Run `agy` interactively and sign in"),
        Some(TurnClass::AuthRequired { .. })
    ));
    assert!(matches!(
        classify_text("jetski: RESOURCE_EXHAUSTED (code 429): You have exhausted your capacity"),
        Some(TurnClass::QuotaExhausted { .. })
    ));
    assert!(matches!(
        classify_text("Your account has been suspended for a violation of the terms of service"),
        Some(TurnClass::ProviderPolicyBlock { .. })
    ));
    assert!(classify_text("jetski: no output produced").is_none());
    assert!(classify_text("").is_none());
}

#[test]
fn a_response_that_merely_mentions_quota_is_not_a_quota_error() {
    let stdout = [
        r#"{"event":"init","conversation_id":"c-1","init":{"cwd":"x","tools":[],"permission_mode":"request-review"}}"#,
        r#"{"event":"result","result":{"conversation_id":"c-1","status":"SUCCESS","response":"Set a disk quota and a rate limit in the config.","num_turns":1}}"#,
    ]
    .join("\n");
    let out = interpret_turn(TurnInput {
        stdout: &stdout,
        stderr: "",
        exit_code: Some(0),
        timed_out: false,
        cancelled: false,
    });
    assert_eq!(out.class, TurnClass::Completed);
}

#[test]
fn parse_line_handles_every_event_kind_and_bad_input() {
    assert_eq!(parse_line("").unwrap_err(), ParseError::Empty);
    assert!(matches!(
        parse_line("not json"),
        Err(ParseError::Malformed(_))
    ));
    assert!(matches!(parse_line("[1,2]"), Err(ParseError::Malformed(_))));
    assert!(
        matches!(parse_line(r#"{"event":"mystery"}"#), Ok(StreamEvent::Unknown(k)) if k == "mystery")
    );
    assert!(matches!(
        parse_line(r#"{"event":"step_update"}"#),
        Err(ParseError::Malformed(_))
    ));
    let StreamEvent::Step(s) = parse_line(
        r#"{"event":"step_update","step_update":{"conversation_id":"c","step_index":3,"state":"WEIRD","step_type":"tool","tool_name":"run_command","tool_info":{"name":"run_command","parameters":{"CommandLine":"x"},"output":"o"}}}"#,
    )
    .unwrap() else {
        panic!("a step");
    };
    assert_eq!(s.step_index, 3);
    assert_eq!(s.state, StepState::Other("WEIRD".to_owned()));
    assert_eq!(s.tool.unwrap().output.as_deref(), Some("o"));
    let StreamEvent::Result(r) = parse_line(
        r#"{"event":"result","result":{"conversation_id":"c","status":"FUTURE_STATUS","response":"r","num_turns":2}}"#,
    )
    .unwrap() else {
        panic!("a result");
    };
    assert_eq!(r.status, ResultStatus::Other("FUTURE_STATUS".to_owned()));
}

#[test]
fn a_stream_with_no_events_and_no_text_is_incomplete_not_a_success() {
    let out = interpret_turn(TurnInput {
        stdout: "",
        stderr: "",
        exit_code: Some(0),
        timed_out: false,
        cancelled: false,
    });
    assert_eq!(out.class, TurnClass::Incomplete);
    assert!(out.conversation_id.is_none());
}
