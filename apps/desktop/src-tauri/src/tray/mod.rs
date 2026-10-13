//! The tray icon (L02): reopen, status and Stop All stay one click away while the window is closed, and Quit pauses safely first.
//!
//! The menu only forwards to the application API; what Stop All does is tested in `vela-core`.

use std::time::Duration;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{App, AppHandle, Manager, Runtime};

use crate::wiring::orchestrator::ApiHandle;

/// The tray icon's identifier.
pub const TRAY_ID: &str = "vela";

/// Menu item identifiers.
pub const OPEN: &str = "open";
/// Stop every running run.
pub const STOP: &str = "stop";
/// Pause safely, then quit.
pub const QUIT: &str = "quit";

/// Shows and focuses the main window.
pub fn show_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn status_text(running: usize) -> String {
    match running {
        0 => "Vela: idle".to_owned(),
        1 => "Vela: 1 run active".to_owned(),
        n => format!("Vela: {n} runs active"),
    }
}

/// Builds the tray icon and its menu, and keeps its tooltip showing how many runs are active.
pub fn install<R: Runtime>(app: &App<R>) -> Result<(), Box<dyn std::error::Error>> {
    let Some(icon) = app.default_window_icon().cloned() else {
        tracing::warn!("no application icon, so there is no tray icon");
        return Ok(());
    };
    let open = MenuItem::with_id(app, OPEN, "Open Vela", true, None::<&str>)?;
    let stop = MenuItem::with_id(app, STOP, "Stop all", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT, "Pause safely and quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &stop, &quit])?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("Vela")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            OPEN => show_main(app),
            STOP => {
                if let Some(api) = app.try_state::<ApiHandle>() {
                    let api = api.0.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = api.stop_all_runs().await {
                            tracing::warn!(error = %e.what_failed(), "Stop All from the tray failed");
                        }
                    });
                }
            }
            QUIT => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Some(api) = app.try_state::<ApiHandle>() {
                        let _ = api.0.stop_all_runs().await;
                    }
                    app.exit(0);
                });
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::DoubleClick { .. } = event {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;

    let handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let running = handle
                .try_state::<ApiHandle>()
                .and_then(|api| api.0.active_runs().ok())
                .map_or(0, |runs| runs.len());
            if let Some(tray) = handle.tray_by_id(TRAY_ID) {
                let _ = tray.set_tooltip(Some(status_text(running)));
            }
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_line_counts_runs() {
        assert_eq!(status_text(0), "Vela: idle");
        assert_eq!(status_text(1), "Vela: 1 run active");
        assert_eq!(status_text(3), "Vela: 3 runs active");
    }
}
