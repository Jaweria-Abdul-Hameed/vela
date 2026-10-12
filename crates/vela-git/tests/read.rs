//! G01: read operations under the hardened invocation, including the hostile-repository fixture.

mod common;

use std::process::Command;

use vela_domain::ports::{GitRead, RepoTrust, RevisionSpec};
use vela_testkit::fixture_repo::{FixtureKind, FixtureRepo};

use common::{ctx, git_cli, wt};

#[tokio::test]
async fn reads_report_head_branch_changes_and_diffs() {
    let repo = FixtureRepo::create(FixtureKind::MultiPackage).unwrap();
    let path = wt(repo.path());
    let git = git_cli();
    let c = ctx(RepoTrust::Trusted);

    let status = git.status(&c, &path).await.unwrap();
    assert!(status.is_clean());
    assert_eq!(status.branch.as_ref().map(|b| b.as_str()), Some("main"));
    let head = status.head.clone().unwrap();
    assert_eq!(head.as_str(), repo.head().unwrap());

    std::fs::write(repo.path().join("new file.txt"), "hello\n").unwrap();
    let dirty = git.status(&c, &path).await.unwrap();
    assert_eq!(dirty.changed_paths, vec!["new file.txt".to_owned()]);

    let resolved = git
        .resolve_ref(&c, &path, &RevisionSpec::new("HEAD").unwrap())
        .await
        .unwrap();
    assert_eq!(resolved, head);
    let missing = git
        .resolve_ref(&c, &path, &RevisionSpec::new("no-such-branch").unwrap())
        .await;
    assert!(missing.is_err());

    assert_eq!(git.merge_base(&c, &path, &head, &head).await.unwrap(), head);
    assert!(
        !git.diff_summary(&c, &path, &head, &head)
            .await
            .unwrap()
            .is_non_empty()
    );
    assert!(git.version(&path).await.unwrap().starts_with("git version"));
    let log = git
        .log_oneline(&c, &path, &RevisionSpec::new("HEAD").unwrap(), 5)
        .await
        .unwrap();
    assert_eq!(log.len(), 1);
}

#[tokio::test]
async fn a_branch_diff_lists_changed_paths() {
    let repo = FixtureRepo::create(FixtureKind::SeededReviewDefect).unwrap();
    let path = wt(repo.path());
    let git = git_cli();
    let c = ctx(RepoTrust::Trusted);
    let main = git
        .resolve_ref(&c, &path, &RevisionSpec::new("main").unwrap())
        .await
        .unwrap();
    let branch = git
        .resolve_ref(
            &c,
            &path,
            &RevisionSpec::new(vela_testkit::fixture_repo::DEFECT_BRANCH).unwrap(),
        )
        .await
        .unwrap();
    let diff = git.diff_summary(&c, &path, &main, &branch).await.unwrap();
    assert!(diff.is_non_empty());
    assert_ne!(
        git.merge_base(&c, &path, &main, &branch).await.unwrap(),
        branch
    );
}

/// A negative control first: on this host the hostile fixture really does run its payloads when Git is invoked without the
/// hardened arguments, so the assertion below is not vacuous.
#[test]
fn the_hostile_fixture_fires_its_payloads_without_hardening() {
    let repo = FixtureRepo::create(FixtureKind::HostileLocalConfig).unwrap();
    for args in [
        vec!["status"],
        vec!["diff", "HEAD~0"],
        vec!["log", "-p", "-1"],
    ] {
        let _ = Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(&args)
            .output()
            .unwrap();
    }
    assert!(
        !repo.fired_markers().unwrap().is_empty(),
        "the hostile repository must fire at least one payload on a plain git invocation"
    );
}

#[tokio::test]
async fn the_hostile_repository_executes_no_marker_under_any_read_operation() {
    let repo = FixtureRepo::create(FixtureKind::HostileLocalConfig).unwrap();
    let path = wt(repo.path());
    let git = git_cli();
    for trust in [RepoTrust::Untrusted, RepoTrust::Trusted] {
        let c = ctx(trust);
        let status = git.status(&c, &path).await.unwrap();
        let head = status.head.clone().unwrap();
        git.resolve_ref(&c, &path, &RevisionSpec::new("HEAD").unwrap())
            .await
            .unwrap();
        git.merge_base(&c, &path, &head, &head).await.unwrap();
        git.diff_summary(&c, &path, &head, &head).await.unwrap();
        git.log_oneline(&c, &path, &RevisionSpec::new("HEAD").unwrap(), 3)
            .await
            .unwrap();
        git.version(&path).await.unwrap();
    }
    assert_eq!(repo.fired_markers().unwrap(), Vec::<String>::new());
}

#[tokio::test]
async fn a_folder_that_is_not_a_repository_is_a_typed_error() {
    let dir = std::env::temp_dir().join(format!("vela-git-norepo-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let git = git_cli();
    let err = git
        .status(&ctx(RepoTrust::Trusted), &wt(&dir))
        .await
        .unwrap_err();
    assert!(
        err.what_failed().contains("not a Git repository"),
        "{}",
        err.what_failed()
    );
    let _ = std::fs::remove_dir_all(&dir);
}
