//! The state store: one dedicated writer thread owns the only read-write connection (ADR-002, ADR-017). Every transition is
//! one transaction that writes state and journal together; events are published to subscribers only after the commit.
//! Reads use their own short-lived read-only connections (WAL lets them run beside the writer).

use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior, params};
use vela_domain::errors::{ErrorClass, ErrorCode, VelaError};
use vela_domain::events::EventEnvelope;

use crate::integrity;
use crate::journal::{self, NewEvent, PayloadFilter, identity_filter};
use crate::migrations;
use crate::recovery;

/// Converts any displayable failure into the generic typed store error.
pub(crate) fn store_error(e: impl Display) -> VelaError {
    VelaError::of_class(ErrorClass::InternalBug, format!("state store error: {e}"))
}

/// How to open a store.
#[derive(Clone)]
pub struct StoreOptions {
    /// Redacts journal payloads (the composition root injects the central redactor).
    pub payload_filter: PayloadFilter,
    /// Clock used for backup names and migration records.
    pub now_ms: Arc<dyn Fn() -> i64 + Send + Sync>,
}

impl Default for StoreOptions {
    fn default() -> Self {
        Self {
            payload_filter: identity_filter(),
            now_ms: Arc::new(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
                    .unwrap_or(0)
            }),
        }
    }
}

/// The transaction handed to a write closure: state statements through [`WriteTx::conn`], journal through
/// [`WriteTx::append`].
pub struct WriteTx<'a> {
    tx: Transaction<'a>,
    filter: &'a PayloadFilter,
    pending: Vec<EventEnvelope>,
}

impl WriteTx<'_> {
    /// The connection to run state statements on (inside the transaction).
    pub fn conn(&self) -> &Connection {
        &self.tx
    }

    /// Journals `event`; it is published to subscribers only if the transaction commits.
    pub fn append(&mut self, event: &NewEvent) -> Result<EventEnvelope, VelaError> {
        let envelope = journal::append(&self.tx, event, self.filter)?;
        self.pending.push(envelope.clone());
        Ok(envelope)
    }
}

type Job = Box<dyn FnOnce(&mut Connection, &Publisher) + Send>;

#[derive(Clone, Default)]
struct Publisher(Arc<Mutex<Vec<Sender<EventEnvelope>>>>);

impl Publisher {
    fn publish(&self, events: &[EventEnvelope]) {
        let mut subs = self.0.lock().expect("subscribers lock");
        subs.retain(|s| events.iter().all(|e| s.send(e.clone()).is_ok()));
    }
}

/// Handle to the state store. Dropping it stops the writer thread.
pub struct StateStore {
    path: PathBuf,
    jobs: Option<Sender<Job>>,
    thread: Option<JoinHandle<()>>,
    publisher: Publisher,
    filter: PayloadFilter,
}

impl std::fmt::Debug for StateStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateStore")
            .field("path", &self.path)
            .finish()
    }
}

fn open_connection(path: &Path) -> Result<Connection, VelaError> {
    let conn = Connection::open(path).map_err(store_error)?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(store_error)?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(store_error)?;
    conn.pragma_update(None, "synchronous", "FULL")
        .map_err(store_error)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(store_error)?;
    Ok(conn)
}

impl StateStore {
    /// Opens (creating if needed) the store at `path`: integrity check, backup before any migration, migration ladder
    /// (restoring the backup if one fails), then integrity check again, then starts the writer thread.
    pub fn open(path: &Path, options: StoreOptions) -> Result<Self, VelaError> {
        Self::open_with(path, options, migrations::MIGRATIONS)
    }

