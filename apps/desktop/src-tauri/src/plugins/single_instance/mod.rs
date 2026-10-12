//! The `single_instance` plugin (L04): a second launch hands over to the running instance, which comes to the front.

use tauri::{AppHandle, Builder, Manager, Runtime};

/// The plugin's name in the sorted registry in `plugins/mod.rs`.
pub const NAME: &str = "single_instance";

/// Brings the main window to the front: restores it if minimized, shows it if hidden, focuses it.
pub fn focus_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Registers the plugin: a second launch focuses the first instance and exits.
pub fn install<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        focus_main_window(app);
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::test::{MockRuntime, mock_context, noop_assets};

    #[test]
    fn focusing_works_with_and_without_a_main_window() {
        let app = tauri::test::mock_builder()
            .build(mock_context(noop_assets()))
            .expect("build the mock application");
        focus_main_window::<MockRuntime>(app.handle());
        let _window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("create the main window");
        focus_main_window::<MockRuntime>(app.handle());
    }
}
