//! The `intervention` command group: Interventions: what needs a person, and resolving it.
//!
//! Generated from one table together with its permission file and TypeScript contract. Every handler only forwards to the
//! application API in `vela-core`, where the behaviour is tested.

use serde_json::Value;
use tauri::{Runtime, State, ipc::Invoke};
use vela_domain::errors::VelaError;

use crate::wiring::orchestrator::ApiHandle;

/// Dotted names of this group's commands (the wire name replaces `.` with `_`).
pub const COMMANDS: &[&str] = &["intervention.list", "intervention.resolve"];

/// `intervention.list`: Open interventions of a run.
#[tauri::command]
pub async fn intervention_list(api: State<'_, ApiHandle>, run_id: String) -> Result<Value, VelaError> {
    api.0.intervention_list(&run_id)
}

/// `intervention.resolve`: Resolves an intervention; the run continues.
#[tauri::command]
pub async fn intervention_resolve(api: State<'_, ApiHandle>, intervention_id: String, resolution: String) -> Result<(), VelaError> {
    api.0.intervention_resolve(&intervention_id, &resolution).await
}

/// Runs the command in `invoke` if this group owns it; returns `false` when it does not.
pub fn handle<R: Runtime>(invoke: Invoke<R>) -> bool {
    run(tauri::generate_handler![intervention_list, intervention_resolve], invoke)
}

/// Gives the handler closure its parameter type from the bound, as `Builder::invoke_handler` does.
fn run<R: Runtime>(handler: impl Fn(Invoke<R>) -> bool, invoke: Invoke<R>) -> bool {
    handler(invoke)
}
