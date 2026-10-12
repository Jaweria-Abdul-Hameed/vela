use std::sync::Mutex;

use vela_domain::ports::{PortFuture, ready};
use vela_testkit::fakes::FakeGit;

use super::*;
use crate::orchestrator::test_support::env;
use crate::settings::Source;

#[derive(Default)]
struct FakePrs {
    calls: Mutex<Vec<String>>,
}

impl FakePrs {
    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl PullRequestPort for FakePrs {
    fn create_draft<'a>(&'a self, r: &'a PullRequestRequest) -> PortFuture<'a, String> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("draft:{}", r.branch));
        ready(Ok("https://github.com/acme/app/pull/1".to_owned()))
    }
    fn mark_ready<'a>(&'a self, url: &'a str) -> PortFuture<'a, ()> {
        self.calls.lock().unwrap().push(format!("ready:{url}"));
        ready(Ok(()))
    }
    fn merge<'a>(&'a self, url: &'a str) -> PortFuture<'a, ()> {
        self.calls.lock().unwrap().push(format!("merge:{url}"));
        ready(Ok(()))
    }
}

fn input(run: &RunId, has_remote: bool) -> PromotionInput {
    PromotionInput {
        run: run.clone(),
        project_id: "p1".into(),
        repo: WorktreePath::new("C:/repo").unwrap(),
        branch: BranchName::new("vela/run-1/integration").unwrap(),
        branch_head: CommitSha::new("b".repeat(40)).unwrap(),
        base: BranchName::new("main").unwrap(),
        base_head: CommitSha::new("a".repeat(40)).unwrap(),
        has_remote,
        title: "Run 1".into(),
        body: "body".into(),
    }
}

async fn setup(
    tag: &str,
    with_changes: bool,
) -> (
    crate::orchestrator::test_support::TestEnv,
    RunId,
    Arc<FakeGit>,
    Arc<FakePrs>,
    PromotionService,
) {
    let e = env(tag);
    let run = e.create_run().await;
    let git = Arc::new(FakeGit::new());
    if with_changes {
        git.set_diff(vec!["src/lib.rs".into()]);
    }
    let prs = Arc::new(FakePrs::default());
    let svc = PromotionService::new(e.services.clone(), git.clone(), Some(prs.clone()));
    (e, run, git, prs, svc)
}

#[tokio::test]
async fn the_default_is_a_draft_then_ready_and_never_a_merge() {
    let (_e, run, _g, prs, svc) = setup("pr", true).await;
    let p = svc.begin(&input(&run, true)).await.unwrap();
    assert_eq!(
        (p.mode.as_str(), p.state.as_str()),
        ("PULL_REQUEST", "DRAFT")
    );
    assert_eq!(
        p.pr_url.as_deref(),
        Some("https://github.com/acme/app/pull/1")
    );
    let done = svc.finalize(&run).await.unwrap();
    assert_eq!(done.state, "READY");
    assert_eq!(
        prs.calls(),
        [
            "draft:vela/run-1/integration",
            "ready:https://github.com/acme/app/pull/1"
        ]
    );
    // Repeating either step does nothing more.
    svc.begin(&input(&run, true)).await.unwrap();
    svc.finalize(&run).await.unwrap();
    assert_eq!(prs.calls().len(), 2);
}

#[tokio::test]
async fn no_remote_or_no_github_means_local_ready() {
    let (_e, run, _g, prs, svc) = setup("local", true).await;
    let p = svc.begin(&input(&run, false)).await.unwrap();
    assert_eq!(p.state, "LOCAL_READY");
    assert!(p.note.contains("not pushed"));
    assert!(prs.calls().is_empty());
    assert_eq!(svc.finalize(&run).await.unwrap().state, "LOCAL_READY");
}

#[tokio::test]
async fn an_unreachable_github_service_leaves_the_run_local_ready() {
    let e = env("nogh");
    let run = e.create_run().await;
    let git = Arc::new(FakeGit::new());
    git.set_diff(vec!["a".into()]);
    let svc = PromotionService::new(e.services.clone(), git, None);
    assert_eq!(
        svc.begin(&input(&run, true)).await.unwrap().state,
        "LOCAL_READY"
    );
}

#[tokio::test]
async fn local_only_mode_never_opens_a_pull_request() {
    let (e, run, _g, prs, svc) = setup("local-mode", true).await;
    SettingsService::new(&e.services)
        .set_user(Source::User, "promotion_mode", "local_only")
        .unwrap();
    assert_eq!(
        svc.begin(&input(&run, true)).await.unwrap().state,
        "LOCAL_READY"
    );
    assert!(prs.calls().is_empty());
}

#[tokio::test]
async fn an_empty_branch_has_nothing_to_promote() {
    let (_e, run, _g, prs, svc) = setup("empty", false).await;
    let p = svc.begin(&input(&run, true)).await.unwrap();
    assert_eq!(p.state, "NONE");
    assert!(p.note.contains("no changes"));
    assert!(prs.calls().is_empty());
}

#[tokio::test]
async fn auto_merge_needs_the_acknowledged_warning_and_is_the_only_path_to_a_merge() {
    let (e, run, _g, prs, svc) = setup("auto", true).await;
    SettingsService::new(&e.services)
        .set_user(Source::User, "promotion_mode", "auto_merge")
        .unwrap();
    let err = svc.begin(&input(&run, true)).await.unwrap_err();
    assert_eq!(err.class(), ErrorClass::PolicyBlock);
    assert!(prs.calls().is_empty());

    Onboarding::new(&e.services).grant("auto_merge").unwrap();
    svc.begin(&input(&run, true)).await.unwrap();
    let done = svc.finalize(&run).await.unwrap();
    assert_eq!(done.state, "MERGED");
    assert_eq!(
        prs.calls().last().unwrap(),
        "merge:https://github.com/acme/app/pull/1"
    );
}

#[tokio::test]
async fn consent_withdrawn_mid_run_stops_at_ready() {
    let (e, run, _g, prs, svc) = setup("withdrawn", true).await;
    SettingsService::new(&e.services)
        .set_user(Source::User, "promotion_mode", "auto_merge")
        .unwrap();
    Onboarding::new(&e.services).grant("auto_merge").unwrap();
    svc.begin(&input(&run, true)).await.unwrap();
    Onboarding::new(&e.services).revoke("auto_merge").unwrap();
    assert_eq!(svc.finalize(&run).await.unwrap().state, "READY");
    assert!(!prs.calls().iter().any(|c| c.starts_with("merge")));
}
