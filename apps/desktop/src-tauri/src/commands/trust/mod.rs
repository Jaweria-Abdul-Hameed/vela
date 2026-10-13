//! The `trust` command group: Repository trust: what trusting would make executable, grant with a confirmed profile, revoke.
//!
//! Generated from one table together with its permission file and TypeScript contract. Every handler only forwards to the
//! application API in `vela-core`, where the behaviour is tested.

use serde_json::Value;
use tauri::{Runtime, State, ipc::Invoke};
use vela_domain::errors::VelaError;

use crate::wiring::orchestrator::ApiHandle;

/// Dotted names of this group's commands (the wire name replaces `.` with `_`).
pub const COMMANDS: &[&str] = &["trust.propose", "trust.grant", "trust.revoke"];

/// `trust.propose`: What trusting would make executable.
#[tauri::command]
pub async fn trust_propose(api: State<'_, ApiHandle>, project_id: String) -> Result<Value, VelaError> {
    api.0.trust_propose(&project_id)
}

/// `trust.grant`: Trusts the repository with the confirmed command profile.
#[tauri::command]
pub async fn trust_grant(api: State<'_, ApiHandle>, project_id: String, commands: Vec<vela_core::trust::ProfileCommand>) -> Result<(), VelaError> {
    api.0.trust_grant(&project_id, &commands)
}

/// `trust.revoke`: Revokes trust.
#[tauri::command]
pub async fn trust_revoke(api: State<'_, ApiHandle>, project_id: String) -> Result<(), VelaError> {
    api.0.trust_revoke(&project_id)
}

/// Runs the command in `invoke` if this group owns it; returns `false` when it does not.
pub fn handle<R: Runtime>(invoke: Invoke<R>) -> bool {
    run(tauri::generate_handler![trust_propose, trust_grant, trust_revoke], invoke)
}

/// Gives the handler closure its parameter type from the bound, as `Builder::invoke_handler` does.
fn run<R: Runtime>(handler: impl Fn(Invoke<R>) -> bool, invoke: Invoke<R>) -> bool {
    handler(invoke)
}
