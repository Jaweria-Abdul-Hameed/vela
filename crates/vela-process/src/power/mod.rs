//! Keep-awake (P03, FR-041): `SetThreadExecutionState` is per thread, so each acquisition owns a dedicated thread that sets
//! `ES_CONTINUOUS | ES_SYSTEM_REQUIRED`, parks until released, then clears the state and exits. Where the system call does
//! not exist the manager is a harmless no-op.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};

use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::ports::{KeepAwakeToken, PowerManager};

struct Holder {
    release: Sender<()>,
    thread: JoinHandle<()>,
}

/// Holds the system awake through dedicated threads.
#[derive(Default)]
pub struct ExecutionStatePower {
    next: AtomicU64,
    holders: Mutex<HashMap<u64, Holder>>,
}

impl std::fmt::Debug for ExecutionStatePower {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionStatePower")
            .field("held", &self.held())
            .finish()
    }
}

impl ExecutionStatePower {
    /// Nothing held yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of active acquisitions.
    pub fn held(&self) -> usize {
        self.holders.lock().expect("holders lock").len()
    }
}

#[cfg(windows)]
fn set_awake(on: bool) -> bool {
    use windows::Win32::System::Power::{
        ES_CONTINUOUS, ES_SYSTEM_REQUIRED, SetThreadExecutionState,
    };
    let flags = if on {
        ES_CONTINUOUS | ES_SYSTEM_REQUIRED
    } else {
        ES_CONTINUOUS
    };
    // SAFETY: a plain Win32 call with flag values; it affects only the calling thread.
    unsafe { SetThreadExecutionState(flags).0 != 0 }
}

#[cfg(not(windows))]
fn set_awake(_on: bool) -> bool {
    true
}

impl PowerManager for ExecutionStatePower {
    fn acquire_keep_awake(&self, reason: &str) -> Result<KeepAwakeToken, VelaError> {
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<bool>();
        let name = format!("vela-keep-awake-{reason}");
        let thread = thread::Builder::new()
            .name(name)
            .spawn(move || {
                let ok = set_awake(true);
                let _ = ready_tx.send(ok);
                if ok {
                    let _ = release_rx.recv();
                    set_awake(false);
                }
            })
            .map_err(|e| {
                VelaError::of_class(
                    ErrorClass::ExternalTemporary,
                    format!("could not start the keep-awake thread: {e}"),
                )
            })?;
        match ready_rx.recv() {
            Ok(true) => {}
            _ => {
                let _ = thread.join();
                return Err(VelaError::of_class(
                    ErrorClass::ExternalTemporary,
                    "the system refused the keep-awake request",
                ));
            }
        }
        let id = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        self.holders.lock().expect("holders lock").insert(
            id,
            Holder {
                release: release_tx,
                thread,
            },
        );
        Ok(KeepAwakeToken(id))
    }

    fn release(&self, token: KeepAwakeToken) {
        let holder = self.holders.lock().expect("holders lock").remove(&token.0);
        if let Some(h) = holder {
            let _ = h.release.send(());
            let _ = h.thread.join();
        }
    }
}

impl Drop for ExecutionStatePower {
    fn drop(&mut self) {
        let tokens: Vec<u64> = self
            .holders
            .lock()
            .expect("holders lock")
            .keys()
            .copied()
            .collect();
        for t in tokens {
            self.release(KeepAwakeToken(t));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquire_and_release_round_trip_and_tokens_are_independent() {
        let p = ExecutionStatePower::new();
        let a = p.acquire_keep_awake("run-1").unwrap();
        let b = p.acquire_keep_awake("run-2").unwrap();
        assert_ne!(a, b);
        assert_eq!(p.held(), 2);
        p.release(a.clone());
        assert_eq!(p.held(), 1);
        p.release(a); // releasing twice is harmless
        p.release(b);
        assert_eq!(p.held(), 0);
    }

    #[test]
    fn dropping_the_manager_releases_everything() {
        let p = ExecutionStatePower::new();
        p.acquire_keep_awake("x").unwrap();
        drop(p);
    }
}
