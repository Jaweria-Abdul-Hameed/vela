//! The remote update queue (T03, FR-048): updates to the issue tracker are persisted first, then applied with bounded
//! exponential backoff and jitter. An outage leaves local state untouched and every update queued. Before an update is
//! retried, the remote is asked whether its effect already exists, so a retry never duplicates a comment.
//!
//! Crash safety: the attempt counter is persisted *before* the effect is tried, so an update with `attempts > 0` may already
//! have happened and is looked up before it is re-posted.

use std::sync::Arc;

use vela_domain::errors::{ErrorCode, VelaError};
use vela_domain::ids::TicketId;
use vela_domain::ports::{Clock, IssueTrackerAdapter};
use vela_persistence::repositories::t03_remote_queue as queue;
use vela_persistence::repositories::t03_remote_queue::RemoteUpdate;
use vela_persistence::writer::StateStore;

/// First retry delay.
pub const BASE_DELAY_MS: u64 = 2_000;
/// No delay is ever longer than this (before jitter).
pub const MAX_DELAY_MS: u64 = 300_000;
/// Jitter spread: the delay is scaled by 0.8 to 1.2.
pub const JITTER_PERMILLE: u64 = 200;
/// After this many attempts a ticket-level lookup that is still impossible escalates instead of retrying forever.
pub const MAX_UNVERIFIABLE_ATTEMPTS: u32 = 10;

/// The delay before attempt number `attempt` (1-based: the delay after the first failure is `attempt = 1`).
/// `jitter` is a value in `0..=1000` chosen by the caller (random in production, fixed in tests): 500 means no jitter.
pub fn backoff_ms(attempt: u32, jitter: u32) -> u64 {
    let shift = attempt.saturating_sub(1).min(20);
    let base = BASE_DELAY_MS
        .saturating_mul(1u64 << shift)
        .min(MAX_DELAY_MS);
    let jitter = u64::from(jitter.min(1000));
    // factor = 1 - spread + 2 * spread * jitter / 1000, in permille.
    let factor = 1000 - JITTER_PERMILLE + 2 * JITTER_PERMILLE * jitter / 1000;
    base * factor / 1000
}

/// The test-visible schedule: the un-jittered delay after each of the first `n` failures.
pub fn schedule(n: u32) -> Vec<u64> {
    (1..=n).map(|a| backoff_ms(a, 500)).collect()
}

/// What to do with an update that is about to be attempted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Apply it.
    Post,
    /// Its effect is already on the remote: mark it done without posting.
    AlreadyDone,
    /// The remote cannot say; try again later (never guess and duplicate).
    RetryLater,
}

/// The idempotency decision table. `attempts` counts attempts started before this one; `lookup` is the remote's answer
/// to "does the effect exist?" (`None` when the lookup failed or is unsupported).
pub fn decide(kind: &str, attempts: u32, lookup: Option<bool>) -> Decision {
    match (kind, attempts, lookup) {
        // Status and close are idempotent on the remote: applying them again is harmless.
        ("status" | "close", _, _) => Decision::Post,
        // First try of a comment: nothing can have been posted yet.
        ("comment", 0, _) => Decision::Post,
        // A retry: only post when the lookup proves it is missing.
        ("comment", _, Some(true)) => Decision::AlreadyDone,
        ("comment", _, Some(false)) => Decision::Post,
        ("comment", _, None) => Decision::RetryLater,
        _ => Decision::RetryLater,
    }
}

/// The marker embedded in a posted comment so a later lookup can find it.
pub fn marker_for(key: &str) -> String {
    format!("<!-- vela:{key} -->")
}

/// What one pass over the queue did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DrainReport {
    /// Updates applied (or found already applied).
    pub done: u32,
    /// Updates rescheduled.
    pub retried: u32,
    /// Updates moved to `FAILED_PERMANENT`.
    pub failed: u32,
}

/// The queue.
pub struct RemoteQueue {
    store: Arc<StateStore>,
    tracker: Arc<dyn IssueTrackerAdapter>,
    clock: Arc<dyn Clock>,
    jitter: Arc<dyn Fn() -> u32 + Send + Sync>,
}

impl RemoteQueue {
    /// A queue over `store`, applying updates through `tracker`. `jitter` returns `0..=1000`.
    pub fn new(
        store: Arc<StateStore>,
        tracker: Arc<dyn IssueTrackerAdapter>,
        clock: Arc<dyn Clock>,
        jitter: Arc<dyn Fn() -> u32 + Send + Sync>,
    ) -> Self {
        Self {
            store,
            tracker,
            clock,
            jitter,
        }
    }

