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

/// Backups of `db_path`, newest first (by modification time, then name).
pub fn list_backups(db_path: &Path) -> Vec<PathBuf> {
    let Ok(dir) = fs::read_dir(backup_dir(db_path)) else {
        return Vec::new();
    };
    let mut found: Vec<(std::time::SystemTime, PathBuf)> = dir
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "db"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    found.sort_by(|a, b| b.cmp(a));
    found.into_iter().map(|(_, p)| p).collect()
}

/// Keeps the newest `keep` backups and deletes the rest (done on clean shutdown; the latest three are retained).
pub fn rotate(db_path: &Path, keep: usize) -> Result<Vec<PathBuf>, VelaError> {
    let mut removed = Vec::new();
    for old in list_backups(db_path).into_iter().skip(keep) {
        fs::remove_file(&old).map_err(store_error)?;
        removed.push(old);
    }
    Ok(removed)
}

/// What starting on a damaged or missing store came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreRecovery {
    /// The store passed its integrity check; nothing was done.
    Healthy,
    /// The store was replaced by this backup.
    Restored(PathBuf),
    /// No backup passed its integrity check: the caller rebuilds a read-only inventory from Git (R05).
    Unrecoverable,
}

fn passes(path: &Path) -> bool {
    let Ok(conn) = Connection::open(path) else {
        return false;
    };
    let sqlite_ok = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
        .is_ok_and(|s| s == "ok");
    if !sqlite_ok {
        return false;
    }
    // A Vela store also has to be consistent with its own journal; a file without one (a fresh or foreign database) is judged
    // by SQLite alone.
    let has_journal: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'event_journal'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .is_ok_and(|n| n > 0);
    !has_journal || matches!(crate::integrity::check(&conn), Ok(issues) if issues.is_empty())
}

/// Checks the store and, when it is missing or damaged, restores the newest backup that itself passes the check. The damaged
/// file is kept beside it (`.damaged`) rather than deleted. The caller must hold no connection.
pub fn recover_store(db_path: &Path) -> Result<StoreRecovery, VelaError> {
    if db_path.exists() && passes(db_path) {
        return Ok(StoreRecovery::Healthy);
    }
    for candidate in list_backups(db_path) {
        if !passes(&candidate) {
            continue;
        }
        if db_path.exists() {
            let mut kept = db_path.as_os_str().to_os_string();
            kept.push(".damaged");
            fs::rename(db_path, PathBuf::from(kept)).map_err(store_error)?;
        }
        restore(db_path, &candidate)?;
        return Ok(StoreRecovery::Restored(candidate));
    }
    Ok(StoreRecovery::Unrecoverable)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_db(path: &Path, marker: &str) {
        let c = Connection::open(path).unwrap();
        c.execute_batch(&format!(
            "CREATE TABLE t (v TEXT); INSERT INTO t VALUES ('{marker}');"
        ))
        .unwrap();
    }

    fn value(path: &Path) -> String {
        Connection::open(path)
            .unwrap()
            .query_row("SELECT v FROM t", [], |r| r.get(0))
            .unwrap()
    }

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("vela-r05-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn rotation_keeps_the_latest_three() {
        let d = dir("rotate");
        let db = d.join("vela.db");
        make_db(&db, "x");
        let conn = Connection::open(&db).unwrap();
        for n in 0..5 {
            backup(&conn, &db, 1, 1000 + n).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        assert_eq!(rotate(&db, 3).unwrap().len(), 2);
        let left = list_backups(&db);
        assert_eq!(left.len(), 3);
        assert!(left[0].to_string_lossy().contains("1004"));
    }

    #[test]
    fn a_healthy_store_is_left_alone() {
        let d = dir("healthy");
        let db = d.join("vela.db");
        make_db(&db, "keep");
        assert_eq!(recover_store(&db).unwrap(), StoreRecovery::Healthy);
        assert_eq!(value(&db), "keep");
    }

    #[test]
    fn a_corrupt_store_is_replaced_by_the_newest_good_backup_and_kept_aside() {
        let d = dir("corrupt");
        let db = d.join("vela.db");
        make_db(&db, "good");
        let conn = Connection::open(&db).unwrap();
        backup(&conn, &db, 1, 1).unwrap();
        drop(conn);
        std::thread::sleep(std::time::Duration::from_millis(15));
        // A newer backup that is itself garbage must be skipped.
        fs::write(
            backup_dir(&db).join("pre-v1-2.db"),
            b"not a database at all",
        )
        .unwrap();
        fs::write(&db, b"corrupted bytes, definitely not sqlite").unwrap();
        let r = recover_store(&db).unwrap();
        assert!(
            matches!(r, StoreRecovery::Restored(p) if p.to_string_lossy().contains("pre-v1-1"))
        );
        assert_eq!(value(&db), "good");
        assert!(d.join("vela.db.damaged").exists());
    }

    #[test]
    fn a_missing_store_with_no_usable_backup_is_unrecoverable() {
        let d = dir("lost");
        let db = d.join("vela.db");
        assert_eq!(recover_store(&db).unwrap(), StoreRecovery::Unrecoverable);
        fs::create_dir_all(backup_dir(&db)).unwrap();
        fs::write(backup_dir(&db).join("pre-v1-1.db"), b"junk").unwrap();
        assert_eq!(recover_store(&db).unwrap(), StoreRecovery::Unrecoverable);
    }
}
