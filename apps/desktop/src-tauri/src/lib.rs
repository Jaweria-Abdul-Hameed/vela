//! vela-desktop: module skeleton (F01). Modules are empty and implemented by the tickets that own them;
//! see docs/issues/graph/SHARED_SURFACE_PROTOCOL.md section 1. Never edit this declaration list in a ticket.

pub mod autostart;
pub mod commands;
pub mod events;
pub mod lifecycle;
pub mod notify;
pub mod plugins;
pub mod tray;
pub mod update;
pub mod window;
pub mod wiring;

// The release-profile guard shared with build.rs (F04), compiled here only so that its tests run.
#[cfg(test)]
#[path = "../build_policy.rs"]
mod build_policy;

/// Runs the desktop application: composes the shell (plugins, commands) and starts Tauri. Called by `main`.
pub fn run() {
    wiring::compose(tauri::Builder::default())
        .run(tauri::generate_context!())
        .expect("error while running the Vela desktop application");
}

#[cfg(test)]
mod tests {
    #[test]
    fn bootstrap_placeholder() {}
}
