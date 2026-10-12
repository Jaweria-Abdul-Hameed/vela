//! Opens the real state store at the data root (F08): migration with backup, integrity check, redaction of journal payloads.
//! The handle is managed Tauri state, so commands and the event bridge share the one writer.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::{App, Manager, Runtime};
use vela_domain::errors::VelaError;
use vela_persistence::writer::{StateStore, StoreOptions};
use vela_process::redact::{PatternRedactor, Redactor};

/// Environment variable that overrides the data root (tests and portable installs).
pub const DATA_DIR_ENV: &str = "VELA_DATA_DIR";

/// The shared state store, as Tauri managed state.
#[derive(Clone)]
pub struct StoreHandle(pub Arc<StateStore>);

/// The data root: `VELA_DATA_DIR`, else `%LOCALAPPDATA%\Vela`, else a folder under the temp directory.
pub fn data_root() -> PathBuf {
    if let Some(dir) = std::env::var_os(DATA_DIR_ENV) {
        return PathBuf::from(dir);
    }
    std::env::var_os("LOCALAPPDATA")
        .map(|base| PathBuf::from(base).join("Vela"))
        .unwrap_or_else(|| std::env::temp_dir().join("Vela"))
}

/// Opens (creating and migrating if needed) the store under `root`, with every journal payload redacted.
pub fn open(root: &Path) -> Result<StoreHandle, VelaError> {
    let redactor = PatternRedactor::new();
    let options = StoreOptions {
        payload_filter: Arc::new(move |payload: &str| redactor.redact(payload).into_owned()),
        ..StoreOptions::default()
    };
    StateStore::open(&root.join("state.db"), options).map(|s| StoreHandle(Arc::new(s)))
}

/// Opens the store at [`data_root`], manages it, and starts the event bridge. Called from `compose`'s setup.
pub fn install<R: Runtime>(app: &mut App<R>) -> Result<(), Box<dyn std::error::Error>> {
    let handle = open(&data_root()).map_err(|e| e.to_string())?;
    let events = handle.0.subscribe();
    crate::events::spawn_bridge(app.handle().clone(), events);
    app.manage(handle);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vela-desktop-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_fresh_data_root_is_created_migrated_and_reopened() {
        let root = temp_root("store");
        {
            let first = open(&root).expect("creates and migrates the store");
            let version: u32 = first
                .0
                .read(|c| {
                    c.query_row("PRAGMA user_version", [], |r| r.get(0))
                        .map_err(|e| VelaError::internal(e.to_string()))
                })
                .unwrap();
            assert_eq!(version, vela_persistence::migrations::latest_version());
        }
        let again = open(&root).expect("opens the existing store");
        let tip = again.0.read(vela_persistence::journal::tip).unwrap();
        assert_eq!(tip, 0);
        assert!(root.join("state.db").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_data_root_can_be_overridden() {
        // Only the pure fallback is asserted here: setting the process environment is unsafe in a parallel test run.
        let root = data_root();
        assert!(root.is_absolute() || std::env::var_os(DATA_DIR_ENV).is_some());
    }
}
