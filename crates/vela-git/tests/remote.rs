//! G05: fetch and push against a local bare remote.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use vela_domain::errors::ErrorClass;
use vela_domain::ids::BranchName;
use vela_domain::ports::{GitRemote, RemoteName, RepoTrust};
use vela_testkit::fixture_repo::{FixtureKind, FixtureRepo, ScratchDir};

use common::{ctx, git_cli, wt};

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

struct Remote {
    repo: FixtureRepo,
    bare: PathBuf,
    counter: PathBuf,
    scratch: ScratchDir,
}

/// A fixture repository with `origin` pointing at a bare repository whose pre-receive hook counts the pushes it accepts.
fn remote() -> Remote {
    let repo = FixtureRepo::create(FixtureKind::MultiPackage).unwrap();
    let scratch = ScratchDir::create("remote").unwrap();
    let bare = scratch.path().join("origin.git");
    git(
        scratch.path(),
        &[
            "init",
            "--bare",
            "--initial-branch=main",
            bare.to_str().unwrap(),
        ],
    );
    let counter = scratch.path().join("pushes.log");
    let hook = bare.join("hooks").join("pre-receive");
    std::fs::write(
        &hook,
        format!(
            "#!/bin/sh\necho push >> '{}'\n",
            counter.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
    repo.git(&["remote", "add", "origin", bare.to_str().unwrap()])
        .unwrap();
    Remote {
        repo,
        bare,
        counter,
        scratch,
    }
}

fn pushes(r: &Remote) -> usize {
    std::fs::read_to_string(&r.counter)
        .map(|s| s.lines().count())
        .unwrap_or(0)
}

fn origin() -> RemoteName {
    RemoteName::new("origin").unwrap()
}

fn main_branch() -> BranchName {
    BranchName::new("main").unwrap()
}

#[tokio::test]
async fn a_push_that_already_happened_is_detected_and_not_repeated() {
    let r = remote();
    let g = git_cli();
    let c = ctx(RepoTrust::Trusted);
    let head = r.repo.head().unwrap();

    let first = g
        .push(&c, &wt(r.repo.path()), &origin(), &main_branch())
        .await
        .unwrap();
    assert_eq!(first.remote_head.as_str(), head);
    assert_eq!(git(&r.bare, &["rev-parse", "refs/heads/main"]), head);
    assert_eq!(pushes(&r), 1);

    let second = g
        .push(&c, &wt(r.repo.path()), &origin(), &main_branch())
        .await
        .unwrap();
    assert_eq!(second.remote_head.as_str(), head);
    assert_eq!(
        pushes(&r),
        1,
        "the second push must be recognized and skipped"
    );
}

#[tokio::test]
async fn a_new_commit_is_pushed_without_forcing() {
    let r = remote();
    let g = git_cli();
    let c = ctx(RepoTrust::Trusted);
    g.push(&c, &wt(r.repo.path()), &origin(), &main_branch())
        .await
        .unwrap();
    std::fs::write(r.repo.path().join("more.txt"), "more\n").unwrap();
    r.repo.git(&["add", "-A"]).unwrap();
    r.repo.git(&["commit", "-m", "more"]).unwrap();
    let head = r.repo.head().unwrap();
    let out = g
        .push(&c, &wt(r.repo.path()), &origin(), &main_branch())
        .await
        .unwrap();
    assert_eq!(out.remote_head.as_str(), head);
    assert_eq!(pushes(&r), 2);
}

#[tokio::test]
async fn a_non_fast_forward_push_is_a_distinct_typed_error_and_the_remote_is_untouched() {
    let r = remote();
    let g = git_cli();
    let c = ctx(RepoTrust::Trusted);
    g.push(&c, &wt(r.repo.path()), &origin(), &main_branch())
        .await
        .unwrap();

    // Someone else advances the remote.
    let other = r.scratch.path().join("other");
    git(
        r.scratch.path(),
        &[
            "clone",
            "-q",
            r.bare.to_str().unwrap(),
            other.to_str().unwrap(),
        ],
    );
    git(
        &other,
        &[
            "-c",
            "user.name=o",
            "-c",
            "user.email=o@example.com",
            "commit",
            "--allow-empty",
            "-m",
            "remote work",
        ],
    );
    git(&other, &["push", "-q", "origin", "main"]);
    let remote_tip = git(&r.bare, &["rev-parse", "refs/heads/main"]);

    // The local branch diverges.
    std::fs::write(r.repo.path().join("local.txt"), "local\n").unwrap();
    r.repo.git(&["add", "-A"]).unwrap();
    r.repo.git(&["commit", "-m", "local work"]).unwrap();

    let err = g
        .push(&c, &wt(r.repo.path()), &origin(), &main_branch())
        .await
        .unwrap_err();
    assert_eq!(
        err.class(),
        ErrorClass::RepositoryConflict,
        "{}",
        err.what_failed()
    );
    assert_eq!(
        git(&r.bare, &["rev-parse", "refs/heads/main"]),
        remote_tip,
        "nothing was forced"
    );
}

#[tokio::test]
async fn fetch_reports_the_remote_tracking_refs_that_changed() {
    let r = remote();
    let g = git_cli();
    let c = ctx(RepoTrust::Trusted);
    g.push(&c, &wt(r.repo.path()), &origin(), &main_branch())
        .await
        .unwrap();
    // Whether the push already updated the remote-tracking ref depends on the Git version, so only the settled state is asserted.
    let _first = g.fetch(&c, &wt(r.repo.path()), &origin()).await.unwrap();
    let again = g.fetch(&c, &wt(r.repo.path()), &origin()).await.unwrap();
    assert!(again.updated_refs.is_empty());

    let other = r.scratch.path().join("other2");
    git(
        r.scratch.path(),
        &[
            "clone",
            "-q",
            r.bare.to_str().unwrap(),
            other.to_str().unwrap(),
        ],
    );
    git(
        &other,
        &[
            "-c",
            "user.name=o",
            "-c",
            "user.email=o@example.com",
            "commit",
            "--allow-empty",
            "-m",
            "more",
        ],
    );
    git(&other, &["push", "-q", "origin", "main"]);
    let after = g.fetch(&c, &wt(r.repo.path()), &origin()).await.unwrap();
    // Newer Git versions also move the symbolic `origin/HEAD`; what matters is that the branch ref is reported.
    assert!(
        after
            .updated_refs
            .contains(&"refs/remotes/origin/main".to_owned()),
        "{:?}",
        after.updated_refs
    );
}

#[tokio::test]
async fn an_unreachable_remote_is_a_network_error_not_an_authentication_error() {
    let repo = FixtureRepo::create(FixtureKind::MultiPackage).unwrap();
    repo.git(&["remote", "add", "origin", "https://127.0.0.1:1/nothing.git"])
        .unwrap();
    let g = git_cli();
    let c = ctx(RepoTrust::Trusted);
    let err = g
        .push(&c, &wt(repo.path()), &origin(), &main_branch())
        .await
        .unwrap_err();
    assert_eq!(
        err.class(),
        ErrorClass::ExternalTemporary,
        "{}",
        err.what_failed()
    );
    assert!(err.code().is_none());
    assert!(err.what_failed().contains("network"));
    let probe = g
        .ls_remote_probe(&c, &wt(repo.path()), &origin())
        .await
        .unwrap_err();
    assert_eq!(probe.class(), ErrorClass::ExternalTemporary);
}

#[tokio::test]
async fn the_probe_reaches_a_local_remote_and_pushing_a_missing_branch_is_refused() {
    let r = remote();
    let g = git_cli();
    let c = ctx(RepoTrust::Trusted);
    assert!(
        g.ls_remote_probe(&c, &wt(r.repo.path()), &origin())
            .await
            .unwrap()
    );
    let err = g
        .push(
            &c,
            &wt(r.repo.path()),
            &origin(),
            &BranchName::new("no-such-branch").unwrap(),
        )
        .await
        .unwrap_err();
    assert!(
        err.what_failed().contains("does not exist locally"),
        "{}",
        err.what_failed()
    );
}

#[tokio::test]
async fn a_hostile_repository_runs_no_payload_during_remote_operations() {
    let repo = FixtureRepo::create(FixtureKind::HostileLocalConfig).unwrap();
    let scratch = ScratchDir::create("hostile-remote").unwrap();
    let bare = scratch.path().join("origin.git");
    git(
        scratch.path(),
        &[
            "init",
            "--bare",
            "--initial-branch=main",
            bare.to_str().unwrap(),
        ],
    );
    git(
        repo.path(),
        &[
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.hooksPath=NUL",
            "remote",
            "add",
            "origin",
            bare.to_str().unwrap(),
        ],
    );
    let g = git_cli();
    let c = ctx(RepoTrust::Untrusted);
    let _ = g
        .push(&c, &wt(repo.path()), &origin(), &main_branch())
        .await;
    let _ = g.fetch(&c, &wt(repo.path()), &origin()).await;
    assert_eq!(repo.fired_markers().unwrap(), Vec::<String>::new());
}
