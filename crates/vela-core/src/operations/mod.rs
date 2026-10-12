//! Operation records and reconciliation (R01): every external, non-idempotent action has an id, an **intent persisted before**
//! the action, an **observed result persisted after**, and a recipe that reconciles it when the process died in between.
//!
//! The operation-owning tickets register their recipes: checkpoint (here), merge and push (S12), worker state (S10), approval
//! (K01), Antigravity recovery (A14). An operation kind with no recipe is never retried: it escalates to a human.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::ids::{CommitSha, OperationId, RunId, WorktreePath};
use vela_domain::ports::PortFuture;
use vela_persistence::repositories::r01_operations::{self, Operation};

use crate::orchestrator::Services;

/// What reconciling one operation concluded.
#[derive(Debug, Clone, PartialEq)]
pub enum Reconciled {
    /// The action had completed; this is what was observed. The record is closed.
    Completed(Value),
    /// The action never started (or left no effect): it is safe to do it again.
    NotStarted,
    /// Recovery cannot tell; a human must look.
    NeedsHuman(String),
}

/// How one kind of operation is reconciled after a crash.
pub trait Recipe: Send + Sync {
    /// The operation kind this recipe handles.
    fn kind(&self) -> &'static str;
    /// Observes the world and says what happened to `operation`.
    fn reconcile<'a>(&'a self, operation: &'a Operation) -> PortFuture<'a, Reconciled>;
}

/// Recipes by operation kind.
#[derive(Default)]
pub struct RecipeRegistry {
    recipes: Mutex<HashMap<&'static str, Arc<dyn Recipe>>>,
}

impl RecipeRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a recipe (a later one for the same kind replaces the earlier).
    pub fn register(&self, recipe: Arc<dyn Recipe>) {
        self.recipes
            .lock()
            .expect("recipes lock")
            .insert(recipe.kind(), recipe);
    }

    fn get(&self, kind: &str) -> Option<Arc<dyn Recipe>> {
        self.recipes
            .lock()
            .expect("recipes lock")
            .get(kind)
            .cloned()
    }
}

/// The result of reconciling one record.
#[derive(Debug, Clone, PartialEq)]
pub struct ReconcileReport {
    /// The operation id.
    pub id: String,
    /// What was concluded.
    pub outcome: Reconciled,
}

/// Finds a commit carrying a trailer: the checkpoint recipe's only question to Git.
pub trait CommitLookup: Send + Sync {
    /// The commit in `worktree` whose message has the trailer `key: value`, if any.
    fn find_by_trailer<'a>(
        &'a self,
        worktree: &'a WorktreePath,
        key: &'a str,
        value: &'a str,
    ) -> PortFuture<'a, Option<CommitSha>>;
}

/// Reconciles a `checkpoint` operation by looking for its trailer (the commit conventions of G03): a checkpoint commit carries
/// the operation's own id in a trailer, so it is either there or it is not.
pub struct CheckpointRecipe {
    lookup: Arc<dyn CommitLookup>,
}

/// The trailer key that carries an operation id on a checkpoint commit.
pub const OPERATION_TRAILER: &str = "Vela-Operation";

impl CheckpointRecipe {
    /// A recipe that asks `lookup`.
    pub fn new(lookup: Arc<dyn CommitLookup>) -> Self {
        Self { lookup }
    }
}

impl Recipe for CheckpointRecipe {
    fn kind(&self) -> &'static str {
        "checkpoint"
    }

    fn reconcile<'a>(&'a self, operation: &'a Operation) -> PortFuture<'a, Reconciled> {
        Box::pin(async move {
            let intent: Value = serde_json::from_str(&operation.intent).map_err(|e| {
                VelaError::internal(format!("the recorded intent is unreadable: {e}"))
            })?;
            let Some(worktree) = intent
                .get("worktree")
                .and_then(Value::as_str)
                .and_then(|w| WorktreePath::new(w).ok())
            else {
                return Ok(Reconciled::NeedsHuman(
                    "the checkpoint intent names no usable worktree".to_owned(),
                ));
            };
            Ok(
                match self
                    .lookup
                    .find_by_trailer(&worktree, OPERATION_TRAILER, &operation.id)
                    .await?
                {
                    Some(commit) => Reconciled::Completed(json!({ "commit": commit.as_str() })),
                    None => Reconciled::NotStarted,
                },
            )
        })
    }
}

