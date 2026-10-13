//! The `timeline` command group: Timeline: the journal of one ticket.
//!
//! Generated from one table together with its permission file and TypeScript contract. Every handler only forwards to the
//! application API in `vela-core`, where the behaviour is tested.

use serde_json::Value;
use tauri::{Runtime, State, ipc::Invoke};
use vela_domain::errors::VelaError;

use crate::wiring::orchestrator::ApiHandle;

/// Dotted names of this group's commands (the wire name replaces `.` with `_`).
pub const COMMANDS: &[&str] = &["timeline.get"];

/// `timeline.get`: A ticket timeline.
#[tauri::command]
pub async fn timeline_get(api: State<'_, ApiHandle>, run_id: String, ticket: String) -> Result<Value, VelaError> {
    api.0.timeline_get(&run_id, &ticket)
}

/// Runs the command in `invoke` if this group owns it; returns `false` when it does not.
pub fn handle<R: Runtime>(invoke: Invoke<R>) -> bool {
    run(tauri::generate_handler![timeline_get], invoke)
}

/// Gives the handler closure its parameter type from the bound, as `Builder::invoke_handler` does.
fn run<R: Runtime>(handler: impl Fn(Invoke<R>) -> bool, invoke: Invoke<R>) -> bool {
    handler(invoke)
}
