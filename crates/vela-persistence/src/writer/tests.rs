use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use vela_domain::errors::ErrorCode;
use vela_domain::events::{Event, TicketReadyPayload};
use vela_domain::ids::RunId;

use super::*;
use crate::repositories::f06_core as repo;

const CHILD_ENV: &str = "VELA_PERSISTENCE_TEST_CHILD";

struct TestDir(PathBuf);

impl TestDir {
    fn new(tag: &str) -> Self {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "vela-persistence-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn db(&self) -> PathBuf {
        self.0.join("state.db")
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn ready(run: &RunId, frontier: u32) -> NewEvent {
    NewEvent {
        ts: 1_000,
        run_id: Some(run.clone()),
        ticket_id: None,
        worker_id: None,
        event: Event::TicketReady(TicketReadyPayload {
            frontier_size: frontier,
        }),
    }
}

fn seed_run(store: &StateStore, run: &RunId) {
    let run = run.clone();
    store
        .write(move |tx| {
            repo::insert_project(tx.conn(), "p1", "demo", "C:/demo", 1)?;
            repo::insert_run(tx.conn(), run.as_str(), "p1", "CREATED", 1)?;
            Ok(())
        })
        .unwrap();
}

#[test]
fn a_transition_writes_state_and_journal_together_and_seq_is_monotonic() {
    let dir = TestDir::new("atomic");
    let store = StateStore::open(&dir.db(), StoreOptions::default()).unwrap();
    let run = RunId::new("run-1").unwrap();
    seed_run(&store, &run);
    let mut last = 0;
    for i in 1..=5u32 {
        let run = run.clone();
        let seq = store
            .write(move |tx| {
                repo::set_run_state(tx.conn(), run.as_str(), "RUNNING", 2)?;
                Ok(tx.append(&ready(&run, i))?.seq)
            })
            .unwrap();
        assert!(seq > last, "seq must increase: {seq} after {last}");
        last = seq;
    }
    let (tip, row_seq) = store
        .read(|c| {
            Ok((
                journal::tip(c)?,
                c.query_row(
                    "SELECT last_event_seq FROM build_runs WHERE id='run-1'",
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap(),
            ))
        })
        .unwrap();
    assert_eq!(tip, last);
    assert_eq!(u64::try_from(row_seq).unwrap(), last);
    let events = store.read(|c| journal::read_after(c, 0, 100)).unwrap();
    assert_eq!(events.len(), 5);
    assert!(events.windows(2).all(|w| w[0].seq < w[1].seq));
    let conn = Connection::open(dir.db()).unwrap();
    assert!(integrity::check(&conn).unwrap().is_empty());
}

#[test]
fn a_failed_transition_rolls_back_state_and_journal_and_publishes_nothing() {
    let dir = TestDir::new("rollback");
    let store = StateStore::open(&dir.db(), StoreOptions::default()).unwrap();
    let run = RunId::new("run-1").unwrap();
    seed_run(&store, &run);
    let rx = store.subscribe();
    let r = run.clone();
    let result: Result<(), _> = store.write(move |tx| {
        repo::set_run_state(tx.conn(), r.as_str(), "RUNNING", 2)?;
        tx.append(&ready(&r, 1))?;
        Err(store_error("boom"))
    });
    assert!(result.is_err());
    assert_eq!(store.read(journal::tip).unwrap(), 0);
    assert_eq!(
        store
            .read(|c| repo::run_state(c, "run-1"))
            .unwrap()
            .as_deref(),
        Some("CREATED")
    );
    assert!(
        rx.recv_timeout(Duration::from_millis(100)).is_err(),
        "nothing may publish"
    );
}

#[test]
fn events_publish_only_after_commit_and_in_order() {
    let dir = TestDir::new("publish");
    let store = StateStore::open(&dir.db(), StoreOptions::default()).unwrap();
    let run = RunId::new("run-1").unwrap();
    seed_run(&store, &run);
    let rx = store.subscribe();
    let r = run.clone();
    store
        .write(move |tx| {
            tx.append(&ready(&r, 1))?;
            tx.append(&ready(&r, 2))?;
            Ok(())
        })
        .unwrap();
    let a = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let b = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!((a.seq, b.seq), (1, 2));
    // Already committed when the subscriber sees it.
    assert_eq!(store.read(journal::tip).unwrap(), 2);
}

#[test]
fn the_journal_is_append_only() {
    let dir = TestDir::new("append-only");
    let store = StateStore::open(&dir.db(), StoreOptions::default()).unwrap();
    let run = RunId::new("run-1").unwrap();
    seed_run(&store, &run);
    let r = run.clone();
    store
        .write(move |tx| tx.append(&ready(&r, 1)).map(|_| ()))
        .unwrap();
    let upd: Result<(), _> = store.write(|tx| {
        tx.conn()
            .execute("UPDATE event_journal SET kind = 'X'", [])
            .map(|_| ())
            .map_err(store_error)
    });
    assert!(upd.is_err());
    let del: Result<(), _> = store.write(|tx| {
        tx.conn()
            .execute("DELETE FROM event_journal", [])
            .map(|_| ())
            .map_err(store_error)
    });
    assert!(del.is_err());
}

#[test]
fn payloads_pass_through_the_filter_before_they_are_stored() {
    let dir = TestDir::new("filter");
    let options = StoreOptions {
        payload_filter: Arc::new(|s: &str| s.replace("\"frontier_size\":7", "\"frontier_size\":0")),
        ..StoreOptions::default()
    };
    let store = StateStore::open(&dir.db(), options).unwrap();
    let run = RunId::new("run-1").unwrap();
    seed_run(&store, &run);
    let r = run.clone();
    store
        .write(move |tx| tx.append(&ready(&r, 7)).map(|_| ()))
        .unwrap();
    let raw: String = store
        .read(|c| {
            Ok(
                c.query_row("SELECT payload FROM event_journal", [], |r| r.get(0))
                    .unwrap(),
            )
        })
        .unwrap();
    assert!(raw.contains("\"frontier_size\":0"), "{raw}");
}

#[test]
fn reopening_keeps_data_and_does_not_back_up_when_nothing_is_pending() {
    let dir = TestDir::new("reopen");
    {
        let store = StateStore::open(&dir.db(), StoreOptions::default()).unwrap();
        seed_run(&store, &RunId::new("run-1").unwrap());
    }
    let store = StateStore::open(&dir.db(), StoreOptions::default()).unwrap();
    assert_eq!(
        store
            .read(|c| repo::run_state(c, "run-1"))
            .unwrap()
            .as_deref(),
        Some("CREATED")
    );
    assert!(!recovery::backup_dir(&dir.db()).exists());
}

#[test]
fn a_failing_migration_restores_the_backup_and_refuses_newer_logic() {
    let dir = TestDir::new("migfail");
    {
        let store = StateStore::open(&dir.db(), StoreOptions::default()).unwrap();
        seed_run(&store, &RunId::new("run-1").unwrap());
    }
    let good = migrations::MIGRATIONS;
    let mut ladder: Vec<(u32, &str, &str)> = good.to_vec();
    ladder.push((
        900,
        "0900_test_ok.sql",
        "CREATE TABLE added_ok (x INTEGER);",
    ));
    ladder.push((
        901,
        "0901_test_bad.sql",
        "CREATE TABLE half (x INTEGER); THIS IS NOT SQL;",
    ));
    let err = StateStore::open_with(&dir.db(), StoreOptions::default(), &ladder).unwrap_err();
    assert_eq!(err.code(), Some(ErrorCode::MigrationFailed));
    assert!(
        recovery::backup_dir(&dir.db()).exists(),
        "a backup is taken before migrating"
    );

    // The database is back to its pre-migration state: no new tables, original data intact, and the older ladder still opens it.
    let store = StateStore::open(&dir.db(), StoreOptions::default()).unwrap();
    let tables: Vec<String> = store
        .read(|c| {
            let mut s = c
                .prepare("SELECT name FROM sqlite_master WHERE type='table'")
                .unwrap();
            Ok(s.query_map([], |r| r.get(0))
                .unwrap()
                .map(Result::unwrap)
                .collect())
        })
        .unwrap();
    assert!(
        !tables.iter().any(|t| t == "added_ok" || t == "half"),
        "{tables:?}"
    );
    assert_eq!(
        store
            .read(|c| repo::run_state(c, "run-1"))
            .unwrap()
            .as_deref(),
        Some("CREATED")
    );
    drop(store);

    // A good newer ladder applies, records a backup row, and then the older build refuses the newer state.
    let mut newer: Vec<(u32, &str, &str)> = good.to_vec();
    newer.push((
        900,
        "0900_test_ok.sql",
        "CREATE TABLE added_ok (x INTEGER);",
    ));
    let store = StateStore::open_with(&dir.db(), StoreOptions::default(), &newer).unwrap();
    let backups: i64 = store
        .read(|c| {
            Ok(
                c.query_row("SELECT COUNT(*) FROM state_store_backups", [], |r| r.get(0))
                    .unwrap(),
            )
        })
        .unwrap();
    assert_eq!(backups, 1);
    drop(store);
    let refused = StateStore::open(&dir.db(), StoreOptions::default()).unwrap_err();
    assert_eq!(refused.code(), Some(ErrorCode::MigrationFailed));
}

#[test]
fn a_state_journal_mismatch_is_reported_and_never_silently_repaired() {
    let dir = TestDir::new("mismatch");
    {
        let store = StateStore::open(&dir.db(), StoreOptions::default()).unwrap();
        let run = RunId::new("run-1").unwrap();
        seed_run(&store, &run);
        store
            .write(move |tx| tx.append(&ready(&run, 1)).map(|_| ()))
            .unwrap();
    }
    {
        let conn = Connection::open(dir.db()).unwrap();
        conn.execute(
            "UPDATE build_runs SET last_event_seq = 99 WHERE id = 'run-1'",
            [],
        )
        .unwrap();
    }
    let err = StateStore::open(&dir.db(), StoreOptions::default()).unwrap_err();
    assert_eq!(err.code(), Some(ErrorCode::StateStoreCorrupt));
    let conn = Connection::open(dir.db()).unwrap();
    let seq: i64 = conn
        .query_row(
            "SELECT last_event_seq FROM build_runs WHERE id = 'run-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(seq, 99, "the mismatch must not be repaired");
    assert!(matches!(
        integrity::check(&conn).unwrap()[0],
        integrity::IntegrityIssue::RunAheadOfJournal { .. }
    ));
}

#[test]
fn a_corrupted_database_file_is_an_integrity_failure() {
    let dir = TestDir::new("corrupt");
    {
        let store = StateStore::open(&dir.db(), StoreOptions::default()).unwrap();
        seed_run(&store, &RunId::new("run-1").unwrap());
    }
    let mut bytes = std::fs::read(dir.db()).unwrap();
    for b in bytes.iter_mut().skip(4096).take(8192) {
        *b = 0xAB;
    }
    std::fs::write(dir.db(), bytes).unwrap();
    for suffix in ["-wal", "-shm"] {
        let mut p = dir.db().into_os_string();
        p.push(suffix);
        let _ = std::fs::remove_file(PathBuf::from(p));
    }
    assert!(StateStore::open(&dir.db(), StoreOptions::default()).is_err());
}

/// Body of the child process: write state and journal, but die at the armed fault point. A normal run skips it.
#[test]
fn child_writes_then_dies_at_the_fault_point() {
    let Ok(dir) = std::env::var(CHILD_ENV) else {
        return;
    };
    let store = StateStore::open(
        &PathBuf::from(&dir).join("state.db"),
        StoreOptions::default(),
    )
    .unwrap();
    let run = RunId::new("run-2").unwrap();
    let r = run.clone();
    store
        .write(move |tx| {
            repo::insert_project(tx.conn(), "p2", "child", "C:/child", 1)?;
            repo::insert_run(tx.conn(), r.as_str(), "p2", "CREATED", 1)?;
            tx.append(&ready(&r, 1)).map(|_| ())
        })
        .unwrap();
    println!("SURVIVED-THE-FAULT-POINT");
}

fn run_child(dir: &TestDir, armed: &str) -> String {
    let out = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "writer::tests::child_writes_then_dies_at_the_fault_point",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_ENV, &dir.0)
        .env(vela_domain::fault::ARMED_ENV_VAR, armed)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn killed_before_commit_leaves_neither_state_nor_journal() {
    let dir = TestDir::new("kill-before");
    let out = run_child(&dir, "persistence_before_commit");
    assert!(
        !out.contains("SURVIVED"),
        "the child must die at the fault point: {out}"
    );
    let store = StateStore::open(&dir.db(), StoreOptions::default()).unwrap();
    assert_eq!(store.read(journal::tip).unwrap(), 0);
    assert_eq!(store.read(|c| repo::run_state(c, "run-2")).unwrap(), None);
}

#[test]
fn killed_right_after_commit_leaves_both_state_and_journal() {
    let dir = TestDir::new("kill-after");
    let out = run_child(&dir, "persistence_after_commit");
    assert!(
        !out.contains("SURVIVED"),
        "the child must die at the fault point: {out}"
    );
    let store = StateStore::open(&dir.db(), StoreOptions::default()).unwrap();
    assert_eq!(store.read(journal::tip).unwrap(), 1);
    assert_eq!(
        store
            .read(|c| repo::run_state(c, "run-2"))
            .unwrap()
            .as_deref(),
        Some("CREATED")
    );
}

#[test]
fn project_and_settings_repositories_round_trip() {
    let dir = TestDir::new("repos");
    let store = StateStore::open(&dir.db(), StoreOptions::default()).unwrap();
    store
        .write(|tx| {
            repo::insert_project(tx.conn(), "a", "A", "C:/a", 10)?;
            repo::insert_project(tx.conn(), "b", "B", "C:/b", 20)?;
            repo::touch_project(tx.conn(), "a", 30)?;
            repo::set_setting(tx.conn(), "graphics_mode", "balanced", 1)?;
            repo::set_setting(tx.conn(), "graphics_mode", "full", 2)?;
            repo::upsert_operation(tx.conn(), "op-1", None, "merge", "STARTED", "", 1)?;
            repo::upsert_operation(tx.conn(), "op-1", None, "merge", "DONE", "ok", 2)?;
            Ok(())
        })
        .unwrap();
    store
        .read(|c| {
            let recent = repo::recent_projects(c, 10)?;
            assert_eq!(
                recent.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
                vec!["a", "b"]
            );
            assert_eq!(repo::project_by_path(c, "C:/b")?.unwrap().name, "B");
            assert_eq!(
                repo::get_setting(c, "graphics_mode")?.as_deref(),
                Some("full")
            );
            assert_eq!(repo::operation_state(c, "op-1")?.as_deref(), Some("DONE"));
            Ok(())
        })
        .unwrap();
    let dup: Result<(), _> = store.write(|tx| repo::insert_project(tx.conn(), "c", "C", "C:/a", 1));
    assert!(dup.is_err(), "repository path is unique");
}

#[test]
fn a_file_that_is_not_a_database_is_a_corrupt_store_not_a_bug() {
    let dir = std::env::temp_dir().join(format!("vela-notdb-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("state.db");
    std::fs::write(&path, b"damaged bytes, not a sqlite file at all").unwrap();
    let Err(err) = StateStore::open(&path, StoreOptions::default()) else {
        panic!("a file that is not a database cannot be opened");
    };
    assert_eq!(err.code(), Some(ErrorCode::StateStoreCorrupt), "{err:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
