//! A16: one real ticket, unattended, through the real Antigravity CLI: implement turn in an isolated profile, checkpoint, merge,
//! finalize, in a throwaway repository. Everything is real: Git, processes, the CLI, the hook.
//!
//! Gated by `VELA_REAL_ANTIGRAVITY=1`. It spends a little of the user's real Antigravity quota and takes minutes, so it is never
//! part of an ordinary run, and it holds the `RK-AGY-REAL-ACCOUNT` resource key: run it alone, with `--test-threads=1`. The hook
//! binary must have been built (`cargo build -p vela-hook`); it is found next to the test executable's directory.

mod real_support;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use vela_adapters::antigravity::profile::ProfileManager;
use vela_adapters::antigravity::session::AgyDriver;
use vela_core::analysis::Analysis;
use vela_core::orchestrator::driver::DriveOutcome;
use vela_domain::ids::{WorkerId, WorktreePath};
use vela_e2e_tests::{
    Fixture, RealAccountLock, config, driver, git, git_adapter, import_and_trust, issue,
    real_enabled, record_result, runner, services,
};
use vela_process::runner::baseline_env;

use real_support::{Prompts, RealAgent, hook_exe};

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
    let profiles = Arc::new(ProfileManager::new(fx.data.join("profiles"), hook));
    let agent = Arc::new(RealAgent {
        services: services.clone(),
        agy: agy.clone(),
        profiles,
        ready: Mutex::new(std::collections::HashSet::new()),
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
