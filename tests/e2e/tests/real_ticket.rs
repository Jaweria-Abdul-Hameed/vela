//! A16: one real ticket, unattended, through the real Antigravity CLI: implement turn in an isolated profile, checkpoint, merge,
//! finalize, in a throwaway repository. Everything is real: Git, processes, the CLI, the hook.
//!
//! Gated by `VELA_REAL_ANTIGRAVITY=1`. It spends a little of the user's real Antigravity quota and takes minutes, so it is never
//! part of an ordinary run, and it holds the `RK-AGY-REAL-ACCOUNT` resource key: run it alone, with `--test-threads=1`. The hook
//! binary must have been built (`cargo build -p vela-hook`); it is found next to the test executable's directory.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use vela_adapters::antigravity::profile::ProfileManager;
use vela_adapters::antigravity::prompts::{
    CompletionStatus as AgyStatus, TicketSnapshot, WorkerEnvelope, correction_prompt,
    parse_completion, render_worker_prompt,
};
use vela_adapters::antigravity::session::AgyDriver;
use vela_core::analysis::Analysis;
use vela_core::orchestrator::Services;
use vela_core::orchestrator::driver::DriveOutcome;
use vela_core::trust::TrustService;
use vela_core::worker::{Completion, CompletionStatus, WorkerPrompts, WorkerTask};
use vela_domain::errors::VelaError;
use vela_domain::ids::{WorkerId, WorktreePath};
use vela_domain::policy::{PolicyEngine, RunMode};
use vela_domain::ports::{
    AgentEventSink, AgentSession, PortFuture, SessionRef, SessionStatus, TurnRequest, TurnResult,
};
use vela_e2e_tests::{
    Fixture, RealAccountLock, config, driver, git, git_adapter, import_and_trust, issue,
    real_enabled, record_result, runner, services,
};
use vela_process::runner::baseline_env;

/// The desktop's `ProfilingAgent`, restated for the test (the composition root lives in the Tauri crate, which tests cannot link).
struct RealAgent {
    services: Arc<Services>,
    agy: Arc<AgyDriver>,
    profiles: ProfileManager,
    ready: Mutex<HashSet<WorkerId>>,
}

impl AgentSession for RealAgent {
    fn run_turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        sink: &'a dyn AgentEventSink,
    ) -> PortFuture<'a, TurnResult> {
        Box::pin(async move {
            if !self.ready.lock().unwrap().contains(&request.worker) {
                let w = request.worker.as_str().to_owned();
                let (run_id, project_id): (String, String) = self.services.store.read(|c| {
                    c.query_row(
                        "SELECT w.run_id, r.project_id FROM workers w JOIN build_runs r ON r.id = w.run_id WHERE w.id = ?1",
                        [&w],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(|e| VelaError::internal(e.to_string()))
                })?;
                let ctx = TrustService::new(&self.services).policy_context(
                    &project_id,
                    request.worktree.as_str(),
                    RunMode::Supervised,
                )?;
                let profile =
                    self.profiles
                        .create(&run_id, &request.worker, &PolicyEngine::new(ctx))?;
                self.agy
                    .set_worker_env(request.worker.clone(), profile.env());
                self.ready.lock().unwrap().insert(request.worker.clone());
            }
            self.agy.run_turn(request, sink).await
        })
    }
    fn cancel<'a>(&'a self, session: &'a SessionRef) -> PortFuture<'a, ()> {
        self.agy.cancel(session)
    }
    fn status<'a>(&'a self, session: &'a SessionRef) -> PortFuture<'a, SessionStatus> {
        self.agy.status(session)
    }
}

/// The desktop's `AgyPrompts`, restated for the test.
struct Prompts;

impl WorkerPrompts for Prompts {
    fn worker_prompt(&self, task: &WorkerTask) -> String {
        render_worker_prompt(&WorkerEnvelope {
            run_id: task.run.as_str().to_owned(),
            ticket: TicketSnapshot {
                id: task.ticket.as_str().to_owned(),
                title: task.title.clone(),
                body: task.body.clone(),
                acceptance: Vec::new(),
            },
            base_sha: task.base.as_str().to_owned(),
            branch: task.branch.as_str().to_owned(),
            worktree: task.worktree.as_str().to_owned(),
            spec_pointers: task.spec_pointers.clone(),
            policy_summary: "commands in the project's confirmed command profile run without asking; everything else is asked about or refused".to_owned(),
            skills: Vec::new(),
        })
    }
    fn fix_prompt(&self, failures: &str) -> String {
        format!(
            "The checks failed:\n{failures}\nFix the cause, then end with the vela-completion block as before."
        )
    }
    fn correction_prompt(&self, problem: &str) -> String {
        correction_prompt(problem)
    }
    fn parse_completion(&self, response: &str, base: &str) -> Result<Completion, VelaError> {
        let r = parse_completion(response, base)?;
        Ok(Completion {
            status: match r.status {
                AgyStatus::Complete => CompletionStatus::Complete,
                AgyStatus::Blocked => CompletionStatus::Blocked,
                AgyStatus::Failed => CompletionStatus::Failed,
            },
            warnings: r.warnings,
            discovered_dependencies: r.discovered_dependencies,
        })
    }
}

