//! Turns a finished `agy` process (stdout events, stderr text, exit code) into what actually happened.

use super::types::{
    ParseError, ResultStatus, StepState, StepUpdate, StreamEvent, Usage, parse_line,
};

/// The raw material of one turn.
#[derive(Debug, Clone, Copy)]
pub struct TurnInput<'a> {
    /// Everything the process wrote to standard output.
    pub stdout: &'a str,
    /// Everything it wrote to standard error.
    pub stderr: &'a str,
    /// Exit code, when the process exited by itself.
    pub exit_code: Option<i32>,
    /// The runner killed it for exceeding its timeout.
    pub timed_out: bool,
    /// Vela cancelled it.
    pub cancelled: bool,
}

/// An action the provider refused because headless mode cannot prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeniedAction {
    /// The action kind (`command`).
    pub action: String,
    /// Its display name (`RunCommand`).
    pub display_name: String,
}

/// Whether a tool step actually ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Executed {
    /// It ran.
    Yes,
    /// It did not run, even if the step says `DONE`.
    No,
    /// The evidence does not say.
    Unknown,
}

/// One tool step.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolStepReport {
    /// Position of the step.
    pub step_index: u32,
    /// The tool.
    pub tool: String,
    /// The step's final state as reported.
    pub state: StepState,
    /// What really happened.
    pub executed: Executed,
    /// Why, when it did not run.
    pub reason: Option<String>,
}

/// The primary classification of a turn.
#[derive(Debug, Clone, PartialEq)]
pub enum TurnClass {
    /// The turn finished with a result and nothing blocked it.
    Completed,
    /// A tool needed a permission nobody could grant: the work did not happen although the process "succeeded".
    SoftDenied,
    /// A hook refused a tool call.
    HookDenied {
        /// The hook's reason.
        reason: String,
    },
    /// A hook crashed, timed out or printed garbage; the tool did not run (fail closed).
    HookFailed {
        /// The first line of the failure.
        message: String,
    },
    /// The provider reported exhausted capacity.
    QuotaExhausted {
        /// The provider's text.
        text: String,
    },
    /// The provider blocked the account or service for terms or policy reasons: never retried, never worked around.
    ProviderPolicyBlock {
        /// The provider's text.
        text: String,
    },
    /// The user is not signed in.
    AuthRequired {
        /// The provider's text.
        text: String,
    },
    /// The result status was `WAITING` (undefined by the provider).
    Waiting,
    /// The runner timed the process out.
    TimedOut,
    /// Vela cancelled the process.
    Cancelled,
    /// The stream ended without a `result` event.
    Incomplete,
    /// The provider reported an error result.
    Failed {
        /// What it said.
        message: String,
    },
}

/// Everything learned from one turn.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnOutcome {
    /// What happened.
    pub class: TurnClass,
    /// The conversation id (from `init` or `result`).
    pub conversation_id: Option<String>,
    /// The agent's final text.
    pub response: String,
    /// Every tool step with whether it really ran.
    pub steps: Vec<ToolStepReport>,
    /// Denied actions reported by the result.
    pub denied: Vec<DeniedAction>,
    /// Turns in the conversation so far.
    pub num_turns: u32,
    /// Total usage.
    pub usage: Option<Usage>,
    /// Lines that were not usable events.
    pub malformed_lines: u32,
    /// Events of kinds this version does not know.
    pub unknown_events: u32,
    /// Usable events seen.
    pub events_seen: u32,
    /// The first line of standard error, trimmed.
    pub stderr_excerpt: String,
}

/// What kind of provider message a text is, when it is one of the known failure shapes.
pub fn classify_text(text: &str) -> Option<TurnClass> {
    let lower = text.to_ascii_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| lower.contains(n));
    let excerpt = || {
        text.lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim()
            .chars()
            .take(300)
            .collect::<String>()
    };
    if has(&[
        "account has been suspended",
        "account has been disabled",
        "violation of the terms",
        "violates the terms of service",
        "access to this service has been blocked",
    ]) {
        return Some(TurnClass::ProviderPolicyBlock { text: excerpt() });
    }
    if has(&[
        "not signed in",
        "not authenticated",
        "unauthenticated",
        "sign in with your google",
        "authentication required",
        "please log in",
        "login required",
    ]) {
        return Some(TurnClass::AuthRequired { text: excerpt() });
    }
    if has(&[
        "resource_exhausted",
        "exhausted your capacity",
        "quota will reset",
        "rate limit",
        "too many requests",
        "quota exceeded",
        "(code 429)",
    ]) {
        return Some(TurnClass::QuotaExhausted { text: excerpt() });
    }
    None
}

/// Tool names grouped by the permission action they need.
fn needs_action(tool: &str) -> Option<&'static str> {
    match tool {
        "run_command" | "send_command_input" => Some("command"),
        "write_to_file"
        | "replace_file_content"
        | "multi_replace_file_content"
        | "sed_file"
        | "notebook_edit" => Some("write"),
        _ => None,
    }
}

fn first_line(s: &str) -> String {
    s.lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim()
        .chars()
        .take(300)
        .collect()
}

fn hook_denial_reason(message: &str) -> Option<String> {
    let marker = "denied by pre-tool hook";
    let at = message.find(marker)?;
    let rest = message[at + marker.len()..].trim_start_matches(':').trim();
    Some(rest.to_owned())
}

fn is_hook_failure(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("hook")
        && (lower.contains("failed") || lower.contains("timed out") || lower.contains("timeout"))
}

