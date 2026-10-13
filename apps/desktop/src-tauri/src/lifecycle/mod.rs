//! Window lifecycle (L02): closing the window never silently abandons work.
//!
//! With background operation on (an opt-in, persisted setting), closing hides the window and the run keeps going; the tray brings
//! it back. With it off, closing while a run is active pauses it safely (Stop All) before the application exits.

use tauri::{App, Manager, Runtime, WindowEvent};

use crate::wiring::orchestrator::ApiHandle;

/// What closing the window should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnClose {
    /// Just close.
    Exit,
    /// Hide the window; orchestration continues.
    HideToTray,
    /// Stop everything safely, then exit.
    PauseThenExit,
}

/// The decision, from the background setting and whether any run is active.
pub fn on_close(background: bool, running_runs: usize) -> OnClose {
    if background {
        OnClose::HideToTray
    } else if running_runs > 0 {
        OnClose::PauseThenExit
    } else {
        OnClose::Exit
    }
}

/// Installs the close handler on the main window.
pub fn install<R: Runtime>(app: &App<R>) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let handle = app.handle().clone();
    window.on_window_event(move |event| {
        let WindowEvent::CloseRequested { api, .. } = event else {
            return;
        };
        let Some(state) = handle.try_state::<ApiHandle>() else {
            return;
        };
        let background = state
            .0
            .settings_get(None)
            .ok()
            .is_some_and(|s| s.get("background").and_then(|v| v.as_str()) == Some("true"));
        let running = state.0.active_runs().map_or(0, |r| r.len());
        match on_close(background, running) {
            OnClose::Exit => {}
            OnClose::HideToTray => {
                api.prevent_close();
                if let Some(w) = handle.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            OnClose::PauseThenExit => {
                api.prevent_close();
                let handle = handle.clone();
                tauri::async_runtime::spawn(async move {
                    if let Some(state) = handle.try_state::<ApiHandle>() {
                        let _ = state.0.stop_all_runs().await;
                    }
                    handle.exit(0);
                });
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_never_abandons_a_running_run() {
        assert_eq!(on_close(true, 3), OnClose::HideToTray);
        assert_eq!(on_close(true, 0), OnClose::HideToTray);
        assert_eq!(on_close(false, 2), OnClose::PauseThenExit);
        assert_eq!(on_close(false, 0), OnClose::Exit);
    }
}
