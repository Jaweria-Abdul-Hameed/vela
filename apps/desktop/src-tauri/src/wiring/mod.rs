//! Composition root wiring (`SHARED_SURFACE_PROTOCOL.md` section 5). One file per concern, each owned by one ticket
//! (the declaration list below is pre-declared by F01 and never edited by a ticket).
//!
//! [`compose`] is the one place that assembles the shell. It calls two registries that are pre-declared by F04 in
//! sorted order, so a ticket that adds a command group or a plugin edits only its own file: [`crate::plugins`] and
//! [`crate::commands`].

pub mod adapters;
pub mod antigravity;
pub mod approval;
pub mod orchestrator;
pub mod project;
pub mod settings;
pub mod store;

use tauri::{Builder, Runtime};

/// Applies every registered plugin, then the command dispatcher, to `builder`.
pub fn compose<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    crate::commands::install(crate::plugins::install_all(builder))
}