/// Interprets one turn. The exit code and `result.status` are inputs, never answers.
pub fn interpret_turn(input: TurnInput<'_>) -> TurnOutcome {
    let mut conversation_id: Option<String> = None;
    let mut steps_by_index: Vec<StepUpdate> = Vec::new();
    let mut result = None;
    let (mut malformed, mut unknown, mut seen) = (0u32, 0u32, 0u32);

    for line in input.stdout.lines() {
        match parse_line(line) {
            Ok(StreamEvent::Init(i)) => {
                seen += 1;
                conversation_id.get_or_insert(i.conversation_id);
            }
            Ok(StreamEvent::Step(s)) => {
                seen += 1;
                if conversation_id.is_none() && !s.conversation_id.is_empty() {
                    conversation_id = Some(s.conversation_id.clone());
                }
                if s.step_type == "tool" {
                    // The last update for a step index is its final state.
                    if let Some(existing) = steps_by_index
                        .iter_mut()
                        .find(|e| e.step_index == s.step_index)
                    {
                        *existing = s;
                    } else {
                        steps_by_index.push(s);
                    }
                }
            }
            Ok(StreamEvent::Result(r)) => {
                seen += 1;
                if !r.conversation_id.is_empty() {
                    conversation_id = Some(r.conversation_id.clone());
                }
                result = Some(r);
            }
            Ok(StreamEvent::Unknown(_)) => unknown += 1,
            Err(ParseError::Empty) => {}
            Err(ParseError::Malformed(_)) => malformed += 1,
        }
    }

    let denied: Vec<DeniedAction> = result
        .as_ref()
        .map(|r| {
            r.denied_actions
                .iter()
                .map(|(action, display_name)| DeniedAction {
                    action: action.clone(),
                    display_name: display_name.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    let stderr_lower = input.stderr.to_ascii_lowercase();
    let stderr_denial =
        stderr_lower.contains("auto-denied") || stderr_lower.contains("cannot prompt for");
    let soft_denied = !denied.is_empty() || stderr_denial;

    let steps: Vec<ToolStepReport> = steps_by_index
        .iter()
        .map(|s| {
            let tool = s.tool_name.clone().unwrap_or_default();
            let error = s
                .tool
                .as_ref()
                .and_then(|t| t.error.as_ref())
                .map(|e| e.message.clone());
            let (executed, reason) = match (&s.state, needs_action(&tool)) {
                (StepState::Error, _) => (Executed::No, error.map(|m| first_line(&m))),
                (StepState::Done, Some(action)) if soft_denied => {
                    let matches = denied.iter().any(|d| d.action.eq_ignore_ascii_case(action));
                    if matches || (denied.is_empty() && stderr_denial) {
                        (
                            Executed::No,
                            Some(
                                "the provider auto-denied it (headless mode cannot prompt)"
                                    .to_owned(),
                            ),
                        )
                    } else {
                        (
                            Executed::Unknown,
                            Some(
                                "a denial was reported but not for this kind of action".to_owned(),
                            ),
                        )
                    }
                }
                (StepState::Done, _) => (Executed::Yes, None),
                (StepState::Active, _) => (
                    Executed::Unknown,
                    Some("the step never finished".to_owned()),
                ),
                (StepState::Other(_), _) => (Executed::Unknown, None),
            };
            ToolStepReport {
                step_index: s.step_index,
                tool,
                state: s.state.clone(),
                executed,
                reason,
            }
        })
        .collect();

    let hook_error = steps_by_index.iter().find_map(|s| {
        let message = s.tool.as_ref()?.error.as_ref()?.message.clone();
        if let Some(reason) = hook_denial_reason(&message) {
            Some(TurnClass::HookDenied { reason })
        } else if is_hook_failure(&message) {
            Some(TurnClass::HookFailed {
                message: first_line(&message),
            })
        } else {
            None
        }
    });

    let response = result
        .as_ref()
        .map(|r| r.response.clone())
        .unwrap_or_default();
    let provider_failure_text = {
        let mut t = input.stderr.to_owned();
        if let Some(r) = &result {
            if let Some(e) = &r.error {
                t.push('\n');
                t.push_str(e);
            }
            if r.status != ResultStatus::Success {
                t.push('\n');
                t.push_str(&r.response);
            }
        }
        t
    };

    let class = if input.cancelled {
        TurnClass::Cancelled
    } else if input.timed_out {
        TurnClass::TimedOut
    } else if let Some(text_class) = classify_text(&provider_failure_text) {
        text_class
    } else if let Some(hook) = hook_error {
        hook
    } else if let Some(r) = &result {
        match &r.status {
            ResultStatus::Waiting => TurnClass::Waiting,
            ResultStatus::Success if soft_denied => TurnClass::SoftDenied,
            ResultStatus::Success => TurnClass::Completed,
            other => TurnClass::Failed {
                message: r
                    .error
                    .clone()
                    .unwrap_or_else(|| format!("the provider reported {other:?}")),
            },
        }
    } else if seen == 0 && !input.stderr.trim().is_empty() {
        TurnClass::Failed {
            message: first_line(input.stderr),
        }
    } else {
        TurnClass::Incomplete
    };

    TurnOutcome {
        class,
        conversation_id,
        response,
        steps,
        denied,
        num_turns: result.as_ref().map_or(0, |r| r.num_turns),
        usage: result.and_then(|r| r.usage),
        malformed_lines: malformed,
        unknown_events: unknown,
        events_seen: seen,
        stderr_excerpt: first_line(input.stderr),
    }
}
