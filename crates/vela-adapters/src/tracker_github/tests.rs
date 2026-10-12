use std::sync::Arc;

use vela_domain::errors::ErrorCode;
use vela_domain::ids::{TicketId, WorktreePath};
use vela_domain::ports::IssueTrackerAdapter;
use vela_testkit::fake_gh::{FakeGh, FakeIssue};

use super::*;

fn tracker(gh: &Arc<FakeGh>) -> GhTracker {
    GhTracker::new(
        gh.clone(),
        "acme/widgets",
        WorktreePath::new("/repo").unwrap(),
        vec![("PATH".into(), "x".into())],
    )
    .unwrap()
}

fn seeded() -> Arc<FakeGh> {
    let gh = Arc::new(FakeGh::new());
    let mut a = FakeIssue::new(1, "First", "body one");
    a.labels.push("status:ready".into());
    let mut b = FakeIssue::new(2, "Second", "body two");
    b.blocked_by.push(1);
    gh.add_issue(a);
    gh.add_issue(b);
    gh
}

#[tokio::test]
async fn lists_and_reads_issues_with_native_blockers() {
    let gh = seeded();
    let t = tracker(&gh);
    assert_eq!(t.detect().await, GhStatus::Ready);
    let all = t.list_tickets().await.unwrap();
    assert_eq!(all.len(), 2);
    let second = all.iter().find(|x| x.id.as_str() == "gh-2").unwrap();
    assert_eq!(second.blockers, vec![TicketId::new("gh-1").unwrap()]);
    let one = t
        .read_ticket(&TicketId::new("gh-1").unwrap())
        .await
        .unwrap();
    assert_eq!(one.title, "First");
    assert_eq!(one.labels, vec!["status:ready".to_owned()]);
    assert!(
        t.read_ticket(&TicketId::new("gh-99").unwrap())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn status_labels_are_replaced_comments_posted_and_issues_closed() {
    let gh = seeded();
    let t = tracker(&gh);
    let id = TicketId::new("gh-1").unwrap();
    t.update_status(&id, "in-progress").await.unwrap();
    assert_eq!(
        gh.issue(1).unwrap().labels,
        vec!["status:in-progress".to_owned()]
    );
    let writes = gh.write_count();
    t.update_status(&id, "in-progress").await.unwrap();
    assert_eq!(
        gh.write_count(),
        writes,
        "an unchanged status writes nothing"
    );
    t.comment_evidence(&id, "tests passed").await.unwrap();
    assert_eq!(
        gh.issue(1).unwrap().comments,
        vec!["tests passed".to_owned()]
    );
    assert!(t.has_evidence_marker(&id, "passed").await.unwrap());
    assert!(!t.has_evidence_marker(&id, "never-posted").await.unwrap());
    t.close_ticket(&id).await.unwrap();
    assert!(gh.issue(1).unwrap().closed);
    assert_eq!(t.list_tickets().await.unwrap().len(), 1);
}

#[tokio::test]
async fn untrusted_text_only_travels_as_a_flag_value() {
    let gh = seeded();
    let t = tracker(&gh);
    let id = TicketId::new("gh-1").unwrap();
    let hostile = "--repo=evil/steal\n$(rm -rf /) `x`";
    t.comment_evidence(&id, hostile).await.unwrap();
    let call = gh
        .calls()
        .into_iter()
        .find(|c| c.get(1).map(String::as_str) == Some("comment"))
        .unwrap();
    assert_eq!(call.iter().filter(|a| a.as_str() == "--repo").count(), 1);
    assert!(call.iter().any(|a| a == &format!("--body={hostile}")));
    assert!(
        call.iter().all(|a| a != hostile),
        "the text must never be a standalone argument"
    );
    assert_eq!(gh.issue(1).unwrap().comments, vec![hostile.to_owned()]);
}

#[tokio::test]
async fn ids_and_repo_names_are_validated() {
    let gh = seeded();
    let t = tracker(&gh);
    for bad in ["1", "gh-", "gh-1x", "gh--1", "gh-1 2", "GH-1"] {
        if let Ok(id) = TicketId::new(bad) {
            assert!(t.read_ticket(&id).await.is_err(), "{bad}");
        }
    }
    for bad in [
        "", "owner", "a/b/c", "-x/y", "x/-y", "x/.y", "x y/z", "x/y;rm",
    ] {
        assert!(
            GhTracker::new(gh.clone(), bad, WorktreePath::new("/r").unwrap(), vec![]).is_err(),
            "{bad}"
        );
    }
}

#[tokio::test]
async fn missing_or_unauthenticated_gh_degrades_to_local_only_with_auth_required() {
    let gh = seeded();
    gh.set_installed(false);
    let t = tracker(&gh);
    assert_eq!(t.detect().await, GhStatus::Missing);
    assert_eq!(
        GhStatus::Missing.degraded_error().unwrap().code(),
        Some(ErrorCode::AuthRequired)
    );

    let gh = seeded();
    gh.set_authenticated(false);
    let t = tracker(&gh);
    assert_eq!(t.detect().await, GhStatus::Unauthenticated);
    let err = t.list_tickets().await.unwrap_err();
    assert_eq!(err.code(), Some(ErrorCode::AuthRequired));
    assert!(GhStatus::Ready.degraded_error().is_none());
}

#[tokio::test]
async fn an_outage_is_a_temporary_external_error() {
    let gh = seeded();
    gh.set_outage(true);
    let t = tracker(&gh);
    let err = t.list_tickets().await.unwrap_err();
    assert_eq!(err.class(), ErrorClass::ExternalTemporary);
}

#[tokio::test]
async fn no_token_is_ever_passed() {
    let gh = seeded();
    let t = tracker(&gh);
    t.list_tickets().await.unwrap();
    for call in gh.calls() {
        assert!(
            call.iter()
                .all(|a| !a.to_ascii_lowercase().contains("token")),
            "{call:?}"
        );
    }
}

mod pull_requests {
    use std::sync::Arc;

    use vela_domain::ids::{BranchName, WorktreePath};
    use vela_domain::ports::{OutputChunk, OutputStream, PullRequestPort, PullRequestRequest};
    use vela_testkit::fakes::{FakeProcessRunner, Scripted};

    use super::super::GhTracker;

    fn setup() -> (Arc<FakeProcessRunner>, GhTracker) {
        let runner = Arc::new(FakeProcessRunner::new());
        let t = GhTracker::new(
            runner.clone(),
            "acme/widgets",
            WorktreePath::new("/repo").unwrap(),
            vec![],
        )
        .unwrap();
        (runner, t)
    }

    #[tokio::test]
    async fn a_draft_is_created_with_untrusted_text_only_as_flag_values() {
        let (runner, t) = setup();
        runner.push(Scripted::exit(0).with_output(OutputChunk {
            stream: OutputStream::Stdout,
            text: "Creating draft...\nhttps://github.com/acme/widgets/pull/7\n".into(),
        }));
        let url = t
            .create_draft(&PullRequestRequest {
                branch: BranchName::new("vela/run-1/integration").unwrap(),
                base: BranchName::new("main").unwrap(),
                title: "--web evil".into(),
                body: "body".into(),
            })
            .await
            .unwrap();
        assert_eq!(url, "https://github.com/acme/widgets/pull/7");
        let call = runner.log().calls().remove(0);
        assert!(call.contains("pr create --draft --repo acme/widgets --head=vela/run-1/integration --base=main --title=--web evil"), "{call}");
    }

    #[tokio::test]
    async fn bad_output_and_bad_addresses_are_refused() {
        let (runner, t) = setup();
        runner.push(Scripted::exit(0));
        let req = PullRequestRequest {
            branch: BranchName::new("a").unwrap(),
            base: BranchName::new("main").unwrap(),
            title: "t".into(),
            body: "b".into(),
        };
        assert!(t.create_draft(&req).await.is_err());
        assert!(t.mark_ready("--repo=x").await.is_err());
        assert!(t.merge("https://evil.example/x").await.is_err());
        assert!(runner.log().calls().len() == 1);
        t.mark_ready("https://github.com/acme/widgets/pull/7")
            .await
            .unwrap();
        t.merge("https://github.com/acme/widgets/pull/7")
            .await
            .unwrap();
        let calls = runner.log().calls();
        assert!(calls[1].contains("pr ready https://github.com/acme/widgets/pull/7"));
        assert!(calls[2].contains("pr merge") && calls[2].contains("--merge"));
    }
}
