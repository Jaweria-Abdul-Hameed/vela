//! Tauri plugin registration (`SHARED_SURFACE_PROTOCOL.md` section 5, F04).
//!
//! Every plugin has its own file (`plugins/<name>.rs`, declared here by F01). A plugin file exposes `NAME` and
//! `install`; `install` is a no-op stub until the ticket that needs the plugin implements it, and only that ticket
//! edits that file. All plugins are registered from the one sorted list in [`registered`], so adding a plugin never
//! edits another plugin's file or this list.

pub mod autostart;
pub mod global_shortcut;
pub mod notification;
pub mod single_instance;
pub mod tray;
pub mod updater;
pub mod window_state;

use tauri::{Builder, Runtime};

/// Applies one plugin to the application builder.
pub type Install<R> = fn(Builder<R>) -> Builder<R>;

/// Every plugin as `(name, install)`, sorted by name.
pub fn registered<R: Runtime>() -> [(&'static str, Install<R>); 7] {
    [
        (autostart::NAME, autostart::install::<R>),
        (global_shortcut::NAME, global_shortcut::install::<R>),
        (notification::NAME, notification::install::<R>),
        (single_instance::NAME, single_instance::install::<R>),
        (tray::NAME, tray::install::<R>),
        (updater::NAME, updater::install::<R>),
        (window_state::NAME, window_state::install::<R>),
    ]
}

/// Applies every registered plugin to `builder`, in sorted order.
pub fn install_all<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    registered::<R>()
        .into_iter()
        .fold(builder, |builder, (_, install)| install(builder))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::test::MockRuntime;

    #[test]
    fn plugins_are_registered_from_one_sorted_unique_list() {
        let names: Vec<&str> = registered::<MockRuntime>().iter().map(|p| p.0).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(names, sorted);
        assert_eq!(
            names,
            [
                "autostart",
                "global_shortcut",
                "notification",
                "single_instance",
                "tray",
                "updater",
                "window_state"
            ]
        );
    }

    #[test]
    fn a_stub_plugin_installs_without_touching_the_builder() {
        // The stubs add nothing, so the composed builder still builds an application.
        let app = install_all(tauri::test::mock_builder())
            .build(tauri::test::mock_context(tauri::test::noop_assets()));
        assert!(app.is_ok());
    }
}
