use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use serde_json::json;
use vela_domain::errors::VelaError;
use vela_domain::events::{Event, RunCreatedPayload, WorkerFailedPayload};
use vela_domain::ids::{BranchName, CommitSha, RunId, TicketId, WorkerId, WorktreePath};
use vela_domain::ports::PortFuture;
use vela_domain::state::{IntegrationHealth, WorkerPhase, WorkerState};
use vela_persistence::journal::NewEvent;
use vela_persistence::repositories::f06_core;
use vela_persistence::writer::{Connection, StoreOptions};

use super::runs::Runs;
use super::supervisor::supervise;
use super::test_support::{env, env_with};
use super::*;

struct Echo;

impl CommandHandler for Echo {
    fn name(&self) -> &'static str {
        "test.echo"
    }
    fn handle<'a>(&'a self, _s: &'a Services, payload: Value) -> PortFuture<'a, Value> {
        Box::pin(async move { Ok(payload) })
    }
}

struct Slow(Arc<AtomicU32>);

impl CommandHandler for Slow {
    fn name(&self) -> &'static str {
        "test.slow"
    }
    fn handle<'a>(&'a self, _s: &'a Services, payload: Value) -> PortFuture<'a, Value> {
        Box::pin(async move {
            // Messages are handled one at a time: no overlap, and in arrival order per sender.
            let before = self.0.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(5)).await;
            assert_eq!(
                self.0.load(Ordering::SeqCst),
                before + 1,
                "another message ran concurrently"
            );
            Ok(payload)
        })
    }
}

struct Boom;

impl CommandHandler for Boom {
    fn name(&self) -> &'static str {
        "test.boom"
    }
    fn handle<'a>(&'a self, _s: &'a Services, _payload: Value) -> PortFuture<'a, Value> {
        Box::pin(async move { panic!("a handler bug") })
    }
}

struct RunsPart;

impl SnapshotContributor for RunsPart {
    fn name(&self) -> &'static str {
        "runs"
    }
    fn contribute(&self, conn: &Connection) -> Result<Value, VelaError> {
        let mut stmt = conn
            .prepare("SELECT id, state FROM build_runs ORDER BY id")
            .map_err(|e| VelaError::internal(e.to_string()))?;
        let rows: Vec<Value> = stmt
            .query_map([], |r| {
                Ok(json!({ "id": r.get::<_, String>(0)?, "state": r.get::<_, String>(1)? }))
            })
            .map_err(|e| VelaError::internal(e.to_string()))?
            .map(Result::unwrap)
            .collect();
        Ok(Value::Array(rows))
    }
}

#[tokio::test]
async fn messages_are_routed_by_name_and_unknown_names_are_typed_errors() {
    let e = env("route");
    let handle = OrchestratorBuilder::new(e.services.clone())
        .command(Arc::new(Echo))
        .spawn(8);
    assert_eq!(
        handle.send("test.echo", json!({"a": 1})).await.unwrap(),
        json!({"a": 1})
    );
    let err = handle.send("test.nope", Value::Null).await.unwrap_err();
    assert!(err.what_failed().contains("no handler"));
}

#[tokio::test]
async fn a_bounded_channel_applies_backpressure_and_never_drops_a_message() {
    let e = env("pressure");
    let seen = Arc::new(AtomicU32::new(0));
    let handle = OrchestratorBuilder::new(e.services.clone())
        .command(Arc::new(Slow(seen.clone())))
        .spawn(2);
    let mut tasks = Vec::new();
    for n in 0..40u64 {
        let h = handle.clone();
        tasks.push(tokio::spawn(async move {
            h.send("test.slow", json!(n)).await.unwrap()
        }));
    }
    let mut answers: Vec<u64> = Vec::new();
    for t in tasks {
        answers.push(t.await.unwrap().as_u64().unwrap());
    }
    answers.sort_unstable();
    assert_eq!(
        answers,
        (0..40).collect::<Vec<u64>>(),
        "every message was answered"
    );
    assert_eq!(seen.load(Ordering::SeqCst), 40);
}

#[tokio::test]
async fn a_panicking_handler_fails_that_message_only() {
    let e = env("panic");
    let handle = OrchestratorBuilder::new(e.services.clone())
        .command(Arc::new(Boom))
        .command(Arc::new(Echo))
        .spawn(4);
    let err = handle.send("test.boom", Value::Null).await.unwrap_err();
    assert!(
        err.what_failed().contains("panicked"),
        "{}",
        err.what_failed()
    );
    assert_eq!(
        handle
            .send("test.echo", json!("still alive"))
            .await
            .unwrap(),
        json!("still alive")
    );
}

#[test]
#[should_panic(expected = "two handlers claim")]
fn two_handlers_for_one_message_is_a_startup_error() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let _guard = rt.enter();
    let e = env("dup");
    let _ = OrchestratorBuilder::new(e.services.clone())
        .command(Arc::new(Echo))
        .command(Arc::new(Echo));
}

