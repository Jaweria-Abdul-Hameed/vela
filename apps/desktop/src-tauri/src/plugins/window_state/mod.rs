//! The `window_state` plugin (L04): the main window's size, position and maximized state persist across launches. The
//! plugin runs in the core, so the renderer needs no permission for it.

use tauri::{Builder, Runtime};
use tauri_plugin_window_state::StateFlags;

/// The plugin's name in the sorted registry in `plugins/mod.rs`.
pub const NAME: &str = "window_state";

/// Registers the plugin. Visibility is not restored (the window decides that at startup).
pub fn install<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    builder.plugin(
        tauri_plugin_window_state::Builder::default()
            .with_state_flags(StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED)
            .build(),
    )
}
