//! The `settings` command group: Settings: read the effective settings and change one.
//!
//! Generated from one table together with its permission file and TypeScript contract. Every handler only forwards to the
//! application API in `vela-core`, where the behaviour is tested.

use serde_json::Value;
use tauri::{Runtime, State, ipc::Invoke};
use vela_domain::errors::VelaError;

use crate::wiring::orchestrator::ApiHandle;

/// Dotted names of this group's commands (the wire name replaces `.` with `_`).
pub const COMMANDS: &[&str] = &["settings.get", "settings.set"];

/// `settings.get`: Effective settings.
#[tauri::command]
pub async fn settings_get(api: State<'_, ApiHandle>, project_id: Option<String>) -> Result<Value, VelaError> {
    api.0.settings_get(project_id.as_deref())
}

/// `settings.set`: Sets one setting.
#[tauri::command]
pub async fn settings_set(api: State<'_, ApiHandle>, project_id: Option<String>, key: String, value: String) -> Result<(), VelaError> {
    api.0.settings_set(project_id.as_deref(), &key, &value)
}

/// Runs the command in `invoke` if this group owns it; returns `false` when it does not.
pub fn handle<R: Runtime>(invoke: Invoke<R>) -> bool {
    run(tauri::generate_handler![settings_get, settings_set], invoke)
}

/// Gives the handler closure its parameter type from the bound, as `Builder::invoke_handler` does.
fn run<R: Runtime>(handler: impl Fn(Invoke<R>) -> bool, invoke: Invoke<R>) -> bool {
    handler(invoke)
}
