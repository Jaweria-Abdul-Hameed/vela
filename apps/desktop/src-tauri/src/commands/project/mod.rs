//! The `project` command group: Projects: import a repository folder, list, remove, and choose where its issues live.
//!
//! Generated from one table together with its permission file and TypeScript contract. Every handler only forwards to the
//! application API in `vela-core`, where the behaviour is tested.

use serde_json::Value;
use tauri::{Runtime, State, ipc::Invoke};
use vela_domain::errors::VelaError;

use crate::wiring::orchestrator::ApiHandle;

/// Dotted names of this group's commands (the wire name replaces `.` with `_`).
pub const COMMANDS: &[&str] = &[
    "project.import",
    "project.list",
    "project.remove",
    "project.set_tracker",
];

/// `project.import`: Imports the repository in `folder`.
#[tauri::command]
pub async fn project_import(api: State<'_, ApiHandle>, folder: String) -> Result<Value, VelaError> {
    api.0.project_import(&folder).await
}

/// `project.list`: Recent projects.
#[tauri::command]
pub async fn project_list(api: State<'_, ApiHandle>) -> Result<Value, VelaError> {
    api.0.project_list()
}

/// `project.remove`: Removes a project from Vela (never touches the repository).
#[tauri::command]
pub async fn project_remove(
    api: State<'_, ApiHandle>,
    project_id: String,
) -> Result<(), VelaError> {
    api.0.project_remove(&project_id)
}

/// `project.set_tracker`: Chooses the issue source: `local:<folder>` or `github:<owner>/<name>`.
#[tauri::command]
pub async fn project_set_tracker(
    api: State<'_, ApiHandle>,
    project_id: String,
    spec: String,
) -> Result<(), VelaError> {
    api.0.project_set_tracker(&project_id, &spec)
}

/// Runs the command in `invoke` if this group owns it; returns `false` when it does not.
pub fn handle<R: Runtime>(invoke: Invoke<R>) -> bool {
    run(
        tauri::generate_handler![
            project_import,
            project_list,
            project_remove,
            project_set_tracker
        ],
        invoke,
    )
}

/// Gives the handler closure its parameter type from the bound, as `Builder::invoke_handler` does.
fn run<R: Runtime>(handler: impl Fn(Invoke<R>) -> bool, invoke: Invoke<R>) -> bool {
    handler(invoke)
}
