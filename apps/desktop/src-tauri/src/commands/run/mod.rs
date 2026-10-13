//! The `run` command group: Runs: prepare (analyze the issues), build, stop, resume and status.
//!
//! Generated from one table together with its permission file and TypeScript contract. Every handler only forwards to the
//! application API in `vela-core`, where the behaviour is tested.

use serde_json::Value;
use tauri::{Runtime, State, ipc::Invoke};
use vela_domain::errors::VelaError;

use crate::wiring::orchestrator::ApiHandle;

/// Dotted names of this group's commands (the wire name replaces `.` with `_`).
pub const COMMANDS: &[&str] = &[
    "run.prepare",
    "run.build",
    "run.stop",
    "run.resume",
    "run.status",
];

/// `run.prepare`: Creates a run and analyzes the issues into a graph.
#[tauri::command]
pub async fn run_prepare(
    api: State<'_, ApiHandle>,
    project_id: String,
) -> Result<Value, VelaError> {
    api.0.run_prepare(&project_id).await
}

/// `run.build`: Approves the graph and starts building.
#[tauri::command]
pub async fn run_build(
    api: State<'_, ApiHandle>,
    run_id: String,
    expected_base: Option<String>,
    acknowledge: bool,
) -> Result<(), VelaError> {
    api.0
        .run_build(&run_id, expected_base.as_deref(), acknowledge)
        .await
}

/// `run.stop`: Stop All.
#[tauri::command]
pub async fn run_stop(api: State<'_, ApiHandle>, run_id: String) -> Result<Value, VelaError> {
    api.0.run_stop(&run_id).await
}

/// `run.resume`: Reconciles, then resumes.
#[tauri::command]
pub async fn run_resume(api: State<'_, ApiHandle>, run_id: String) -> Result<Value, VelaError> {
    api.0.run_resume(&run_id).await
}

/// `run.status`: Everything the run screen shows.
#[tauri::command]
pub async fn run_status(api: State<'_, ApiHandle>, run_id: String) -> Result<Value, VelaError> {
    api.0.run_status(&run_id)
}

/// Runs the command in `invoke` if this group owns it; returns `false` when it does not.
pub fn handle<R: Runtime>(invoke: Invoke<R>) -> bool {
    run(
        tauri::generate_handler![run_prepare, run_build, run_stop, run_resume, run_status],
        invoke,
    )
}

/// Gives the handler closure its parameter type from the bound, as `Builder::invoke_handler` does.
fn run<R: Runtime>(handler: impl Fn(Invoke<R>) -> bool, invoke: Invoke<R>) -> bool {
    handler(invoke)
}
