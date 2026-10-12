//! G03: checkpoint commits.

mod common;

use vela_domain::ports::{CheckpointMessage, GitCheckpoints, GitRead, RepoTrust, Trailer};
use vela_git::read::is_index_locked;
use vela_testkit::fixture_repo::{FixtureKind, FixtureRepo};

use common::{ctx, git_cli, wt};

fn message() -> CheckpointMessage {
    CheckpointMessage::new(
        "checkpoint: implement login",
        vec![
            Trailer::new("Vela-Run", "run-1").unwrap(),
            Trailer::new("Vela-Ticket", "T-42").unwrap(),
        ],
    )
    .unwrap()
}

fn dirty(repo: &FixtureRepo) {
    std::fs::write(repo.path().join("login.txt"), "work in progress\n").unwrap();
}

#[tokio::test]
async fn a_clean_worktree_has_nothing_to_checkpoint() {
    let repo = FixtureRepo::create(FixtureKind::MultiPackage).unwrap();
    let git = git_cli();
    let before = repo.head().unwrap();
    let err = git
        .commit_checkpoint(&ctx(RepoTrust::Trusted), &wt(repo.path()), &message())
        .await
        .unwrap_err();
    assert!(
        err.what_failed().contains("nothing to checkpoint"),
        "{}",
        err.what_failed()
    );
    assert_eq!(repo.head().unwrap(), before);
}

#[tokio::test]
async fn changes_become_one_commit_with_run_and_ticket_trailers() {
    let repo = FixtureRepo::create(FixtureKind::MultiPackage).unwrap();
    let git = git_cli();
    let c = ctx(RepoTrust::Trusted);
    let before = repo.head().unwrap();
    dirty(&repo);
    let head = git
        .commit_checkpoint(&c, &wt(repo.path()), &message())
        .await
        .unwrap();
    assert_ne!(head.as_str(), before);
    assert_eq!(head.as_str(), repo.head().unwrap());
    assert!(git.status(&c, &wt(repo.path())).await.unwrap().is_clean());
    let trailers = repo
        .git(&["log", "-1", "--format=%(trailers:only,unfold)"])
        .unwrap();
    assert!(trailers.contains("Vela-Run: run-1"), "{trailers}");
    assert!(trailers.contains("Vela-Ticket: T-42"), "{trailers}");
    let subject = repo.git(&["log", "-1", "--format=%s"]).unwrap();
    assert_eq!(subject.trim(), "checkpoint: implement login");
    assert_eq!(
        repo.git(&["rev-list", "--count", "HEAD"]).unwrap().trim(),
        "2"
    );
}

#[tokio::test]
async fn work_the_agent_already_committed_counts_as_the_checkpoint() {
    let repo = FixtureRepo::create(FixtureKind::MultiPackage).unwrap();
    let git = git_cli();
    let c = ctx(RepoTrust::Trusted);
    let fixed = vela_domain::ids::CommitSha::new(repo.head().unwrap()).unwrap();
    std::fs::write(repo.path().join("agent.txt"), "committed by the agent\n").unwrap();
    repo.git(&["add", "-A"]).unwrap();
    repo.git(&["commit", "-m", "agent commit"]).unwrap();
    let head = git
        .checkpoint(&c, &wt(repo.path()), &message(), Some(&fixed))
        .await
        .unwrap();
    assert_eq!(head.as_str(), repo.head().unwrap());
    assert_ne!(head, fixed);
    assert_eq!(
        repo.git(&["rev-list", "--count", "HEAD"]).unwrap().trim(),
        "2",
        "no extra commit"
    );
}

fn install_failing_hook(repo: &FixtureRepo) {
    let hooks = repo.path().join(".git").join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    std::fs::write(
        hooks.join("pre-commit"),
        "#!/bin/sh\necho 'lint failed' >&2\nexit 1\n",
    )
    .unwrap();
}

#[tokio::test]
async fn a_failing_repository_hook_is_a_typed_failure_and_no_commit_exists() {
    let repo = FixtureRepo::create(FixtureKind::MultiPackage).unwrap();
    install_failing_hook(&repo);
    let git = git_cli();
    let before = repo.head().unwrap();
    dirty(&repo);
    let err = git
        .commit_checkpoint(&ctx(RepoTrust::Trusted), &wt(repo.path()), &message())
        .await
        .unwrap_err();
    assert_eq!(
        err.class(),
        vela_domain::errors::ErrorClass::ValidationFailure
    );
    assert!(
        err.what_failed().contains("rejected"),
        "{}",
        err.what_failed()
    );
    assert!(
        err.what_failed().contains("lint failed"),
        "the hook output is surfaced: {}",
        err.what_failed()
    );
    assert_eq!(
        repo.head().unwrap(),
        before,
        "no commit was made and the hook was not bypassed"
    );
}

#[tokio::test]
async fn an_untrusted_repositorys_hooks_never_run() {
    let repo = FixtureRepo::create(FixtureKind::MultiPackage).unwrap();
    install_failing_hook(&repo);
    let git = git_cli();
    dirty(&repo);
    let head = git
        .commit_checkpoint(&ctx(RepoTrust::Untrusted), &wt(repo.path()), &message())
        .await
        .unwrap();
    assert_eq!(head.as_str(), repo.head().unwrap());
}

#[tokio::test]
async fn a_locked_index_is_waited_out_then_reported_and_the_lock_is_never_deleted() {
    let repo = FixtureRepo::create(FixtureKind::MultiPackage).unwrap();
    let git = git_cli();
    dirty(&repo);
    let lock = repo.path().join(".git").join("index.lock");
    std::fs::write(&lock, "").unwrap();
    let started = std::time::Instant::now();
    let err = git
        .commit_checkpoint(&ctx(RepoTrust::Trusted), &wt(repo.path()), &message())
        .await
        .unwrap_err();
    assert!(is_index_locked(&err), "{}", err.what_failed());
    assert!(
        started.elapsed() >= std::time::Duration::from_millis(1500),
        "bounded waiting happened"
    );
    assert!(lock.exists(), "the lock file is never deleted");
}

#[tokio::test]
async fn the_hostile_repository_runs_no_payload_during_a_checkpoint() {
    let repo = FixtureRepo::create(FixtureKind::HostileLocalConfig).unwrap();
    let git = git_cli();
    dirty(&repo);
    let _ = git
        .commit_checkpoint(&ctx(RepoTrust::Untrusted), &wt(repo.path()), &message())
        .await;
    assert_eq!(repo.fired_markers().unwrap(), Vec::<String>::new());
}
