//! The whole core over the **real** Git and process adapters, with a scripted agent that writes a file: import, trust, plan, build,
//! worker, checkpoint, merge lane, finalize. Everything but the agent is real, so this is where a mismatch between the core's
//! expectations and Git's actual behaviour shows up.

use std::sync::Arc;

use vela_core::analysis::Analysis;
use vela_core::orchestrator::driver::DriveOutcome;
use vela_core::orchestrator::runs::Runs;
use vela_domain::errors::VelaError;
use vela_domain::ports::{
    AgentEvent, AgentEventSink, AgentSession, PortFuture, SessionRef, SessionStatus, TurnRequest,
    TurnResult,
};
use vela_domain::state::RunState;
use vela_e2e_tests::{
    Fixture, PlainPrompts, config, driver, git, git_adapter, import_and_trust, issue, services,
};

/// An agent that does what the ticket asks by writing one file named after the ticket into the worktree.
struct FileAgent;

impl AgentSession for FileAgent {
    fn run_turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        sink: &'a dyn AgentEventSink,
    ) -> PortFuture<'a, TurnResult> {
        Box::pin(async move {
            let ticket = request
                .prompt
                .split_whitespace()
                .nth(1)
                .unwrap_or("x")
                .trim_end_matches(':')
                .to_ascii_lowercase();
            std::fs::write(
                std::path::Path::new(request.worktree.as_str()).join(format!("{ticket}.txt")),
                format!("done by Vela for {ticket}\n"),
            )
            .map_err(|e| VelaError::internal(e.to_string()))?;
            sink.on_event(AgentEvent::TurnFinished);
            Ok(TurnResult {
                conversation: vela_domain::ports::ConversationId::new("conv-e2e").ok(),
                finished: true,
                response: "DONE".to_owned(),
            })
        })
    }
    fn cancel<'a>(&'a self, _s: &'a SessionRef) -> PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
    fn status<'a>(&'a self, _s: &'a SessionRef) -> PortFuture<'a, SessionStatus> {
        Box::pin(async { Ok(SessionStatus::Idle) })
    }
}

#[tokio::test]
async fn two_dependent_tickets_are_built_merged_and_completed_on_a_real_repository() {
    let fx = Fixture::new(
        "flow",
        &[
            ("T-1", &issue("T-1", "first", &[], "write the first file")),
            (
                "T-2",
                &issue("T-2", "second", &["T-1"], "write the second file"),
            ),
        ],
    );
    let services = services(&fx);
    let git_adapter = git_adapter(&fx);
    let project = import_and_trust(&services, &git_adapter, &fx).await;
    let driver = driver(
        &services,
        &git_adapter,
        Arc::new(FileAgent),
        &fx,
        Arc::new(PlainPrompts),
    );
    let cfg = config(&fx, &project);

    let (run, analysis) = driver.prepare(&cfg).await.expect("the issues are analyzed");
    let Analysis::Ready { snapshot, .. } = analysis else {
        panic!("expected a valid graph")
    };
    assert_eq!((snapshot.tickets.len(), snapshot.edges.len()), (2, 1));
    driver
        .approve_and_start(&run, &cfg, None, false)
        .await
        .expect("the build starts");
    let outcome = driver.drive(&run, &cfg).await.expect("the run is driven");
    let DriveOutcome::Completed(summary) = outcome else {
        panic!("expected completion, got {outcome:?}")
    };
    assert_eq!((summary.tickets_done, summary.tickets_total), (2, 2));
    assert_eq!(
        Runs::new(&services).run_state(&run).unwrap(),
        RunState::Completed
    );

    // Both files are on the integration branch of the real repository, merged by non-fast-forward merge commits, and each
    // worker's checkpoint carries Vela's trailers.
    let integration = format!("vela/{}/integration", run.as_str());
    let tree = git(&fx.repo, &["ls-tree", "--name-only", &integration]);
    assert!(
        tree.contains("t-1.txt") && tree.contains("t-2.txt"),
        "{tree}"
    );
    let merges = git(&fx.repo, &["log", "--merges", "--format=%s", &integration]);
    assert_eq!(merges.lines().count(), 2, "{merges}");
    let log = git(&fx.repo, &["log", "--format=%B", &integration]);
    assert!(
        log.contains("Vela-Run: ")
            && log.contains("Vela-Ticket: T-1")
            && log.contains("Vela-Operation: "),
        "{log}"
    );
    // The primary checkout was never touched.
    assert_eq!(git(&fx.repo, &["status", "--porcelain"]).trim(), "");
    assert_eq!(
        git(&fx.repo, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(),
        "main"
    );
    assert!(!fx.repo.join("t-1.txt").exists());
}

#[tokio::test]
async fn an_untrusted_repository_is_refused_before_any_branch_exists() {
    let fx = Fixture::new(
        "untrusted",
        &[("T-1", &issue("T-1", "only", &[], "write a file"))],
    );
    let services = services(&fx);
    let git_adapter = git_adapter(&fx);
    let imported = vela_core::project::ProjectService::new(services.clone(), git_adapter.clone())
        .import(&fx.repo_path())
        .await
        .unwrap();
    let driver = driver(
        &services,
        &git_adapter,
        Arc::new(FileAgent),
        &fx,
        Arc::new(PlainPrompts),
    );
    let cfg = config(&fx, &imported.project.id);
    let (run, _) = driver.prepare(&cfg).await.unwrap();
    let err = driver
        .approve_and_start(&run, &cfg, None, false)
        .await
        .unwrap_err();
    assert_eq!(err.class(), vela_domain::errors::ErrorClass::PolicyBlock);
    assert!(!git(&fx.repo, &["branch", "--list", "vela/*"]).contains("vela/"));
}
