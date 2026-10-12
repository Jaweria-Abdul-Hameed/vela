//! The `window_state` plugin (F04 stub). The ticket that needs this plugin replaces `install` with the real registration
//! (and adds its own capability file); no other file changes.

use tauri::{Builder, Runtime};

/// The plugin's name in the sorted registry in `plugins/mod.rs`.
pub const NAME: &str = "window_state";

/// Registers the plugin on the application builder. A stub: registers nothing yet.
pub fn install<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    builder
}
