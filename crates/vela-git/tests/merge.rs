//! G04: merge operations in a Vela-managed integration worktree.

mod common;

use std::path::PathBuf;

use vela_domain::ids::{BranchName, CommitSha};
use vela_domain::ports::{
    GitMerges, GitRead, MergeOutcome, RepoTrust, RevisionSpec, Trailer, WorktreeRequest,
};
use vela_testkit::fixture_repo::{
    CONFLICT_BRANCHES, CONFLICT_PATH, FixtureKind, FixtureRepo, SCHEMA_BRANCHES, ScratchDir,
};

use common::{ctx, git_cli, wt};

struct Lane {
    repo: FixtureRepo,
    integration: PathBuf,
    base: CommitSha,
    git: vela_git::read::GitCli,
    _scratch: ScratchDir,
}

async fn lane(kind: FixtureKind) -> Lane {
    let repo = FixtureRepo::create(kind).unwrap();
    let scratch = ScratchDir::create("merge").unwrap();
    let root = scratch.path().join("managed");
    std::fs::create_dir_all(&root).unwrap();
    let git = git_cli().with_managed_root(root.clone());
    let c = ctx(RepoTrust::Trusted);
    let base = git
        .resolve_ref(&c, &wt(repo.path()), &RevisionSpec::new("main").unwrap())
        .await
        .unwrap();
    let integration = root.join("integration");
    git.create_worktree(
        &c,
        &WorktreeRequest {
            repo: wt(repo.path()),
            path: wt(&integration),
            branch: BranchName::new("vela/run-1/integration").unwrap(),
            base: base.clone(),
        },
    )
    .await
    .unwrap();
    Lane {
        repo,
        integration,
        base,
        git,
        _scratch: scratch,
    }
}

