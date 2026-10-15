use super::*;
use vela_domain::ids::{BranchName, OperationId};
use vela_domain::ports::{RepoTrust, WorktreeRequest};
use vela_testkit::fakes::FakeGit;

fn ctx() -> GitContext {
    GitContext {
        trust: RepoTrust::Trusted,
        operation: OperationId::new("op-1").unwrap(),
    }
}

fn sha(c: char) -> CommitSha {
    CommitSha::new(c.to_string().repeat(40)).unwrap()
}

#[test]
fn branch_names_and_trailers_are_parsed() {
    assert_eq!(parse_branch("vela/run-1/t-2"), Some(("run-1", "t-2")));
    assert_eq!(
        parse_branch("vela/run-1/integration"),
        Some(("run-1", "integration"))
    );
    assert_eq!(parse_branch("feature/x"), None);
    assert_eq!(parse_branch("vela/run-1"), None);
    let msg = "checkpoint: t-2\n\nVela-Run: run-1\nVela-Ticket: t-2\nVela-Operation: op-9\n";
    assert_eq!(trailer(msg, "vela-run"), Some("run-1"));
    assert_eq!(trailer(msg, "Vela-Ticket"), Some("t-2"));
    assert_eq!(trailer(msg, "Missing"), None);
    // A subject that looks like a trailer is not one.
    assert_eq!(trailer("Vela-Run: fake", "Vela-Run"), None);
}

#[tokio::test]
async fn the_inventory_is_rebuilt_from_branches_worktrees_and_trailers() {
    let git = FakeGit::new();
    git.set_branch_with_message(
        "vela/run-1/t-1",
        &sha('a'),
        "checkpoint: t-1\n\nVela-Run: run-1\nVela-Ticket: t-1\n",
    );
    git.set_branch_with_message("vela/run-1/t-2", &sha('b'), "a commit by someone else\n");
    git.set_branch_with_message("vela/run-1/integration", &sha('c'), "merge\n");
    git.set_branch_with_message("vela/run-2/t-9", &sha('d'), "x\n");
    git.set_branch_with_message("feature/unrelated", &sha('e'), "x\n");
    let repo = WorktreePath::new("C:/repo").unwrap();
    git.add_worktree(
        &ctx(),
        &WorktreeRequest {
            repo: repo.clone(),
            path: WorktreePath::new("C:/wt/t-1").unwrap(),
            branch: BranchName::new("vela/run-1/t-1").unwrap(),
            base: sha('a'),
        },
    )
    .await
    .unwrap();
    let inv = rebuild(&git, &ctx(), &repo).await.unwrap();
    assert_eq!(inv.runs.len(), 2);
    let r1 = &inv.runs[0];
    assert_eq!(r1.run_id, "run-1");
    assert_eq!(r1.integration, Some(sha('c')));
    assert_eq!(r1.tickets.len(), 2);
    let t1 = r1.tickets.iter().find(|t| t.ticket == "t-1").unwrap();
    assert!(t1.trailers_confirm);
    assert_eq!(t1.worktree.as_deref(), Some("C:/wt/t-1"));
    let t2 = r1.tickets.iter().find(|t| t.ticket == "t-2").unwrap();
    assert!(
        !t2.trailers_confirm,
        "a branch alone is not proof Vela made it"
    );
    assert!(t2.worktree.is_none());
}

#[tokio::test]
async fn the_inventory_is_read_only_needs_human_and_never_resumes() {
    let git = FakeGit::new();
    git.set_branch_with_message("vela/run-1/t-1", &sha('a'), "x\n");
    let repo = WorktreePath::new("C:/repo").unwrap();
    let mut inv = rebuild(&git, &ctx(), &repo).await.unwrap();
    assert!(!inv.auto_resumes());
    assert!(
        inv.runs
            .iter()
            .all(|r| r.state == Reconstructed::NeedsHuman)
    );
    assert!(
        git.log()
            .calls()
            .iter()
            .all(|c| !c.starts_with("remove") && !c.starts_with("add"))
    );
    assert_eq!(inv.abandon("run-1"), Some("STATE_STORE_UNRECOVERABLE"));
    assert_eq!(inv.runs[0].state, Reconstructed::Failed);
    assert_eq!(inv.abandon("nope"), None);
    // Abandoning leaves the branches where they are.
    assert_eq!(
        git.list_branches(&ctx(), &repo, "vela/")
            .await
            .unwrap()
            .len(),
        1
    );
}