    /// Persists an update; a repeated `key` is ignored. Returns whether it was newly queued.
    pub fn enqueue(
        &self,
        run_id: Option<&str>,
        ticket: &TicketId,
        kind: &str,
        payload: &str,
        key: &str,
    ) -> Result<bool, VelaError> {
        let (run_id, ticket, kind, payload, key) = (
            run_id.map(str::to_owned),
            ticket.as_str().to_owned(),
            kind.to_owned(),
            payload.to_owned(),
            key.to_owned(),
        );
        let now = self.clock.now_ms();
        self.store.write(move |tx| {
            queue::enqueue(
                tx.conn(),
                run_id.as_deref(),
                &ticket,
                &kind,
                &payload,
                &key,
                now,
            )
        })
    }

    /// Number of updates still waiting.
    pub fn pending(&self) -> Result<u32, VelaError> {
        self.store.read(|c| queue::count(c, "PENDING"))
    }

    /// Applies every due update once.
    pub async fn drain(&self) -> Result<DrainReport, VelaError> {
        let now = self.clock.now_ms();
        let due = self.store.read(|c| queue::due(c, now, 50))?;
        let mut report = DrainReport::default();
        for update in due {
            self.process(update, &mut report).await?;
        }
        Ok(report)
    }

    async fn process(&self, u: RemoteUpdate, report: &mut DrainReport) -> Result<(), VelaError> {
        let Ok(ticket) = TicketId::new(u.ticket_id.clone()) else {
            self.fail(u.id, "the queued ticket id is not valid")?;
            report.failed += 1;
            return Ok(());
        };
        let lookup = if u.kind == "comment" && u.attempts > 0 {
            self.tracker
                .has_evidence_marker(&ticket, &marker_for(&u.idempotency_key))
                .await
                .ok()
        } else {
            None
        };
        match decide(&u.kind, u.attempts, lookup) {
            Decision::AlreadyDone => {
                self.done(u.id)?;
                report.done += 1;
            }
            Decision::RetryLater => {
                if u.attempts >= MAX_UNVERIFIABLE_ATTEMPTS {
                    self.fail(
                        u.id,
                        "the remote could not confirm whether the update already exists",
                    )?;
                    report.failed += 1;
                } else {
                    self.retry(
                        &u,
                        "the remote could not be asked whether the update exists",
                    )?;
                    report.retried += 1;
                }
            }
            Decision::Post => {
                let id = u.id;
                let now = self.clock.now_ms();
                self.store
                    .write(move |tx| queue::begin_attempt(tx.conn(), id, now))?;
                let result = match u.kind.as_str() {
                    "status" => self.tracker.update_status(&ticket, &u.payload).await,
                    "close" => self.tracker.close_ticket(&ticket).await,
                    _ => {
                        let body = format!("{}\n{}", u.payload, marker_for(&u.idempotency_key));
                        self.tracker.comment_evidence(&ticket, &body).await
                    }
                };
                match result {
                    Ok(()) => {
                        self.done(id)?;
                        report.done += 1;
                    }
                    Err(e) if e.code() == Some(ErrorCode::PolicyDenied) => {
                        self.fail(id, "the remote refused the update")?;
                        report.failed += 1;
                    }
                    Err(e) => {
                        let mut after = u.clone();
                        after.attempts += 1;
                        self.retry(&after, &e.to_string())?;
                        report.retried += 1;
                    }
                }
            }
        }
        Ok(())
    }

    fn done(&self, id: i64) -> Result<(), VelaError> {
        let now = self.clock.now_ms();
        self.store
            .write(move |tx| queue::mark_done(tx.conn(), id, now))
    }

    fn fail(&self, id: i64, why: &str) -> Result<(), VelaError> {
        let (now, why) = (self.clock.now_ms(), why.to_owned());
        self.store
            .write(move |tx| queue::mark_permanent(tx.conn(), id, &why, now))
    }

    fn retry(&self, u: &RemoteUpdate, why: &str) -> Result<(), VelaError> {
        let now = self.clock.now_ms();
        let next =
            now + i64::try_from(backoff_ms(u.attempts.max(1), (self.jitter)())).unwrap_or(i64::MAX);
        let (id, why) = (u.id, why.to_owned());
        self.store
            .write(move |tx| queue::mark_retry(tx.conn(), id, next, &why, now))
    }
}

#[cfg(test)]
mod tests;
