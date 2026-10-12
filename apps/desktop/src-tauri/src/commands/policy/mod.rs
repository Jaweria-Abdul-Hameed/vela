//! The `policy` command group (F04 stub). The ticket that owns the group adds its commands here: list their dotted names
//! in `COMMANDS`, implement them with `#[tauri::command]`, and route them in `handle` with
//! `tauri::generate_handler!` (see `diagnostics`). It also adds `packages/contracts/src/commands/policy.ts` and its own
//! permission file. No other group's file changes.

use tauri::{Runtime, ipc::Invoke};

/// Dotted names of this group's commands (the wire name replaces `.` with `_`).
pub const COMMANDS: &[&str] = &[];

/// Runs the command in `invoke` if this group owns it; returns `false` when it does not.
pub fn handle<R: Runtime>(_invoke: Invoke<R>) -> bool {
    false
}