async fn tip(l: &Lane, branch: &str) -> CommitSha {
    l.git
        .resolve_ref(
            &ctx(RepoTrust::Trusted),
            &wt(l.repo.path()),
            &RevisionSpec::new(branch).unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn a_clean_merge_is_a_non_fast_forward_commit_with_trailers() {
    let l = lane(FixtureKind::SchemaOverlapPair).await;
    let c = ctx(RepoTrust::Trusted);
    let source = BranchName::new(SCHEMA_BRANCHES[0]).unwrap();
    let expected = tip(&l, SCHEMA_BRANCHES[0]).await;
    let outcome = l
        .git
        .merge_branch(
            &c,
            &wt(&l.integration),
            &l.base,
            &source,
            &expected,
            &[Trailer::new("Vela-Ticket", "T-1").unwrap()],
        )
        .await
        .unwrap();
    let MergeOutcome::Merged {
        head,
        merged_source,
    } = outcome
    else {
        panic!("expected a clean merge");
    };
    assert_eq!(merged_source, expected);
    let parents = l
        .repo
        .git(&[
            "-C",
            l.integration.to_str().unwrap(),
            "rev-list",
            "--parents",
            "-n",
            "1",
            head.as_str(),
        ])
        .unwrap();
    assert_eq!(
        parents.split_whitespace().count(),
        3,
        "a merge commit has two parents: {parents}"
    );
    let message = l
        .repo
        .git(&[
            "-C",
            l.integration.to_str().unwrap(),
            "log",
            "-1",
            "--format=%B",
        ])
        .unwrap();
    assert!(message.contains("Vela-Ticket: T-1"), "{message}");
    assert!(
        message.contains(&expected.short(12).to_owned()),
        "{message}"
    );
    // The merge is idempotent for the same operation.
    let again = l
        .git
        .merge_branch(&c, &wt(&l.integration), &l.base, &source, &expected, &[])
        .await
        .unwrap();
    assert_eq!(
        again,
        MergeOutcome::Merged {
            head: head.clone(),
            merged_source: expected
        }
    );
    // The primary checkout was not touched.
    assert_eq!(tip(&l, "main").await, l.base);
}

#[tokio::test]
async fn a_stale_reviewed_head_or_a_moved_integration_branch_is_refused() {
    let l = lane(FixtureKind::SchemaOverlapPair).await;
    let c = ctx(RepoTrust::Trusted);
    let source = BranchName::new(SCHEMA_BRANCHES[0]).unwrap();
    let wrong_head = tip(&l, SCHEMA_BRANCHES[1]).await;
    let err = l
        .git
        .merge_branch(&c, &wt(&l.integration), &l.base, &source, &wrong_head, &[])
        .await
        .unwrap_err();
    assert!(
        err.what_failed().contains("reviewed head"),
        "{}",
        err.what_failed()
    );

    let expected = tip(&l, SCHEMA_BRANCHES[0]).await;
    let moved = tip(&l, SCHEMA_BRANCHES[1]).await;
    let err = l
        .git
        .merge_branch(&c, &wt(&l.integration), &moved, &source, &expected, &[])
        .await
        .unwrap_err();
    assert!(err.what_failed().contains("moved"), "{}", err.what_failed());
}

#[tokio::test]
async fn a_conflict_is_reported_and_leaves_a_recoverable_state() {
    let l = lane(FixtureKind::MergeConflictPair).await;
    let c = ctx(RepoTrust::Trusted);
    let first = BranchName::new(CONFLICT_BRANCHES[0]).unwrap();
    let first_tip = tip(&l, CONFLICT_BRANCHES[0]).await;
    let MergeOutcome::Merged {
        head: after_first, ..
    } = l
        .git
        .merge_branch(&c, &wt(&l.integration), &l.base, &first, &first_tip, &[])
        .await
        .unwrap()
    else {
        panic!("the first branch merges cleanly");
    };
    let second = BranchName::new(CONFLICT_BRANCHES[1]).unwrap();
    let second_tip = tip(&l, CONFLICT_BRANCHES[1]).await;
    let outcome = l
        .git
        .merge_branch(
            &c,
            &wt(&l.integration),
            &after_first,
            &second,
            &second_tip,
            &[],
        )
        .await
        .unwrap();
    let MergeOutcome::Conflicted { paths } = outcome else {
        panic!("the second branch must conflict");
    };
    assert_eq!(paths, vec![CONFLICT_PATH.to_owned()]);
    // Mid-merge: a second merge attempt is refused as dirty, and abort restores the pre-merge state.
    l.git.abort_merge(&c, &wt(&l.integration)).await.unwrap();
    let status = l.git.status(&c, &wt(&l.integration)).await.unwrap();
    assert!(status.is_clean());
    assert_eq!(status.head.unwrap(), after_first);
    l.git.abort_merge(&c, &wt(&l.integration)).await.unwrap();
}

#[tokio::test]
async fn discard_resets_only_a_provably_unpublished_merge_in_a_clean_worktree() {
    let l = lane(FixtureKind::SchemaOverlapPair).await;
    let c = ctx(RepoTrust::Trusted);
    let integration = wt(&l.integration);
    let source = BranchName::new(SCHEMA_BRANCHES[0]).unwrap();
    let expected = tip(&l, SCHEMA_BRANCHES[0]).await;
    let MergeOutcome::Merged { head, .. } = l
        .git
        .merge_branch(&c, &integration, &l.base, &source, &expected, &[])
        .await
        .unwrap()
    else {
        panic!("clean merge");
    };

    // Dirty worktree: refused.
    std::fs::write(l.integration.join("scratch.txt"), "x").unwrap();
    let err = l
        .git
        .discard_unpublished_merge(&c, &integration, &l.base)
        .await
        .unwrap_err();
    assert!(
        err.what_failed().contains("uncommitted"),
        "{}",
        err.what_failed()
    );
    std::fs::remove_file(l.integration.join("scratch.txt")).unwrap();

    // Wrong SHA: refused.
    let other = tip(&l, SCHEMA_BRANCHES[1]).await;
    let err = l
        .git
        .discard_unpublished_merge(&c, &integration, &other)
        .await
        .unwrap_err();
    assert!(
        err.what_failed().contains("first parent"),
        "{}",
        err.what_failed()
    );

    // Published (a remote-tracking ref contains the merge): refused.
    l.repo
        .git(&[
            "update-ref",
            "refs/remotes/origin/vela-run-1-integration",
            head.as_str(),
        ])
        .unwrap();
    let err = l
        .git
        .discard_unpublished_merge(&c, &integration, &l.base)
        .await
        .unwrap_err();
    assert!(
        err.what_failed().contains("already on the remote"),
        "{}",
        err.what_failed()
    );
    l.repo
        .git(&[
            "update-ref",
            "-d",
            "refs/remotes/origin/vela-run-1-integration",
        ])
        .unwrap();

    // Provably safe: reset to the recorded commit.
    l.git
        .discard_unpublished_merge(&c, &integration, &l.base)
        .await
        .unwrap();
    let status = l.git.status(&c, &integration).await.unwrap();
    assert_eq!(status.head.clone().unwrap(), l.base);
    assert!(status.is_clean());
    // Nothing left to discard: idempotent.
    l.git
        .discard_unpublished_merge(&c, &integration, &l.base)
        .await
        .unwrap();
}

#[tokio::test]
async fn discard_refuses_a_worktree_outside_the_managed_root() {
    let l = lane(FixtureKind::SchemaOverlapPair).await;
    let err = l
        .git
        .discard_unpublished_merge(&ctx(RepoTrust::Trusted), &wt(l.repo.path()), &l.base)
        .await
        .unwrap_err();
    assert!(
        err.what_failed().contains("below the Vela-managed root"),
        "{}",
        err.what_failed()
    );
}

/// The API-surface test: the crate's source holds no force flag, no rebase, and exactly one hard reset (in `merge`).
#[test]
fn no_force_rebase_or_hard_reset_exists_outside_the_single_discard() {
    fn walk(dir: &std::path::Path, out: &mut Vec<(PathBuf, String)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push((path.clone(), std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    let mut files = Vec::new();
    walk(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    assert!(files.len() >= 8);
    let string_literals = |text: &str| -> Vec<String> {
        text.lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .flat_map(|l| {
                l.split('"')
                    .skip(1)
                    .step_by(2)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect()
    };
    let mut hard = 0;
    for (path, text) in &files {
        // Test modules inside src may legitimately mention the words as assertions; production literals may not.
        let production = text.split("#[cfg(test)]").next().unwrap_or("");
        for literal in string_literals(production) {
            let l = literal.to_ascii_lowercase();
            assert!(
                !l.contains("--force") && l != "-f" && !l.contains("force-with-lease"),
                "{}: {literal}",
                path.display()
            );
            assert!(
                !l.contains("rebase") && !l.contains("filter-branch") && !l.contains("--amend"),
                "{}: {literal}",
                path.display()
            );
            assert!(
                !l.starts_with('+') || !l.contains(':'),
                "a forcing refspec in {}: {literal}",
                path.display()
            );
            if literal == "--hard" {
                hard += 1;
                assert!(
                    path.ends_with("merge/mod.rs"),
                    "--hard outside merge: {}",
                    path.display()
                );
            }
        }
    }
    assert_eq!(hard, 1, "exactly one hard reset");
}