fn hook_exe() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    // target/debug/deps/<test>.exe -> target/debug/vela-hook.exe
    exe.parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(if cfg!(windows) {
            "vela-hook.exe"
        } else {
            "vela-hook"
        })
}

#[tokio::test]
async fn one_real_ticket_runs_unattended_from_import_to_a_merged_completed_run() {
    if !real_enabled() {
        eprintln!("skipped: set VELA_REAL_ANTIGRAVITY=1 to run the real-account suite");
        return;
    }
    let _lock = RealAccountLock::acquire();
    let started = std::time::Instant::now();
    let hook = hook_exe();
    assert!(
        hook.is_file(),
        "build the hook first: cargo build -p vela-hook ({})",
        hook.display()
    );

    let fx = Fixture::new(
        "real-ticket",
        &[(
            "T-1",
            &issue(
                "T-1",
                "Add a hello file",
                &[],
                "Create a file named HELLO.md in the repository root whose only line is: Hello from Vela",
            ),
        )],
    );
    let services = services(&fx);
    let git_adapter = git_adapter(&fx);
    let project = import_and_trust(&services, &git_adapter, &fx).await;

    let agy = Arc::new(AgyDriver::new(runner(), baseline_env()));
    let profiles = ProfileManager::new(fx.data.join("profiles"), hook);
    let agent = Arc::new(RealAgent {
        services: services.clone(),
        agy: agy.clone(),
        profiles,
        ready: Mutex::new(HashSet::new()),
    });
    let driver = driver(&services, &git_adapter, agent, &fx, Arc::new(Prompts));
    let cfg = config(&fx, &project);

    let (run, analysis) = driver.prepare(&cfg).await.expect("the issue is analyzed");
    assert!(matches!(analysis, Analysis::Ready { .. }));
    driver
        .approve_and_start(&run, &cfg, None, false)
        .await
        .expect("the build starts");
    let outcome = tokio::time::timeout(Duration::from_secs(1200), driver.drive(&run, &cfg))
        .await
        .expect("the run finishes within twenty minutes")
        .expect("the run is driven");
    let DriveOutcome::Completed(summary) = outcome else {
        let journal: Vec<String> = services
            .store
            .read(|c| {
                let mut s = c
                    .prepare("SELECT kind, payload FROM event_journal ORDER BY seq")
                    .unwrap();
                Ok(s.query_map([], |r| {
                    Ok(format!(
                        "{}: {}",
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?
                    ))
                })
                .unwrap()
                .map(Result::unwrap)
                .collect())
            })
            .unwrap();
        let workers: Vec<String> = services
            .store
            .read(|c| {
                let mut s = c.prepare("SELECT id FROM workers").unwrap();
                Ok(s.query_map([], |r| r.get::<_, String>(0))
                    .unwrap()
                    .map(Result::unwrap)
                    .collect())
            })
            .unwrap();
        let mut agent_view = String::new();
        for w in workers {
            if let Some(o) = agy.last_outcome(&WorkerId::new(w.clone()).unwrap()) {
                agent_view.push_str(&format!(
                    "worker {w}: class {:?}\nresponse: {}\nsteps: {:?}\ndenied: {:?}\nstderr: {}\n",
                    o.class, o.response, o.steps, o.denied, o.stderr_excerpt
                ));
            }
        }
        panic!(
            "the run did not complete: {outcome:?}\n{agent_view}\n{}",
            journal.join("\n")
        );
    };
    assert_eq!(summary.tickets_done, 1);

    let integration = format!("vela/{}/integration", run.as_str());
    let hello = git(&fx.repo, &["show", &format!("{integration}:HELLO.md")]);
    assert!(hello.contains("Hello from Vela"), "HELLO.md says: {hello}");
    assert!(git(&fx.repo, &["log", "--format=%B", &integration]).contains("Vela-Ticket: T-1"));
    assert_eq!(
        git(&fx.repo, &["status", "--porcelain"]).trim(),
        "",
        "the primary checkout is untouched"
    );
    record_result("one_real_ticket", true, started.elapsed().as_secs());
    let _ = WorktreePath::new(fx.repo_path());
}
