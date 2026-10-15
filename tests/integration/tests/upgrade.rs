//! Z02 (AT-024): upgrading from build N-1 to build N keeps the state store, applies migrations after a backup, and rolls back a
//! failed migration. Build N-1 is modelled by the migration ladder without its newest step; build N is the full ladder.

use std::path::PathBuf;

use vela_domain::errors::ErrorCode;
use vela_persistence::migrations::MIGRATIONS;
use vela_persistence::recovery::{backup_dir, list_backups};
use vela_persistence::repositories::f06_core;
use vela_persistence::writer::{StateStore, StoreOptions};

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vela-upgrade-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn n_minus_one() -> Vec<(u32, &'static str, &'static str)> {
    let mut ladder: Vec<_> = MIGRATIONS.to_vec();
    ladder.pop();
    ladder
}

fn projects(store: &StateStore) -> Vec<String> {
    store
        .read(|c| {
            let mut s = c.prepare("SELECT id FROM projects ORDER BY id").unwrap();
            Ok(s.query_map([], |r| r.get::<_, String>(0))
                .unwrap()
                .map(Result::unwrap)
                .collect())
        })
        .unwrap()
}

#[test]
fn the_upgrade_keeps_the_data_and_backs_up_before_migrating() {
    let d = dir("keep");
    let db = d.join("state.db");
    let old = n_minus_one();
    {
        let store = StateStore::open_with(&db, StoreOptions::default(), &old).unwrap();
        store
            .write(|tx| f06_core::insert_project(tx.conn(), "p1", "demo", "C:/demo", 1))
            .unwrap();
        // Just before the update is applied, the application backs the store up.
        let backup = store.backup_now(1_000).unwrap();
        assert!(backup.exists());
        assert!(list_backups(&db).contains(&backup));
    }
    let store = StateStore::open(&db, StoreOptions::default()).unwrap();
    assert_eq!(projects(&store), ["p1"], "the state survived the upgrade");
    let backups: i64 = store
        .read(|c| {
            Ok(
                c.query_row("SELECT COUNT(*) FROM state_store_backups", [], |r| r.get(0))
                    .unwrap(),
            )
        })
        .unwrap();
    assert_eq!(backups, 1, "migrating took its own backup first");
    let applied = store.read(vela_persistence::migrations::applied).unwrap();
    assert_eq!(
        applied.into_iter().max(),
        MIGRATIONS.iter().map(|m| m.0).max()
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_failed_migration_rolls_back_and_the_old_build_still_opens_the_store() {
    let d = dir("rollback");
    let db = d.join("state.db");
    let old = n_minus_one();
    {
        let store = StateStore::open_with(&db, StoreOptions::default(), &old).unwrap();
        store
            .write(|tx| f06_core::insert_project(tx.conn(), "p1", "demo", "C:/demo", 1))
            .unwrap();
    }
    let mut broken = MIGRATIONS.to_vec();
    broken.push((
        9_000,
        "9000_bad.sql",
        "CREATE TABLE half (x INTEGER); NOT SQL;",
    ));
    let err = StateStore::open_with(&db, StoreOptions::default(), &broken).unwrap_err();
    assert_eq!(err.code(), Some(ErrorCode::MigrationFailed));
    assert!(backup_dir(&db).exists());
    // Rolled back: the previous build opens it and sees exactly what it had.
    let store = StateStore::open_with(&db, StoreOptions::default(), &old).unwrap();
    assert_eq!(projects(&store), ["p1"]);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn the_old_build_refuses_a_store_the_new_build_has_migrated() {
    let d = dir("refuse");
    let db = d.join("state.db");
    {
        let _ = StateStore::open(&db, StoreOptions::default()).unwrap();
    }
    let refused = StateStore::open_with(&db, StoreOptions::default(), &n_minus_one()).unwrap_err();
    assert_eq!(refused.code(), Some(ErrorCode::MigrationFailed));
    let _ = std::fs::remove_dir_all(&d);
}
