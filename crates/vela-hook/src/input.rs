//! Parsing the provider's PreToolUse input and mapping a tool call to a policy operation.
//!
//! Verified input (agy 1.2.14, probe P2): `conversationId`, `workspacePaths`, `transcriptPath`, `modelName`,
//! `toolCall { name, args }`, `stepIdx`. Argument names differ per tool, so each mapping looks for the documented key first
//! and a short list of aliases second; a call whose path or command cannot be found is `Unknown` and therefore asked about,
//! never allowed.

use serde_json::Value;
use vela_domain::policy::{Dialect, Operation};

/// What the hook knows about one tool call.
#[derive(Debug, Clone, PartialEq)]
pub struct HookInput {
    /// The tool name (`run_command`).
    pub tool: String,
    /// The tool arguments.
    pub args: Value,
    /// The conversation id, when given.
    pub conversation_id: Option<String>,
    /// The step index, when given.
    pub step_idx: Option<u64>,
}

/// What a tool call means to the policy engine.
#[derive(Debug, Clone, PartialEq)]
pub enum Mapped {
    /// A command, file read or file write the engine can classify.
    Operation(Operation),
    /// A tool the agent uses on itself; it touches nothing outside the conversation.
    Internal,
    /// Anything else: asked about, never allowed.
    Unknown(String),
}

/// Parses the standard input. Empty or malformed input, a missing tool call and a missing tool name are errors (denied).
pub fn parse(text: &str) -> Result<HookInput, String> {
    if text.trim().is_empty() {
        return Err("the input is empty".to_owned());
    }
    let value: Value =
        serde_json::from_str(text).map_err(|e| format!("the input is not JSON: {e}"))?;
    let call = value.get("toolCall").ok_or("the input has no toolCall")?;
    let tool = call
        .get("name")
        .and_then(Value::as_str)
        .filter(|n| !n.is_empty())
        .ok_or("the toolCall has no name")?
        .to_owned();
    Ok(HookInput {
        tool,
        args: call.get("args").cloned().unwrap_or(Value::Null),
        conversation_id: value
            .get("conversationId")
            .and_then(Value::as_str)
            .map(str::to_owned),
        step_idx: value.get("stepIdx").and_then(Value::as_u64),
    })
}

fn string_arg(args: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|k| args.get(*k).and_then(Value::as_str))
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
}

const WRITE_PATH_KEYS: &[&str] = &[
    "TargetFile",
    "AbsolutePath",
    "FilePath",
    "Path",
    "File",
    "NotebookPath",
];
const READ_PATH_KEYS: &[&str] = &[
    "AbsolutePath",
    "DirectoryPath",
    "SearchPath",
    "SearchDirectory",
    "TargetFile",
    "Path",
];

/// Maps the call.
pub fn to_operation(input: &HookInput) -> Mapped {
    match input.tool.as_str() {
        "run_command" => match string_arg(&input.args, &["CommandLine", "Command"]) {
            Some(text) => Mapped::Operation(Operation::Command {
                dialect: Dialect::PowerShell,
                text,
                cwd: string_arg(&input.args, &["Cwd", "WorkingDirectory"]),
            }),
            None => Mapped::Unknown("a command call without a command line".to_owned()),
        },
        "write_to_file"
        | "replace_file_content"
        | "multi_replace_file_content"
        | "sed_file"
        | "notebook_edit" => match string_arg(&input.args, WRITE_PATH_KEYS) {
            Some(path) => Mapped::Operation(Operation::FileWrite { path }),
            None => Mapped::Unknown("a write call without a path".to_owned()),
        },
        "view_file" | "list_dir" | "grep_search" | "find_by_name" => {
            match string_arg(&input.args, READ_PATH_KEYS) {
                Some(path) => Mapped::Operation(Operation::FileRead { path }),
                None => Mapped::Unknown("a read call without a path".to_owned()),
            }
        }
        "finish" | "wait" | "wait_5_seconds" | "command_status" | "list_resources"
        | "list_permissions" => Mapped::Internal,
        other => Mapped::Unknown(format!("the tool `{other}` is not one Vela classifies")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(json: &str) -> Result<HookInput, String> {
        parse(json)
    }

    #[test]
    fn the_verified_input_shape_is_parsed() {
        let i = input(
            r#"{"conversationId":"c-1","workspacePaths":["C:/w"],"transcriptPath":"t","modelName":"m","toolCall":{"name":"run_command","args":{"CommandLine":"git status","Cwd":"C:/w"}},"stepIdx":4}"#,
        )
        .unwrap();
        assert_eq!(i.tool, "run_command");
        assert_eq!(i.conversation_id.as_deref(), Some("c-1"));
        assert_eq!(i.step_idx, Some(4));
        assert!(matches!(
            to_operation(&i),
            Mapped::Operation(Operation::Command { text, cwd: Some(c), .. }) if text == "git status" && c == "C:/w"
        ));
    }

    #[test]
    fn empty_malformed_and_incomplete_input_is_an_error() {
        for bad in [
            "",
            "   ",
            "not json",
            "[]",
            "{}",
            r#"{"toolCall":{}}"#,
            r#"{"toolCall":{"name":""}}"#,
            r#"{"toolCall":5}"#,
        ] {
            assert!(input(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn tools_map_to_reads_writes_commands_internal_or_unknown() {
        let call = |tool: &str, args: &str| {
            to_operation(&HookInput {
                tool: tool.to_owned(),
                args: serde_json::from_str(args).unwrap(),
                conversation_id: None,
                step_idx: None,
            })
        };
        assert!(matches!(
            call("view_file", r#"{"AbsolutePath":"C:/w/a.rs"}"#),
            Mapped::Operation(Operation::FileRead { .. })
        ));
        assert!(matches!(
            call("write_to_file", r#"{"TargetFile":"C:/w/a.rs"}"#),
            Mapped::Operation(Operation::FileWrite { .. })
        ));
        assert!(matches!(
            call("sed_file", r#"{"Path":"C:/w/a.rs"}"#),
            Mapped::Operation(Operation::FileWrite { .. })
        ));
        assert!(matches!(call("finish", "{}"), Mapped::Internal));
        assert!(matches!(call("write_to_file", "{}"), Mapped::Unknown(_)));
        assert!(matches!(
            call("run_command", r#"{"CommandLine":"  "}"#),
            Mapped::Unknown(_)
        ));
        assert!(matches!(
            call("browser_click_element", "{}"),
            Mapped::Unknown(_)
        ));
        assert!(matches!(call("run_workflow", "{}"), Mapped::Unknown(_)));
    }
}