#[tokio::test]
async fn the_snapshot_reflects_committed_state_only_and_carries_its_journal_position() {
    let e = env("snapshot");
    let handle = OrchestratorBuilder::new(e.services.clone())
        .snapshot_part(Arc::new(RunsPart))
        .spawn(4);
    let empty = handle.snapshot().unwrap();
    assert_eq!(empty["last_seq"], 0);
    assert_eq!(empty["parts"]["runs"], json!([]));

    let run = e.create_run().await;
    let after = handle.snapshot().unwrap();
    assert_eq!(after["last_seq"], 1);
    assert_eq!(after["parts"]["runs"].as_array().unwrap().len(), 1);

    // A transaction that fails after writing state leaves no trace in the snapshot.
    let r = run.as_str().to_owned();
    let failed: Result<(), _> = e.services.store.write(move |tx| {
        f06_core::set_run_state(tx.conn(), &r, "\"BROKEN\"", 5)?;
        Err(VelaError::internal("rolled back"))
    });
    assert!(failed.is_err());
    assert_eq!(handle.snapshot().unwrap(), after);
}

#[tokio::test]
async fn a_run_survives_a_restart() {
    let e = env("restart");
    let run = e.create_run().await;
    let before = Runs::new(&e.services).run_state(&run).unwrap();
    let reopened = e.restart();
    assert_eq!(Runs::new(&reopened).run_state(&run).unwrap(), before);
}

#[tokio::test]
async fn a_seeded_secret_in_an_event_payload_is_absent_from_the_persisted_journal_row() {
    let secret = ["sk-seeded", "-SECRET-4f9a8b7c"].concat();
    let needle = secret.clone();
    let options = StoreOptions {
        payload_filter: Arc::new(move |payload: &str| {
            payload.replace(needle.as_str(), "[REDACTED]")
        }),
        ..StoreOptions::default()
    };
    let e = env_with("redact", options);
    e.seed_project();
    let run = RunId::new("run-1").unwrap();
    Runs::new(&e.services)
        .create_run(
            "p1",
            run,
            CommitSha::new("a".repeat(40)).unwrap(),
            BranchName::new("vela/run-1/integration").unwrap(),
            &format!("build with token {secret}"),
        )
        .await
        .unwrap();
    let rows = e.raw_journal();
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].1.contains(secret.as_str()), "{}", rows[0].1);
    assert!(rows[0].1.contains("[REDACTED]"));
    // Reading it back through the journal API also shows no secret.
    let events = e
        .services
        .store
        .read(|c| vela_persistence::journal::read_after(c, 0, 10))
        .unwrap();
    assert!(!format!("{events:?}").contains(secret.as_str()));
    let _ = (
        Event::RunCreated(RunCreatedPayload {
            run_base_sha: CommitSha::new("b".repeat(40)).unwrap(),
            integration_branch: BranchName::new("x").unwrap(),
            scope: String::new(),
        }),
        NewEvent {
            ts: 0,
            run_id: None,
            ticket_id: None,
            worker_id: None,
            event: Event::WorkerFailed(WorkerFailedPayload {
                error: VelaError::internal("x"),
            }),
        },
    );
}

#[tokio::test]
async fn a_panicking_worker_task_fails_that_worker_only() {
    let e = env("supervise");
    let run = e.create_run().await;
    let runs = Runs::new(&e.services);
    let ticket = TicketId::new("T-1").unwrap();
    for name in ["w-1", "w-2"] {
        runs.create_worker(
            &run,
            WorkerId::new(name).unwrap(),
            &ticket,
            BranchName::new(format!("vela/run-1/{name}")).unwrap(),
            WorktreePath::new(format!("C:/wt/{name}")).unwrap(),
            CommitSha::new("a".repeat(40)).unwrap(),
            "default",
            IntegrationHealth::Healthy,
        )
        .await
        .unwrap();
    }
    let bad = supervise(
        e.services.clone(),
        run.clone(),
        WorkerId::new("w-1").unwrap(),
        async { panic!("worker bug") },
    );
    let good = supervise(
        e.services.clone(),
        run.clone(),
        WorkerId::new("w-2").unwrap(),
        async { Ok(()) },
    );
    bad.await.unwrap();
    good.await.unwrap();
    assert_eq!(
        runs.worker_status(&WorkerId::new("w-1").unwrap())
            .unwrap()
            .state(),
        WorkerState::Failed
    );
    assert_eq!(
        runs.worker_status(&WorkerId::new("w-2").unwrap())
            .unwrap()
            .state(),
        WorkerState::Active(WorkerPhase::Provisioning)
    );
    assert!(
        e.raw_journal()
            .iter()
            .any(|(kind, _)| kind == "WorkerFailed")
    );
    // A returned error is recorded the same way.
    let err = supervise(
        e.services.clone(),
        run.clone(),
        WorkerId::new("w-2").unwrap(),
        async { Err(VelaError::internal("cannot go on")) },
    );
    err.await.unwrap();
    assert_eq!(
        runs.worker_status(&WorkerId::new("w-2").unwrap())
            .unwrap()
            .state(),
        WorkerState::Failed
    );
}

#[test]
fn services_are_registered_and_found_by_type() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let _guard = rt.enter();
    let e = env("ext");
    assert!(e.services.get::<String>().is_none());
    e.services.register(Arc::new("hello".to_owned()));
    assert_eq!(e.services.get::<String>().unwrap().as_str(), "hello");
}
