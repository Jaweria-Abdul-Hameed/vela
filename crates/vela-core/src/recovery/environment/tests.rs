use std::sync::Mutex;

use vela_domain::errors::ErrorClass;
use vela_domain::ids::OperationId;
use vela_domain::ports::{GitRead, RepoTrust, ready};
use vela_testkit::fakes::FakeGit;

use super::*;

struct Scripted {
    ran: Mutex<Vec<Check>>,
    fail: Option<(Check, &'static str)>,
}

impl WakeChecks for Scripted {
    fn run<'a>(&'a self, check: Check) -> PortFuture<'a, ()> {
        self.ran.lock().unwrap().push(check);
        match self.fail {
            Some((c, why)) if c == check => {
                ready(Err(VelaError::of_class(ErrorClass::ExternalTemporary, why)))
            }
            _ => ready(Ok(())),
        }
    }
}

#[test]
fn a_gap_is_detected_with_a_fake_clock() {
    let d = GapDetector {
        interval_ms: 1_000,
        tolerance: 4,
    };
    assert_eq!(d.gap(10_000, 10_900), None);
    assert_eq!(d.gap(10_000, 14_000), None, "exactly at the tolerance");
    assert_eq!(d.gap(10_000, 14_001), Some(4_001));
    assert_eq!(d.gap(10_000, 3_600_000), Some(3_590_000));
    assert_eq!(d.gap(10_000, 5_000), None, "a clock that went backwards");
    assert_eq!(d.gap(i64::MIN, i64::MAX), None, "overflow is not a gap");
}

#[tokio::test]
async fn the_rechecks_run_processes_then_locks_then_network_then_authentication() {
    let s = Scripted {
        ran: Mutex::new(Vec::new()),
        fail: None,
    };
    let out = post_wake(&s).await;
    assert!(out.may_resume());
    assert_eq!(
        *s.ran.lock().unwrap(),
        [
            Check::Processes,
            Check::Locks,
            Check::Network,
            Check::Authentication
        ]
    );
}

#[tokio::test]
async fn the_first_failure_stops_the_rest_and_blocks_resuming() {
    let s = Scripted {
        ran: Mutex::new(Vec::new()),
        fail: Some((Check::Network, "no route to host")),
    };
    let out = post_wake(&s).await;
    assert!(!out.may_resume());
    assert_eq!(out.passed, [Check::Processes, Check::Locks]);
    assert_eq!(
        out.failed,
        Some((Check::Network, "no route to host".into()))
    );
    assert!(
        !s.ran.lock().unwrap().contains(&Check::Authentication),
        "authentication is never tried over a dead network"
    );
}

fn ctx() -> GitContext {
    GitContext {
        trust: RepoTrust::Trusted,
        operation: OperationId::new("op-1").unwrap(),
    }
}

fn down() -> VelaError {
    VelaError::of_class(ErrorClass::ExternalTemporary, "network is unreachable")
}

#[tokio::test]
async fn a_push_lost_to_the_network_is_retried_and_succeeds_once_it_returns() {
    let git = FakeGit::new();
    git.fail_next_pushes(vec![down(), down()]);
    let repo = WorktreePath::new("C:/repo").unwrap();
    let remote = RemoteName::new("origin").unwrap();
    let branch = BranchName::new("vela/run-1/integration").unwrap();
    let head_before = git
        .resolve_ref(
            &ctx(),
            &repo,
            &vela_domain::ports::RevisionSpec::new("HEAD").unwrap(),
        )
        .await
        .unwrap();
    let out = push_retrying(&git, &ctx(), &repo, &remote, &branch, 4, Duration::ZERO)
        .await
        .unwrap();
    assert_eq!(out.remote_head, head_before);
    let pushes = git
        .log()
        .calls()
        .iter()
        .filter(|c| c.starts_with("push:"))
        .count();
    assert_eq!(pushes, 3, "two failures and then the push that landed");
    // Local state is exactly as it was: the failed pushes changed nothing.
    let head_after = git
        .resolve_ref(
            &ctx(),
            &repo,
            &vela_domain::ports::RevisionSpec::new("HEAD").unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(head_before, head_after);
    // Pushing again is idempotent.
    let again = push_retrying(&git, &ctx(), &repo, &remote, &branch, 1, Duration::ZERO)
        .await
        .unwrap();
    assert_eq!(again.remote_head, out.remote_head);
}

#[tokio::test]
async fn an_outage_that_outlasts_the_attempts_is_reported_not_swallowed() {
    let git = FakeGit::new();
    git.fail_next_pushes(vec![down(), down(), down()]);
    let repo = WorktreePath::new("C:/repo").unwrap();
    let r = push_retrying(
        &git,
        &ctx(),
        &repo,
        &RemoteName::new("origin").unwrap(),
        &BranchName::new("vela/run-1/integration").unwrap(),
        2,
        Duration::ZERO,
    )
    .await;
    assert!(r.unwrap_err().class().is_retryable());
}

#[tokio::test]
async fn a_rejected_push_is_never_retried() {
    let git = FakeGit::new();
    git.fail_next_pushes(vec![VelaError::of_class(
        ErrorClass::ValidationFailure,
        "non-fast-forward",
    )]);
    let repo = WorktreePath::new("C:/repo").unwrap();
    let r = push_retrying(
        &git,
        &ctx(),
        &repo,
        &RemoteName::new("origin").unwrap(),
        &BranchName::new("vela/run-1/integration").unwrap(),
        5,
        Duration::ZERO,
    )
    .await;
    assert_eq!(r.unwrap_err().class(), ErrorClass::ValidationFailure);
    assert_eq!(
        git.log()
            .calls()
            .iter()
            .filter(|c| c.starts_with("push:"))
            .count(),
        1
    );
}
