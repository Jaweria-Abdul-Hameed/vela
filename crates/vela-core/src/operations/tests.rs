use std::sync::{Arc, Mutex};

use serde_json::json;
use vela_domain::ids::{CommitSha, OperationId, WorktreePath};
use vela_domain::ports::{PortFuture, ready};

use super::*;
use crate::orchestrator::test_support::env;

struct FakeLookup(Mutex<Option<CommitSha>>);

impl CommitLookup for FakeLookup {
    fn find_by_trailer<'a>(
        &'a self,
        _wt: &'a WorktreePath,
        key: &'a str,
        _value: &'a str,
    ) -> PortFuture<'a, Option<CommitSha>> {
        assert_eq!(key, OPERATION_TRAILER);
        ready(Ok(self.0.lock().unwrap().clone()))
    }
}

fn op(id: &str) -> OperationId {
    OperationId::new(id).unwrap()
}

#[tokio::test]
async fn intent_is_persisted_before_and_the_result_after() {
    let e = env("intent");
    e.seed_project();
    let log = OperationLog::new(&e.services);
    let id = op("op-1");
    log.begin(&id, None, "checkpoint", &json!({"worktree": "C:/wt/a"}))
        .unwrap();
    // Between the two, the record says only "intended": exactly what recovery would see after a crash.
    let mid = log.get(&id).unwrap().unwrap();
    assert_eq!(mid.state, "INTENDED");
    assert!(mid.result.is_none());
    assert_eq!(mid.intent, r#"{"worktree":"C:/wt/a"}"#);
    assert!(log.finish(&id, true, &json!({"commit": "abc"})).unwrap());
    let done = log.get(&id).unwrap().unwrap();
    assert_eq!(
        (done.state.as_str(), done.result.as_deref()),
        ("DONE", Some(r#"{"commit":"abc"}"#))
    );
    // A result can be recorded once.
    assert!(!log.finish(&id, false, &json!({})).unwrap());
    assert!(
        log.begin(&id, None, "checkpoint", &json!({})).is_err(),
        "an id is used once"
    );
}

#[tokio::test]
async fn the_checkpoint_recipe_finds_the_commit_by_its_operation_trailer() {
    let e = env("checkpoint-recipe");
    e.seed_project();
    let sha = CommitSha::new("c".repeat(40)).unwrap();
    let lookup = Arc::new(FakeLookup(Mutex::new(Some(sha.clone()))));
    OperationLog::new(&e.services);
    e.services
        .get::<RecipeRegistry>()
        .unwrap()
        .register(Arc::new(CheckpointRecipe::new(lookup.clone())));
    let log = OperationLog::new(&e.services);
    log.begin(
        &op("op-1"),
        None,
        "checkpoint",
        &json!({"worktree": "C:/wt/a"}),
    )
    .unwrap();
    log.begin(
        &op("op-2"),
        None,
        "checkpoint",
        &json!({"worktree": "C:/wt/b"}),
    )
    .unwrap();

    let reports = log.reconcile_all().await.unwrap();
    assert_eq!(reports.len(), 2);
    assert!(
        reports
            .iter()
            .all(|r| matches!(&r.outcome, Reconciled::Completed(v) if v["commit"] == sha.as_str()))
    );
    let closed = log.get(&op("op-1")).unwrap().unwrap();
    assert_eq!(closed.state, "RECONCILED");

    // The commit is not there: the action never happened, the record stays intended for its owner to retry.
    *lookup.0.lock().unwrap() = None;
    log.begin(
        &op("op-3"),
        None,
        "checkpoint",
        &json!({"worktree": "C:/wt/c"}),
    )
    .unwrap();
    let reports = log.reconcile_all().await.unwrap();
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].outcome, Reconciled::NotStarted);
    assert_eq!(log.get(&op("op-3")).unwrap().unwrap().state, "INTENDED");
}

#[tokio::test]
async fn an_operation_kind_with_no_recipe_escalates_and_is_never_retried() {
    let e = env("no-recipe");
    e.seed_project();
    let log = OperationLog::new(&e.services);
    log.begin(&op("op-9"), None, "push", &json!({"branch": "x"}))
        .unwrap();
    let reports = log.reconcile_all().await.unwrap();
    assert!(
        matches!(&reports[0].outcome, Reconciled::NeedsHuman(why) if why.contains("no reconciliation recipe"))
    );
    // Still intended: nothing re-ran it, nothing closed it.
    assert_eq!(log.get(&op("op-9")).unwrap().unwrap().state, "INTENDED");
}

#[tokio::test]
async fn a_checkpoint_intent_without_a_usable_worktree_needs_a_human() {
    let e = env("bad-intent");
    e.seed_project();
    OperationLog::new(&e.services);
    e.services
        .get::<RecipeRegistry>()
        .unwrap()
        .register(Arc::new(CheckpointRecipe::new(Arc::new(FakeLookup(
            Mutex::new(None),
        )))));
    let log = OperationLog::new(&e.services);
    log.begin(&op("op-5"), None, "checkpoint", &json!({"worktree": 7}))
        .unwrap();
    let reports = log.reconcile_all().await.unwrap();
    assert!(matches!(reports[0].outcome, Reconciled::NeedsHuman(_)));
}
