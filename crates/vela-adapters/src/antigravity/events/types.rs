//! Wire types of the `stream-json` output. Every field the provider might omit or add is optional or ignored.

use std::fmt;

use serde::Deserialize;
use serde_json::Value;

/// A line that is not a usable event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The line is blank.
    Empty,
    /// The line is not JSON, or is JSON with an unusable shape.
    Malformed(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("empty line"),
            Self::Malformed(why) => write!(f, "malformed event: {why}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Token usage, as far as it is reported.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Usage {
    /// Input tokens.
    pub input_tokens: u64,
    /// Output tokens.
    pub output_tokens: u64,
    /// Thinking tokens.
    pub thinking_tokens: u64,
    /// Tokens read from cache.
    pub cache_read_tokens: u64,
    /// Total tokens.
    pub total_tokens: u64,
}

/// The `init` event.
#[derive(Debug, Clone, PartialEq)]
pub struct InitEvent {
    /// The conversation this process runs.
    pub conversation_id: String,
    /// Working directory.
    pub cwd: String,
    /// Tool names offered to the model.
    pub tools: Vec<String>,
    /// `request-review` and similar.
    pub permission_mode: String,
}

/// State of a step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepState {
    /// In progress.
    Active,
    /// Finished (not necessarily executed: see the interpreter).
    Done,
    /// Failed.
    Error,
    /// Anything else the provider invents.
    Other(String),
}

/// A tool's own error.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ToolError {
    /// Error type (`TOOL_ERROR`).
    #[serde(rename = "type", default)]
    pub kind: String,
    /// Message.
    #[serde(default)]
    pub message: String,
}

/// What a tool step carried.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolInfo {
    /// Tool arguments (opaque JSON).
    pub parameters: Value,
    /// Output text, when the tool produced any.
    pub output: Option<String>,
    /// The tool's error, when it failed.
    pub error: Option<ToolError>,
}

/// A `step_update` event.
#[derive(Debug, Clone, PartialEq)]
pub struct StepUpdate {
    /// The conversation.
    pub conversation_id: String,
    /// Position of the step.
    pub step_index: u32,
    /// State.
    pub state: StepState,
    /// `user_input`, `agent_response`, `tool`, `unknown`, `system_message`, ...
    pub step_type: String,
    /// The tool's name for tool steps.
    pub tool_name: Option<String>,
    /// Tool details.
    pub tool: Option<ToolInfo>,
    /// A fragment of the agent's text.
    pub text_delta: Option<String>,
    /// How long the step took.
    pub duration_seconds: Option<f64>,
    /// Usage for the step.
    pub usage: Option<Usage>,
}

/// `result.status`. Never trusted on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultStatus {
    /// `SUCCESS`.
    Success,
    /// `ERROR`.
    Error,
    /// `CANCELED`.
    Canceled,
    /// `INTERRUPTED`.
    Interrupted,
    /// `INVALID`.
    Invalid,
    /// `WAITING` (undefined by the provider).
    Waiting,
    /// `RUNNING`.
    Running,
    /// Anything else.
    Other(String),
}

/// The `result` event.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultEvent {
    /// The conversation.
    pub conversation_id: String,
    /// Reported status.
    pub status: ResultStatus,
    /// The agent's final text.
    pub response: String,
    /// Total duration.
    pub duration_seconds: f64,
    /// Number of turns in the conversation so far.
    pub num_turns: u32,
    /// Total usage.
    pub usage: Option<Usage>,
    /// Actions denied because headless mode cannot prompt (`action`, `display_name`).
    pub denied_actions: Vec<(String, String)>,
    /// An error text, when the result carries one.
    pub error: Option<String>,
}

/// One parsed line.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    /// `init`.
    Init(InitEvent),
    /// `step_update`.
    Step(StepUpdate),
    /// `result`.
    Result(ResultEvent),
    /// An event kind this version does not know (kept so a newer provider does not break a run).
    Unknown(String),
}

