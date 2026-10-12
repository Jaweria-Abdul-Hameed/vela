//! Backup before migration and restore on failure (F06). The Git-derived inventory rebuild after unrecoverable corruption is
//! R05; this module only protects a migration.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use vela_domain::errors::{ErrorCode, VelaError};

use crate::writer::store_error;

/// Where backups of `db_path` live: `<db file name>.backups` next to it.
pub fn backup_dir(db_path: &Path) -> PathBuf {
    let mut name = db_path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".backups");
    db_path.with_file_name(name)
}

/// Writes a consistent copy of the open database (`VACUUM INTO`, safe with WAL) and returns its path.
pub fn backup(
    conn: &Connection,
    db_path: &Path,
    schema_version: u32,
    now_ms: i64,
) -> Result<PathBuf, VelaError> {
    let dir = backup_dir(db_path);
    fs::create_dir_all(&dir).map_err(store_error)?;
    let target = dir.join(format!("pre-v{schema_version}-{now_ms}.db"));
    if target.exists() {
        fs::remove_file(&target).map_err(store_error)?;
    }
    conn.execute("VACUUM INTO ?1", [target.to_string_lossy().as_ref()])
        .map_err(store_error)?;
    Ok(target)
}

/// Replaces the database files with `backup`. The caller must have closed every connection first.
pub fn restore(db_path: &Path, backup: &Path) -> Result<(), VelaError> {
    for suffix in ["-wal", "-shm"] {
        let mut side = db_path.as_os_str().to_os_string();
        side.push(suffix);
        let side = PathBuf::from(side);
        if side.exists() {
            fs::remove_file(&side).map_err(store_error)?;
        }
    }
    fs::copy(backup, db_path).map_err(|e| {
        VelaError::from_code(
            ErrorCode::StateStoreCorrupt,
            format!(
                "could not restore the state store from {}: {e}",
                backup.display()
            ),
        )
    })?;
    Ok(())
}
