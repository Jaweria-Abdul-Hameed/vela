use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use vela_persistence::writer::StoreOptions;
use vela_testkit::fakes::{FakeClock, FakeTracker};

use super::*;

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "vela-rq-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn setup() -> (
    TestDir,
    Arc<FakeTracker>,
    Arc<FakeClock>,
    RemoteQueue,
    TicketId,
) {
    let dir = TestDir::new();
    let store =
        Arc::new(StateStore::open(&dir.0.join("state.db"), StoreOptions::default()).unwrap());
    let tracker = Arc::new(FakeTracker::new());
    let clock = Arc::new(FakeClock::new(1_000_000));
    let id = TicketId::new("gh-1").unwrap();
    tracker.add(vela_domain::ports::TrackerTicket {
        id: id.clone(),
        title: "t".into(),
        body: String::new(),
        blockers: vec![],
        labels: vec![],
    });
    let q = RemoteQueue::new(store, tracker.clone(), clock.clone(), Arc::new(|| 500));
    (dir, tracker, clock, q, id)
}

#[test]
fn backoff_is_exponential_bounded_and_jittered_within_the_spread() {
    assert_eq!(
        schedule(8),
        vec![
            2_000, 4_000, 8_000, 16_000, 32_000, 64_000, 128_000, 256_000
        ]
    );
    assert_eq!(backoff_ms(9, 500), 300_000);
    assert_eq!(backoff_ms(500, 500), 300_000, "never longer than the cap");
    assert_eq!(backoff_ms(1, 0), 1_600);
    assert_eq!(backoff_ms(1, 1000), 2_400);
    for attempt in 1..30 {
        for j in [0, 250, 500, 750, 1000, 5000] {
            let d = backoff_ms(attempt, j);
            assert!(
                (1_600..=MAX_DELAY_MS * 12 / 10).contains(&d),
                "{attempt} {j} {d}"
            );
        }
    }
}

#[test]
fn idempotency_decision_table() {
    use Decision::*;
    assert_eq!(decide("status", 0, None), Post);
    assert_eq!(decide("status", 5, None), Post);
    assert_eq!(decide("close", 3, Some(false)), Post);
    assert_eq!(decide("comment", 0, None), Post);
    assert_eq!(decide("comment", 1, Some(true)), AlreadyDone);
    assert_eq!(decide("comment", 1, Some(false)), Post);
    assert_eq!(decide("comment", 2, None), RetryLater);
    assert_eq!(decide("weird", 0, None), RetryLater);
}

#[tokio::test]
async fn an_update_is_applied_once_and_the_same_key_is_not_queued_twice() {
    let (_d, tracker, _clock, q, id) = setup();
    assert!(q.enqueue(None, &id, "comment", "evidence", "k1").unwrap());
    assert!(!q.enqueue(None, &id, "comment", "evidence", "k1").unwrap());
    let r = q.drain().await.unwrap();
    assert_eq!(r.done, 1);
    assert_eq!(tracker.comments().len(), 1);
    assert!(tracker.comments()[0].1.contains(&marker_for("k1")));
    assert_eq!(q.pending().unwrap(), 0);
    assert_eq!(q.drain().await.unwrap(), DrainReport::default());
    assert_eq!(tracker.comments().len(), 1);
}

#[tokio::test]
async fn an_outage_retains_updates_and_backoff_gates_the_retry() {
    let (_d, tracker, clock, q, id) = setup();
    tracker.set_outage(true);
    q.enqueue(None, &id, "status", "in-progress", "s1").unwrap();
    q.enqueue(None, &id, "comment", "tests passed", "c1")
        .unwrap();
    let r = q.drain().await.unwrap();
    assert_eq!((r.done, r.retried), (0, 2));
    assert_eq!(q.pending().unwrap(), 2, "nothing is lost");
    // Not yet due: the backoff delay has not elapsed.
    assert_eq!(q.drain().await.unwrap(), DrainReport::default());
    clock.advance(2_000);
    tracker.set_outage(false);
    let r = q.drain().await.unwrap();
    assert_eq!(r.done, 2);
    assert_eq!(tracker.status_of(&id).as_deref(), Some("in-progress"));
    assert_eq!(tracker.comments().len(), 1);
}

#[tokio::test]
async fn a_retried_comment_never_duplicates_when_the_first_attempt_actually_landed() {
    let (_d, tracker, clock, q, id) = setup();
    q.enqueue(None, &id, "comment", "evidence", "k9").unwrap();
    // Simulate "posted, but the process died before marking it done": the comment exists and attempts is already 1.
    IssueTrackerAdapter::comment_evidence(
        &*tracker,
        &id,
        &format!(
            "evidence
{}",
            marker_for("k9")
        ),
    )
    .await
    .unwrap();
    q.store
        .write(|tx| {
            let now = 1;
            queue::begin_attempt(tx.conn(), 1, now)
        })
        .unwrap();
    clock.advance(10);
    let r = q.drain().await.unwrap();
    assert_eq!(r.done, 1);
    assert_eq!(tracker.comments().len(), 1, "no duplicate comment");
}

#[tokio::test]
async fn a_retried_comment_that_is_missing_is_posted_once_the_outage_ends() {
    let (_d, tracker, clock, q, id) = setup();
    q.enqueue(None, &id, "comment", "evidence", "k2").unwrap();
    q.store
        .write(|tx| queue::begin_attempt(tx.conn(), 1, 1))
        .unwrap();
    tracker.set_outage(true);
    let r = q.drain().await.unwrap();
    // The lookup finds nothing, so the post is attempted and fails during the outage.
    assert_eq!(r.retried, 1);
    assert!(tracker.comments().is_empty());
    clock.advance(600_000);
    tracker.set_outage(false);
    assert_eq!(q.drain().await.unwrap().done, 1);
    assert_eq!(tracker.comments().len(), 1);
}
