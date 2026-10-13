//! The Orchestrator (O01, ADR-002, ADR-017): the single owner of run state.
//!
//! It is an actor: callers send typed messages through a **bounded** channel (a full channel makes the sender wait, nothing is
//! ever dropped) and the actor handles them one at a time, in order. Every state change is one transaction that writes state
//! and journal together; events reach subscribers only after the commit. A handler that panics fails that message only.
//!
//! Message routing and snapshot assembly are open-ended: each module registers a [`CommandHandler`] and a
//! [`SnapshotContributor`] with the builder, so adding a message or a snapshot part edits no shared enum or function
//! (`SHARED_SURFACE_PROTOCOL.md` section 10). Services other modules need are registered by type in [`Services`].

pub mod driver;
pub mod runs;
pub mod supervisor;
#[cfg(test)]
pub(crate) mod test_support;

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};
use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::ports::{Clock, IdGenerator, PortFuture};
use vela_persistence::journal;
use vela_persistence::writer::{Connection, StateStore};

/// What every handler can reach.
pub struct Services {
    /// The state store (the only writer of orchestration state).
    pub store: Arc<StateStore>,
    /// Time.
    pub clock: Arc<dyn Clock>,
    /// Identifiers.
    pub ids: Arc<dyn IdGenerator>,
    extensions: Mutex<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl Services {
    /// Services over a store, a clock and an identifier source.
    pub fn new(
        store: Arc<StateStore>,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn IdGenerator>,
    ) -> Arc<Self> {
        Arc::new(Self {
            store,
            clock,
            ids,
            extensions: Mutex::new(HashMap::new()),
        })
    }

    /// Registers a service other modules can look up by its type (a later registration replaces an earlier one).
    pub fn register<T: Any + Send + Sync>(&self, service: Arc<T>) {
        self.extensions
            .lock()
            .expect("extensions lock")
            .insert(TypeId::of::<T>(), service);
    }

    /// Looks a registered service up by type.
    pub fn get<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        let found = self
            .extensions
            .lock()
            .expect("extensions lock")
            .get(&TypeId::of::<T>())
            .cloned()?;
        found.downcast::<T>().ok()
    }

    /// Epoch milliseconds now.
    pub fn now_ms(&self) -> i64 {
        self.clock.now_ms()
    }
}

/// A named message handler. The name is the message's identity (`project.import`).
pub trait CommandHandler: Send + Sync {
    /// The message name this handler owns.
    fn name(&self) -> &'static str;
    /// Handles one message.
    fn handle<'a>(&'a self, services: &'a Services, payload: Value) -> PortFuture<'a, Value>;
}

/// A part of the snapshot. It reads committed state from the connection it is given (one consistent read for all parts).
pub trait SnapshotContributor: Send + Sync {
    /// The key the part appears under.
    fn name(&self) -> &'static str;
    /// Reads the part.
    fn contribute(&self, conn: &Connection) -> Result<Value, VelaError>;
}

struct Envelope {
    name: String,
    payload: Value,
    reply: oneshot::Sender<Result<Value, VelaError>>,
}

/// Collects handlers and snapshot parts, then starts the actor.
pub struct OrchestratorBuilder {
    services: Arc<Services>,
    handlers: HashMap<&'static str, Arc<dyn CommandHandler>>,
    parts: Vec<Arc<dyn SnapshotContributor>>,
}

impl OrchestratorBuilder {
    /// A builder over the shared services.
    pub fn new(services: Arc<Services>) -> Self {
        Self {
            services,
            handlers: HashMap::new(),
            parts: Vec::new(),
        }
    }

    /// Registers a handler. Two handlers for one name is a programming error and panics at startup.
    pub fn command(mut self, handler: Arc<dyn CommandHandler>) -> Self {
        let name = handler.name();
        assert!(
            self.handlers.insert(name, handler).is_none(),
            "two handlers claim the message {name}"
        );
        self
    }

    /// Registers a snapshot part. Parts appear under their names in registration-independent (sorted) order.
    pub fn snapshot_part(mut self, part: Arc<dyn SnapshotContributor>) -> Self {
        self.parts.push(part);
        self
    }

    /// Starts the actor with a channel of `capacity` messages.
    pub fn spawn(mut self, capacity: usize) -> OrchestratorHandle {
        self.parts.sort_by_key(|p| p.name());
        let (tx, mut rx) = mpsc::channel::<Envelope>(capacity.max(1));
        let handlers = Arc::new(self.handlers);
        let services = self.services.clone();
        let actor_handlers = handlers.clone();
        tokio::spawn(async move {
            while let Some(envelope) = rx.recv().await {
                let result = match actor_handlers.get(envelope.name.as_str()).cloned() {
                    None => Err(VelaError::of_class(
                        ErrorClass::ValidationFailure,
                        format!(
                            "no handler is registered for the message `{}`",
                            envelope.name
                        ),
                    )),
                    Some(handler) => {
                        let services = services.clone();
                        let payload = envelope.payload;
                        // Each message runs in its own task: a panic fails this message only, never the actor.
                        match tokio::spawn(async move { handler.handle(&services, payload).await })
                            .await
                        {
                            Ok(result) => result,
                            Err(join) if join.is_panic() => Err(VelaError::internal(format!(
                                "the handler for `{}` panicked; the message failed and the orchestrator continues",
                                envelope.name
                            ))),
                            Err(_) => Err(VelaError::internal("the handler task was cancelled")),
                        }
                    }
                };
                let _ = envelope.reply.send(result);
            }
        });
        OrchestratorHandle {
            tx,
            services: self.services,
            parts: Arc::new(self.parts),
        }
    }
}

/// How callers reach the actor.
#[derive(Clone)]
pub struct OrchestratorHandle {
    tx: mpsc::Sender<Envelope>,
    services: Arc<Services>,
    parts: Arc<Vec<Arc<dyn SnapshotContributor>>>,
}

impl OrchestratorHandle {
    /// Sends a message and waits for its result. When the channel is full this waits for room (backpressure); the message is
    /// never dropped.
    pub async fn send(&self, name: &str, payload: Value) -> Result<Value, VelaError> {
        let (reply, answer) = oneshot::channel();
        self.tx
            .send(Envelope {
                name: name.to_owned(),
                payload,
                reply,
            })
            .await
            .map_err(|_| VelaError::internal("the orchestrator is not running"))?;
        answer.await.map_err(|_| {
            VelaError::internal("the orchestrator dropped the message without answering")
        })?
    }

    /// The services (for modules that need the store or the clock).
    pub fn services(&self) -> &Arc<Services> {
        &self.services
    }

    /// The snapshot: the journal position and every part, read in one transaction so they describe the same committed state.
    pub fn snapshot(&self) -> Result<Value, VelaError> {
        let parts = self.parts.clone();
        self.services.store.read(move |conn| {
            conn.execute_batch("BEGIN DEFERRED")
                .map_err(|e| VelaError::internal(format!("snapshot: {e}")))?;
            let outcome = (|| {
                let last_seq = journal::tip(conn)?;
                let mut map = serde_json::Map::new();
                for part in parts.iter() {
                    map.insert(part.name().to_owned(), part.contribute(conn)?);
                }
                Ok::<_, VelaError>(json!({ "last_seq": last_seq, "parts": Value::Object(map) }))
            })();
            let _ = conn.execute_batch("COMMIT");
            outcome
        })
    }
}

#[cfg(test)]
mod tests;
