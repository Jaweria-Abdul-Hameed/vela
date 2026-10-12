//! A08 with the real Git adapter: installing skills happens on a separate Vela branch in a managed worktree and leaves the
//! user's checkout, branch and files exactly as they were.

use std::sync::Arc;

use vela_adapters::antigravity::skills::{Approval, bootstrap, detect};
use vela_domain::ids::{BranchName, OperationId, WorktreePath};
use vela_domain::ports::{GitContext, GitRead, RepoTrust, RevisionSpec};
use vela_git::read::GitCli;
use vela_process::redact::PatternRedactor;
use vela_process::runner::{TokioProcessRunner, baseline_env};
use vela_process::tree::TreeRegistry;
use vela_testkit::fixture_repo::{FixtureKind, FixtureRepo, ScratchDir};

fn slash(p: &std::path::Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

#[tokio::test]
async fn skills_are_installed_on_a_separate_branch_and_the_users_checkout_is_untouched() {
    let repo = FixtureRepo::create(FixtureKind::MultiPackage).unwrap();
    let scratch = ScratchDir::create("skills").unwrap();
    // The user's own instruction file: it must survive untouched.
    std::fs::write(repo.path().join("AGENTS.md"), "my own rules\n").unwrap();
    repo.git(&["add", "-A"]).unwrap();
    repo.git(&["commit", "-m", "add agents file"]).unwrap();

    // A "global" skills folder holding the skill to install.
    let home = scratch.path().join("home");
    let skill_dir = home.join(".agents").join("skills").join("implement");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: implement\ndescription: d\n---\n\nImplement the ticket.\n",
    )
    .unwrap();
    let report = detect(repo.path(), &home, None);
    assert!(report.ready() && report.instruction_files == vec!["AGENTS.md".to_owned()]);

    let managed = scratch.path().join("managed");
    std::fs::create_dir_all(&managed).unwrap();
    let runner = TokioProcessRunner::new(
        Arc::new(PatternRedactor::new()),
        Arc::new(TreeRegistry::new()),
    );
    let git = GitCli::new(Arc::new(runner), baseline_env()).with_managed_root(managed.clone());
    let ctx = GitContext {
        trust: RepoTrust::Trusted,
        operation: OperationId::new("op-skills").unwrap(),
    };
    let repo_path = WorktreePath::new(slash(repo.path())).unwrap();
    let base = git
        .resolve_ref(&ctx, &repo_path, &RevisionSpec::new("HEAD").unwrap())
        .await
        .unwrap();
    let branch_before = repo.git(&["rev-parse", "--abbrev-ref", "HEAD"]).unwrap();
    let agents_before = std::fs::read_to_string(repo.path().join("AGENTS.md")).unwrap();

    let outcome = bootstrap(
        &git,
        &ctx,
        &Approval::granted_by_user(),
        &repo_path,
        &WorktreePath::new(slash(&managed.join("skills"))).unwrap(),
        &BranchName::new("vela/bootstrap-skills").unwrap(),
        &base,
        &report.found,
        report.instruction_files.clone(),
    )
    .await
    .unwrap();

    assert_eq!(outcome.installed, vec!["implement".to_owned()]);
    // The skill is committed on the new branch, and only there.
    let on_branch = repo
        .git(&[
            "show",
            &format!("{}:.agents/skills/implement/SKILL.md", outcome.commit),
        ])
        .unwrap();
    assert!(on_branch.contains("Implement the ticket."));
    assert_ne!(outcome.commit, base);
    // The user's checkout is exactly as it was: same branch, same head, same files, clean tree.
    assert_eq!(
        repo.git(&["rev-parse", "--abbrev-ref", "HEAD"]).unwrap(),
        branch_before
    );
    assert_eq!(repo.head().unwrap(), base.as_str());
    assert!(!repo.path().join(".agents").exists());
    assert_eq!(
        std::fs::read_to_string(repo.path().join("AGENTS.md")).unwrap(),
        agents_before
    );
    assert_eq!(repo.git(&["status", "--porcelain"]).unwrap().trim(), "");
    assert!(outcome.skipped_existing.is_empty());
}