    /// [`StateStore::open`] with an explicit migration ladder (tests inject failing ladders).
    pub fn open_with(
        path: &Path,
        options: StoreOptions,
        ladder: migrations::Ladder<'_>,
    ) -> Result<Self, VelaError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(store_error)?;
        }
        let existed = path.exists();
        let mut conn = open_connection(path)?;
        if existed {
            integrity::require_healthy(&conn)?;
        }
        let pending = migrations::pending_in(&conn, ladder)?;
        let fresh = migrations::applied(&conn)?.is_empty();
        let mut backup_record: Option<(PathBuf, u32)> = None;
        if !pending.is_empty() && !fresh {
            let version = migrations::applied(&conn)?.into_iter().max().unwrap_or(0);
            let made = recovery::backup(&conn, path, version, (options.now_ms)())?;
            backup_record = Some((made, version));
        }
        if let Err(e) = migrations::migrate_in(&mut conn, ladder, (options.now_ms)()) {
            drop(conn);
            if let Some((backup, _)) = &backup_record {
                recovery::restore(path, backup)?;
            }
            return Err(e);
        }
        if let Some((backup, version)) = &backup_record {
            conn.execute(
                "INSERT INTO state_store_backups (path, schema_version, created_ms) VALUES (?1, ?2, ?3)",
                params![backup.to_string_lossy(), version, (options.now_ms)()],
            )
            .map_err(store_error)?;
        }
        integrity::require_healthy(&conn)?;

        let (jobs_tx, jobs_rx) = mpsc::channel::<Job>();
        let publisher = Publisher::default();
        let thread_publisher = publisher.clone();
        let thread = thread::Builder::new()
            .name("vela-state-writer".to_owned())
            .spawn(move || {
                let mut conn = conn;
                while let Ok(job) = jobs_rx.recv() {
                    job(&mut conn, &thread_publisher);
                }
            })
            .map_err(store_error)?;
        Ok(Self {
            path: path.to_owned(),
            jobs: Some(jobs_tx),
            thread: Some(thread),
            publisher,
            filter: options.payload_filter,
        })
    }

    /// Subscribes to committed events. Receivers see events in `seq` order, only after the commit that wrote them.
    pub fn subscribe(&self) -> Receiver<EventEnvelope> {
        let (tx, rx) = mpsc::channel();
        self.publisher.0.lock().expect("subscribers lock").push(tx);
        rx
    }

    /// Runs `work` on the writer thread in one immediate transaction. State and journal rows commit together or not at
    /// all; events publish after the commit.
    pub fn write<T, F>(&self, work: F) -> Result<T, VelaError>
    where
        T: Send + 'static,
        F: FnOnce(&mut WriteTx<'_>) -> Result<T, VelaError> + Send + 'static,
    {
        let (done_tx, done_rx): (SyncSender<Result<T, VelaError>>, _) = mpsc::sync_channel(1);
        let filter = self.filter.clone();
        let job: Job = Box::new(move |conn, publisher| {
            let outcome = (|| {
                let tx = conn
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(store_error)?;
                let mut w = WriteTx {
                    tx,
                    filter: &filter,
                    pending: Vec::new(),
                };
                let value = work(&mut w)?;
                let WriteTx { tx, pending, .. } = w;
                vela_domain::fault_point!(vela_domain::fault::FaultPoint::PersistenceBeforeCommit);
                tx.commit().map_err(store_error)?;
                vela_domain::fault_point!(vela_domain::fault::FaultPoint::PersistenceAfterCommit);
                Ok((value, pending))
            })();
            let result = match outcome {
                Ok((value, pending)) => {
                    publisher.publish(&pending);
                    Ok(value)
                }
                Err(e) => Err(e),
            };
            let _ = done_tx.send(result);
        });
        self.jobs
            .as_ref()
            .ok_or_else(|| store_error("the writer is stopped"))?
            .send(job)
            .map_err(|_| store_error("the writer thread is gone"))?;
        done_rx
            .recv()
            .map_err(|_| store_error("the writer thread dropped the request"))?
    }

    /// Runs `read` on a fresh read-only connection.
    pub fn read<T>(
        &self,
        read: impl FnOnce(&Connection) -> Result<T, VelaError>,
    ) -> Result<T, VelaError> {
        let conn = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(store_error)?;
        conn.pragma_update(None, "query_only", "ON")
            .map_err(store_error)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(store_error)?;
        read(&conn)
    }

    /// The store's file path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for StateStore {
    fn drop(&mut self) {
        self.jobs.take();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Maps a corrupt-store failure for callers that only want the typed code.
pub fn is_corrupt(e: &VelaError) -> bool {
    e.code() == Some(ErrorCode::StateStoreCorrupt)
}

#[cfg(test)]
mod tests;