fn text(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn state_of(s: &str) -> StepState {
    match s {
        "ACTIVE" => StepState::Active,
        "DONE" => StepState::Done,
        "ERROR" => StepState::Error,
        other => StepState::Other(other.to_owned()),
    }
}

fn status_of(s: &str) -> ResultStatus {
    match s {
        "SUCCESS" => ResultStatus::Success,
        "ERROR" => ResultStatus::Error,
        "CANCELED" => ResultStatus::Canceled,
        "INTERRUPTED" => ResultStatus::Interrupted,
        "INVALID" => ResultStatus::Invalid,
        "WAITING" => ResultStatus::Waiting,
        "RUNNING" => ResultStatus::Running,
        other => ResultStatus::Other(other.to_owned()),
    }
}

fn usage_of(v: &Value) -> Option<Usage> {
    v.get("usage")
        .and_then(|u| serde_json::from_value(u.clone()).ok())
}

/// Parses one line of output. Blank lines and anything that is not a JSON object with a string `event` are errors the
/// caller counts and skips.
pub fn parse_line(line: &str) -> Result<StreamEvent, ParseError> {
    let line = line.trim();
    if line.is_empty() {
        return Err(ParseError::Empty);
    }
    let value: Value =
        serde_json::from_str(line).map_err(|e| ParseError::Malformed(e.to_string()))?;
    let kind = value
        .get("event")
        .and_then(Value::as_str)
        .ok_or_else(|| ParseError::Malformed("no \"event\" field".to_owned()))?;
    match kind {
        "init" => {
            let init = value.get("init").unwrap_or(&Value::Null);
            Ok(StreamEvent::Init(InitEvent {
                conversation_id: text(&value, "conversation_id"),
                cwd: text(init, "cwd"),
                tools: init
                    .get("tools")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default(),
                permission_mode: text(init, "permission_mode"),
            }))
        }
        "step_update" => {
            let s = value
                .get("step_update")
                .ok_or_else(|| ParseError::Malformed("step_update without a body".to_owned()))?;
            let step_index = s
                .get("step_index")
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| ParseError::Malformed("step_index is not a number".to_owned()))?;
            let tool = s.get("tool_info").map(|t| ToolInfo {
                parameters: t.get("parameters").cloned().unwrap_or(Value::Null),
                output: t.get("output").and_then(Value::as_str).map(str::to_owned),
                error: t
                    .get("error")
                    .and_then(|e| serde_json::from_value(e.clone()).ok()),
            });
            Ok(StreamEvent::Step(StepUpdate {
                conversation_id: text(s, "conversation_id"),
                step_index,
                state: state_of(&text(s, "state")),
                step_type: text(s, "step_type"),
                tool_name: s
                    .get("tool_name")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                tool,
                text_delta: s
                    .get("text_delta")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                duration_seconds: s.get("duration_seconds").and_then(Value::as_f64),
                usage: usage_of(s),
            }))
        }
        "result" => {
            let r = value
                .get("result")
                .ok_or_else(|| ParseError::Malformed("result without a body".to_owned()))?;
            let denied = r
                .get("denied_actions")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .map(|d| (text(d, "action"), text(d, "display_name")))
                        .collect()
                })
                .unwrap_or_default();
            Ok(StreamEvent::Result(ResultEvent {
                conversation_id: text(r, "conversation_id"),
                status: status_of(&text(r, "status")),
                response: text(r, "response"),
                duration_seconds: r
                    .get("duration_seconds")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0),
                num_turns: r
                    .get("num_turns")
                    .and_then(Value::as_u64)
                    .and_then(|n| u32::try_from(n).ok())
                    .unwrap_or(0),
                usage: usage_of(r),
                denied_actions: denied,
                error: r.get("error").and_then(Value::as_str).map(str::to_owned),
            }))
        }
        other => Ok(StreamEvent::Unknown(other.to_owned())),
    }
}
