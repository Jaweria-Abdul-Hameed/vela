//! The real clock and identifier source (the domain never reads the system clock or makes randomness itself).
//!
//! Identifiers are `<prefix>-<time in hex><counter in hex>`: unique across restarts (time) and within a process (counter), short,
//! and made only of characters every identifier type accepts.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use vela_domain::ids::{OperationId, RunId, WorkerId};
use vela_domain::ports::{Clock, IdGenerator};

/// Wall-clock time in epoch milliseconds.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
    }
}

/// Identifiers from the system clock and a counter.
#[derive(Debug, Default)]
pub struct SystemIds {
    counter: AtomicU64,
}

impl SystemIds {
    /// A fresh source.
    pub fn new() -> Self {
        Self::default()
    }

    fn next(&self, prefix: &str) -> String {
        let n = self.counter.fetch_add(1, Ordering::SeqCst);
        let ms = SystemClock.now_ms();
        format!("{prefix}-{ms:x}{n:03x}")
    }
}

impl IdGenerator for SystemIds {
    fn new_run_id(&self) -> RunId {
        RunId::new(self.next("run")).expect("a generated run id is valid")
    }

    fn new_worker_id(&self) -> WorkerId {
        WorkerId::new(self.next("w")).expect("a generated worker id is valid")
    }

    fn new_operation_id(&self) -> OperationId {
        OperationId::new(self.next("op")).expect("a generated operation id is valid")
    }

    fn new_token(&self) -> String {
        self.next("t")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn identifiers_are_valid_and_never_repeat() {
        let ids = SystemIds::new();
        let mut seen = HashSet::new();
        for _ in 0..2000 {
            assert!(seen.insert(ids.new_run_id().as_str().to_owned()));
            assert!(seen.insert(ids.new_worker_id().as_str().to_owned()));
            assert!(seen.insert(ids.new_operation_id().as_str().to_owned()));
            assert!(seen.insert(ids.new_token()));
        }
    }

    #[test]
    fn the_clock_is_recent() {
        assert!(SystemClock.now_ms() > 1_700_000_000_000);
    }
}