/// Operation records over the shared services.
pub struct OperationLog<'a> {
    services: &'a Services,
    recipes: Arc<RecipeRegistry>,
}

impl<'a> OperationLog<'a> {
    /// Operations over `services`; the recipe registry is the one registered there (created on first use).
    pub fn new(services: &'a Services) -> Self {
        let recipes = services.get::<RecipeRegistry>().unwrap_or_else(|| {
            let r = Arc::new(RecipeRegistry::new());
            services.register(r.clone());
            r
        });
        Self { services, recipes }
    }

    /// Persists the intent. Call this, and wait for it, **before** taking the action.
    pub fn begin(
        &self,
        id: &OperationId,
        run: Option<&RunId>,
        kind: &str,
        intent: &Value,
    ) -> Result<(), VelaError> {
        let (id, run, kind, intent) = (
            id.as_str().to_owned(),
            run.map(|r| r.as_str().to_owned()),
            kind.to_owned(),
            intent.to_string(),
        );
        let now = self.services.now_ms();
        self.services.store.write(move |tx| {
            r01_operations::record_intent(
                tx.conn(),
                &id,
                run.as_deref(),
                &kind,
                &intent,
                Some(&kind),
                now,
            )
        })
    }

    /// Persists what was observed after the action. Returns false when the record was not waiting for a result.
    pub fn finish(&self, id: &OperationId, ok: bool, result: &Value) -> Result<bool, VelaError> {
        let (id, result) = (id.as_str().to_owned(), result.to_string());
        let now = self.services.now_ms();
        self.services
            .store
            .write(move |tx| r01_operations::record_result(tx.conn(), &id, ok, &result, now))
    }

    /// The record.
    pub fn get(&self, id: &OperationId) -> Result<Option<Operation>, VelaError> {
        self.services
            .store
            .read(|c| r01_operations::get(c, id.as_str()))
    }

    /// Reconciles every operation whose result was never recorded. A completed one is closed with what was observed; one that
    /// never started is left `INTENDED` for its owner to retry; one with no recipe, or whose recipe cannot tell, is reported
    /// as needing a human and is **never retried**.
    pub async fn reconcile_all(&self) -> Result<Vec<ReconcileReport>, VelaError> {
        let pending = self.services.store.read(r01_operations::list_intended)?;
        let mut reports = Vec::with_capacity(pending.len());
        for op in pending {
            let outcome = match self.recipes.get(op.recipe.as_deref().unwrap_or(&op.kind)) {
                None => Reconciled::NeedsHuman(format!(
                    "no reconciliation recipe is registered for `{}`",
                    op.kind
                )),
                Some(recipe) => recipe.reconcile(&op).await.unwrap_or_else(|e| {
                    Reconciled::NeedsHuman(format!(
                        "the recipe for `{}` failed: {}",
                        op.kind,
                        e.what_failed()
                    ))
                }),
            };
            if let Reconciled::Completed(result) = &outcome {
                let (id, text, now) = (op.id.clone(), result.to_string(), self.services.now_ms());
                self.services.store.write(move |tx| {
                    r01_operations::mark_reconciled(tx.conn(), &id, &text, now).map(|_| ())
                })?;
            }
            reports.push(ReconcileReport { id: op.id, outcome });
        }
        Ok(reports)
    }
}

/// Convenience: fail an operation id that is not a valid [`OperationId`].
pub fn operation_id(text: &str) -> Result<OperationId, VelaError> {
    OperationId::new(text)
        .map_err(|e| VelaError::of_class(ErrorClass::ValidationFailure, e.to_string()))
}

#[cfg(test)]
mod tests;
