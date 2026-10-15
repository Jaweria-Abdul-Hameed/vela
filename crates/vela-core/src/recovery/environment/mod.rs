//! Sleep, wake, network loss and remote outages (R06). Sleep is a recoverable pause, not a failure: on wake Vela detects the gap,
//! assumes nothing survived (no command is assumed to still be running), and re-checks processes, locks, network and
//! authentication **in that order** before anything continues. A network failure never corrupts local state: a push that cannot
//! reach the remote leaves every local ref as it was and is retried later, which is safe because a push without force is
//! idempotent.

use std::time::Duration;

use vela_domain::errors::VelaError;
use vela_domain::ids::{BranchName, WorktreePath};
use vela_domain::ports::{GitContext, GitRemote, PortFuture, PushOutcome, RemoteName};

/// Detects that the machine slept: the time since the last heartbeat is far longer than the heartbeat interval.
#[derive(Debug, Clone, Copy)]
pub struct GapDetector {
    /// How often the heartbeat is meant to tick.
    pub interval_ms: i64,
    /// A gap beyond this many intervals is a sleep.
    pub tolerance: i64,
}

impl Default for GapDetector {
    fn default() -> Self {
        Self {
            interval_ms: 5_000,
            tolerance: 4,
        }
    }
}

impl GapDetector {
    /// The length of the gap in milliseconds when `now` is far past `last`, otherwise `None`. A clock that went backwards is
    /// not a gap.
    pub fn gap(&self, last_tick_ms: i64, now_ms: i64) -> Option<i64> {
        let elapsed = now_ms.checked_sub(last_tick_ms)?;
        (elapsed > self.interval_ms.saturating_mul(self.tolerance)).then_some(elapsed)
    }
}

/// One post-wake check. The declaration order is the order they run in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Check {
    /// Child processes: nothing is assumed to have stayed alive.
    Processes,
    /// Lane, worktree and operation locks: any held across the sleep are re-validated.
    Locks,
    /// Whether the network (and the remote) is reachable.
    Network,
    /// Whether the provider and the remote still accept the sign-in.
    Authentication,
}

/// Every check, in the order they run.
pub const ORDER: [Check; 4] = [
    Check::Processes,
    Check::Locks,
    Check::Network,
    Check::Authentication,
];

/// What the environment can be asked. The implementation talks to the process table, the lock files, the network and the
/// provider; a failing check returns the error that says what was wrong. This module only fixes the order and the rule that a failure stops everything after it.
pub trait WakeChecks: Send + Sync {
    /// Runs one check; `Err` carries what was wrong, in words.
    fn run<'a>(&'a self, check: Check) -> PortFuture<'a, ()>;
}

/// The result of the post-wake re-checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WakeOutcome {
    /// The checks that passed, in the order they ran.
    pub passed: Vec<Check>,
    /// The first check that failed and why; later checks were not run.
    pub failed: Option<(Check, String)>,
}

impl WakeOutcome {
    /// Work may continue only when every check passed.
    pub fn may_resume(&self) -> bool {
        self.failed.is_none() && self.passed.len() == ORDER.len()
    }
}

/// Runs the checks in order, stopping at the first failure.
pub async fn post_wake(checks: &dyn WakeChecks) -> WakeOutcome {
    let mut passed = Vec::new();
    for check in ORDER {
        match checks.run(check).await {
            Ok(()) => passed.push(check),
            Err(e) => {
                return WakeOutcome {
                    passed,
                    failed: Some((check, e.what_failed().to_owned())),
                };
            }
        }
    }
    WakeOutcome {
        passed,
        failed: None,
    }
}

/// Whether a failure is the kind that waiting can fix (network down, remote unavailable).
pub fn is_transient(error: &VelaError) -> bool {
    error.class().is_retryable()
}

/// Pushes with bounded retries over transient failures, doubling `base` between attempts. A non-transient failure (a rejected
/// non-fast-forward, an authentication failure) is returned at once and never retried. Because the push has no force option and
/// an already-pushed branch is accepted as is, calling this again after any outcome is safe.
pub async fn push_retrying(
    git: &dyn GitRemote,
    ctx: &GitContext,
    repo: &WorktreePath,
    remote: &RemoteName,
    branch: &BranchName,
    attempts: u32,
    base: Duration,
) -> Result<PushOutcome, VelaError> {
    let mut wait = base;
    let mut last = None;
    for attempt in 0..attempts.max(1) {
        match git.push(ctx, repo, remote, branch).await {
            Ok(done) => return Ok(done),
            Err(e) if is_transient(&e) => {
                last = Some(e);
                if attempt + 1 < attempts {
                    tokio::time::sleep(wait).await;
                    wait = wait.saturating_mul(2);
                }
            }
            Err(e) => return Err(e),
        }
    }
    Err(last.unwrap_or_else(|| VelaError::internal("no push attempt was made")))
}

#[cfg(test)]
mod tests;
