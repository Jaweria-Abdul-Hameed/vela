//! G02: worktrees under the managed root.

mod common;

use std::path::{Path, PathBuf};

use vela_domain::ids::{BranchName, CommitSha};
use vela_domain::ports::{GitRead, GitWorktrees, RepoTrust, RevisionSpec, WorktreeRequest};
use vela_testkit::fixture_repo::{FixtureKind, FixtureRepo, ScratchDir};

use common::{ctx, git_cli, wt};

struct Setup {
    repo: FixtureRepo,
    root: PathBuf,
    _scratch: ScratchDir,
}

fn setup() -> Setup {
    let repo = FixtureRepo::create(FixtureKind::MultiPackage).unwrap();
    let scratch = ScratchDir::create("worktrees").unwrap();
    let root = scratch.path().join("managed");
    std::fs::create_dir_all(&root).unwrap();
    Setup {
        repo,
        root,
        _scratch: scratch,
    }
}

async fn head_of(git: &vela_git::read::GitCli, s: &Setup) -> CommitSha {
    git.resolve_ref(
        &ctx(RepoTrust::Trusted),
        &wt(s.repo.path()),
        &RevisionSpec::new("HEAD").unwrap(),
    )
    .await
    .unwrap()
}

fn request(s: &Setup, dir: &str, branch: &str, base: &CommitSha) -> WorktreeRequest {
    WorktreeRequest {
        repo: wt(s.repo.path()),
        path: wt(&s.root.join(dir)),
        branch: BranchName::new(branch).unwrap(),
        base: base.clone(),
    }
}

#[tokio::test]
async fn a_worktree_is_created_at_the_recorded_commit_and_listed() {
    let s = setup();
    let git = git_cli().with_managed_root(s.root.clone());
    let c = ctx(RepoTrust::Trusted);
    let base = head_of(&git, &s).await;
    let info = git
        .create_worktree(&c, &request(&s, "ticket-a", "vela/run-1/ticket-a", &base))
        .await
        .unwrap();
    assert_eq!(info.head, base);
    assert_eq!(
        info.branch.as_ref().map(|b| b.as_str()),
        Some("vela/run-1/ticket-a")
    );
    assert!(s.root.join("ticket-a").join(".git").exists());
    let listed = git.list_worktrees(&c, &wt(s.repo.path())).await.unwrap();
    assert_eq!(listed.len(), 2, "the primary checkout and the new worktree");
    assert!(
        listed
            .iter()
            .any(|w| w.branch.as_ref().map(|b| b.as_str()) == Some("vela/run-1/ticket-a"))
    );
    // The primary checkout was not touched.
    assert_eq!(head_of(&git, &s).await, base);
    assert!(git.status(&c, &wt(s.repo.path())).await.unwrap().is_clean());
}

#[tokio::test]
async fn a_root_under_a_synced_folder_or_a_path_outside_the_root_is_refused() {
    let s = setup();
    let c = ctx(RepoTrust::Trusted);
    let base = head_of(&git_cli(), &s).await;

    let synced =
        git_cli().with_managed_root(PathBuf::from("C:/Users/someone/OneDrive/vela-worktrees"));
    let mut req = request(&s, "x", "vela/x", &base);
    req.path = wt(Path::new("C:/Users/someone/OneDrive/vela-worktrees/x"));
    let err = synced.create_worktree(&c, &req).await.unwrap_err();
    assert!(
        err.what_failed().contains("OneDrive"),
        "{}",
        err.what_failed()
    );

    let git = git_cli().with_managed_root(s.root.clone());
    let outside = s.root.parent().unwrap().join("elsewhere");
    let mut req = request(&s, "x", "vela/x", &base);
    req.path = wt(&outside);
    let err = git.create_worktree(&c, &req).await.unwrap_err();
    assert!(err.what_failed().contains("below the Vela-managed root"));
    assert!(!outside.exists());

    let unconfigured = git_cli();
    assert!(
        unconfigured
            .create_worktree(&c, &request(&s, "y", "vela/y", &base))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn a_second_worktree_at_a_path_that_differs_only_in_case_is_refused() {
    let s = setup();
    let git = git_cli().with_managed_root(s.root.clone());
    let c = ctx(RepoTrust::Trusted);
    let base = head_of(&git, &s).await;
    git.create_worktree(&c, &request(&s, "Ticket-A", "vela/a1", &base))
        .await
        .unwrap();
    let err = git
        .create_worktree(&c, &request(&s, "ticket-a", "vela/a2", &base))
        .await
        .unwrap_err();
    assert!(
        err.what_failed().contains("already exists"),
        "{}",
        err.what_failed()
    );
}

#[tokio::test]
async fn cleanup_removes_only_clean_worktrees_under_the_root() {
    let s = setup();
    let git = git_cli().with_managed_root(s.root.clone());
    let c = ctx(RepoTrust::Trusted);
    let repo_path = wt(s.repo.path());
    let base = head_of(&git, &s).await;
    git.create_worktree(&c, &request(&s, "clean", "vela/clean", &base))
        .await
        .unwrap();
    git.create_worktree(&c, &request(&s, "dirty", "vela/dirty", &base))
        .await
        .unwrap();
    std::fs::write(s.root.join("dirty").join("scratch.txt"), "uncommitted").unwrap();

    git.remove_worktree(&c, &repo_path, &wt(&s.root.join("clean")))
        .await
        .unwrap();
    assert!(!s.root.join("clean").exists());

    let err = git
        .remove_worktree(&c, &repo_path, &wt(&s.root.join("dirty")))
        .await
        .unwrap_err();
    assert!(
        err.what_failed().contains("uncommitted"),
        "{}",
        err.what_failed()
    );
    assert!(s.root.join("dirty").join("scratch.txt").exists());

    let err = git
        .remove_worktree(&c, &repo_path, &repo_path)
        .await
        .unwrap_err();
    assert!(err.what_failed().contains("below the Vela-managed root"));
    assert!(s.repo.path().join(".git").exists());
}

/// A file held open with no sharing makes the folder undeletable on Windows. Removal retries with bounded backoff and then
/// reports; it never kills the holder or forces the removal.
#[cfg(windows)]
#[tokio::test]
async fn a_locked_folder_is_retried_then_reported_not_forced() {
    let s = setup();
    let git = git_cli().with_managed_root(s.root.clone());
    let c = ctx(RepoTrust::Trusted);
    let base = head_of(&git, &s).await;
    git.create_worktree(&c, &request(&s, "held", "vela/held", &base))
        .await
        .unwrap();
    // A terminal sitting in the folder: a process whose working directory is the worktree keeps the folder from being deleted.
    let mut holder = std::process::Command::new("cmd")
        .args(["/C", "ping -n 30 127.0.0.1 >nul"])
        .current_dir(s.root.join("held"))
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    let started = std::time::Instant::now();
    let result = git
        .remove_worktree(&c, &wt(s.repo.path()), &wt(&s.root.join("held")))
        .await;
    let _ = holder.kill();
    let _ = holder.wait();
    let err = result.unwrap_err();
    let text = err.what_failed();
    assert!(text.contains("in use by another process"), "{text}");
    assert!(
        started.elapsed() >= std::time::Duration::from_millis(1000),
        "it must have retried with backoff"
    );
    assert!(s.root.join("held").exists(), "nothing was force-removed");
}
