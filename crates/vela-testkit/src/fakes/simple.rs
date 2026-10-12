//! Fakes for the small ports: clock, identifiers, power, session probe, notifier, and the shared call recorder.

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::ids::{OperationId, RunId, WorkerId};
use vela_domain::ports::{
    Clock, IdGenerator, KeepAwakeToken, Notification, Notifier, PortFuture, PowerManager,
    SessionAvailability, SessionProbe, ready,
};

/// Records the calls a fake received, in order. Cloning shares the log.
#[derive(Debug, Clone, Default)]
pub struct Recorder(Arc<Mutex<Vec<String>>>);

impl Recorder {
    /// An empty log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends one entry.
    pub fn record(&self, entry: impl Into<String>) {
        self.0.lock().expect("recorder lock").push(entry.into());
    }

    /// A copy of the log.
    pub fn calls(&self) -> Vec<String> {
        self.0.lock().expect("recorder lock").clone()
    }
}

/// A clock that only moves when told to.
#[derive(Debug)]
pub struct FakeClock {
    now: AtomicI64,
}

impl FakeClock {
    /// Starts at `start_ms`.
    pub fn new(start_ms: i64) -> Self {
        Self {
            now: AtomicI64::new(start_ms),
        }
    }

    /// Moves time forward.
    pub fn advance(&self, ms: i64) {
        self.now.fetch_add(ms, Ordering::SeqCst);
    }

    /// Sets the time.
    pub fn set(&self, ms: i64) {
        self.now.store(ms, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now_ms(&self) -> i64 {
        self.now.load(Ordering::SeqCst)
    }
}

/// Sequential identifiers: `run-1`, `w-1`, `op-1`, `tok-1`, each counter independent.
#[derive(Debug, Default)]
pub struct FakeIds {
    run: AtomicU64,
    worker: AtomicU64,
    operation: AtomicU64,
    token: AtomicU64,
}

impl FakeIds {
    /// Counters start at zero.
    pub fn new() -> Self {
        Self::default()
    }
}

impl IdGenerator for FakeIds {
    fn new_run_id(&self) -> RunId {
        let n = self.run.fetch_add(1, Ordering::SeqCst) + 1;
        RunId::new(format!("run-{n}")).expect("valid run id")
    }

    fn new_worker_id(&self) -> WorkerId {
        let n = self.worker.fetch_add(1, Ordering::SeqCst) + 1;
        WorkerId::new(format!("w-{n}")).expect("valid worker id")
    }

    fn new_operation_id(&self) -> OperationId {
        let n = self.operation.fetch_add(1, Ordering::SeqCst) + 1;
        OperationId::new(format!("op-{n}")).expect("valid operation id")
    }

    fn new_token(&self) -> String {
        let n = self.token.fetch_add(1, Ordering::SeqCst) + 1;
        format!("tok-{n}")
    }
}

/// A power manager that can be told to fail the next acquire.
#[derive(Debug, Default)]
pub struct FakePower {
    next: AtomicU64,
    fail_next: Mutex<bool>,
    log: Recorder,
}

impl FakePower {
    /// Works until [`FakePower::fail_next_acquire`] is called.
    pub fn new() -> Self {
        Self::default()
    }

    /// The next `acquire_keep_awake` fails once.
    pub fn fail_next_acquire(&self) {
        *self.fail_next.lock().expect("power lock") = true;
    }

    /// The recorded calls.
    pub fn log(&self) -> &Recorder {
        &self.log
    }
}

impl PowerManager for FakePower {
    fn acquire_keep_awake(&self, reason: &str) -> Result<KeepAwakeToken, VelaError> {
        if std::mem::take(&mut *self.fail_next.lock().expect("power lock")) {
            self.log.record(format!("acquire-failed:{reason}"));
            return Err(VelaError::of_class(
                ErrorClass::ExternalTemporary,
                "keep-awake request failed",
            ));
        }
        let id = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        self.log.record(format!("acquire:{reason}"));
        Ok(KeepAwakeToken(id))
    }

    fn release(&self, token: KeepAwakeToken) {
        self.log.record(format!("release:{}", token.0));
    }
}

/// A session probe whose answer is set by the test.
#[derive(Debug)]
pub struct FakeSessionProbe {
    state: Mutex<SessionAvailability>,
}

impl FakeSessionProbe {
    /// Starts with `state`.
    pub fn new(state: SessionAvailability) -> Self {
        Self {
            state: Mutex::new(state),
        }
    }

    /// Changes the answer.
    pub fn set(&self, state: SessionAvailability) {
        *self.state.lock().expect("probe lock") = state;
    }
}

impl SessionProbe for FakeSessionProbe {
    fn interactive_session(&self) -> SessionAvailability {
        *self.state.lock().expect("probe lock")
    }
}

/// A notifier that keeps every notification.
#[derive(Debug, Default)]
pub struct FakeNotifier {
    sent: Mutex<Vec<Notification>>,
}

impl FakeNotifier {
    /// Nothing sent yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Every notification shown, in order.
    pub fn sent(&self) -> Vec<Notification> {
        self.sent.lock().expect("notifier lock").clone()
    }
}

impl Notifier for FakeNotifier {
    fn notify<'a>(&'a self, notification: &'a Notification) -> PortFuture<'a, ()> {
        self.sent
            .lock()
            .expect("notifier lock")
            .push(notification.clone());
        ready(Ok(()))
    }
}
