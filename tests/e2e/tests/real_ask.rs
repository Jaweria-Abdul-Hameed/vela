//! The approval path on the real Antigravity: a ticket needs a command that is not in the confirmed profile, the hook asks, the person
//! (the test) allows it once, the worker resumes and finishes. Gated by `VELA_REAL_ANTIGRAVITY=1` like `real_ticket.rs` (same lock, same
//! rules: run it alone with `--test-threads=1`, after `cargo build -p vela-hook`).

mod real_support;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use vela_adapters::antigravity::profile::ProfileManager;
use vela_adapters::antigravity::session::AgyDriver;
use vela_core::analysis::Analysis;
use vela_core::intervention::Interventions;
use vela_core::orchestrator::driver::DriveOutcome;
use vela_core::worker::{ApprovedNotes, AskedCommands, AskedCommandsService};
use vela_e2e_tests::{
    Fixture, RealAccountLock, config, driver, git, git_adapter, import_and_trust, issue,
    real_enabled, record_result, runner, services,
};
use vela_process::runner::baseline_env;

use real_support::{Prompts, RealAgent, RealAsked, hook_exe};

#[tokio::test]
async fn a_command_outside_the_profile_is_asked_about_and_allow_once_lets_the_real_worker_finish() {
    if !real_enabled() {
        eprintln!("skipped: set VELA_REAL_ANTIGRAVITY=1 to run the real-account suite");
        return;
    }
    let _lock = RealAccountLock::acquire();
    let started = std::time::Instant::now();
    let hook = hook_exe();
    assert!(
        hook.is_file(),
        "build the hook first: cargo build -p vela-hook"
    );

    let fx = Fixture::new(
        "real-ask",
        &[(
            "T-1",
            &issue(
                "T-1",
                "Record the Git version",
                &[],
                "Run the command `git --version` and write exactly its output into a file named GITV.txt in the repository root.",
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
        profiles: profiles.clone(),
        ready: Mutex::new(std::collections::HashSet::new()),
    });
    let asked = Arc::new(RealAsked {
        services: services.clone(),
        profiles,
        agy: agy.clone(),
        readers: Mutex::new(std::collections::HashMap::new()),
    });
    services.register(Arc::new(AskedCommandsService(asked.clone())));
    let notes = Arc::new(ApprovedNotes::new());
    services.register(notes.clone());
    let driver = driver(&services, &git_adapter, agent, &fx, Arc::new(Prompts));
    let cfg = config(&fx, &project);

    let (run, analysis) = driver.prepare(&cfg).await.expect("the issue is analyzed");
    assert!(matches!(analysis, Analysis::Ready { .. }));
    driver
        .approve_and_start(&run, &cfg, None, false)
        .await
        .expect("the build starts");

    // First pass: the hook asks about `git`, the agent cannot go on, and the person is asked.
    let first = tokio::time::timeout(Duration::from_secs(900), driver.drive(&run, &cfg))
        .await
        .expect("finishes in time")
        .expect("driven");
    assert!(
        matches!(first, DriveOutcome::Waiting(_)),
        "expected a wait for a decision, got {first:?}"
    );
    let ivs = Interventions::new(&services);
    let open = ivs.open_for_run(&run).unwrap();
    assert_eq!(open.len(), 1, "{open:?}");
    assert_eq!(open[0].kind, "APPROVAL_ASK", "{:?}", open[0]);
    let evidence: Vec<String> = serde_json::from_str(&open[0].evidence).unwrap();
    assert!(
        evidence.iter().any(|e| e == "git --version" || e == "git"),
        "the exact command the agent tried: {evidence:?}
{}",
        real_support::diagnose(&services, &agy)
    );

    // The person allows it once: the exact rule goes into that worker's profile and the resumed session is told.
    let worker = vela_domain::ids::WorkerId::new(open[0].worker_id.clone().unwrap()).unwrap();
    for what in &evidence {
        asked.grant_once(&worker, what, services.now_ms()).unwrap();
        notes.add(&worker, what);
    }
    ivs.resolve(&open[0].id, "allow_once").await.unwrap();

    let second = tokio::time::timeout(Duration::from_secs(900), driver.drive(&run, &cfg))
        .await
        .expect("finishes in time")
        .expect("driven");
    assert!(
        matches!(second, DriveOutcome::Completed(_)),
        "after allow-once the run completes: {second:?}
{}",
        real_support::diagnose(&services, &agy)
            + &real_support::spool_dump(&fx.data.join("profiles"))
    );
    let integration = format!("vela/{}/integration", run.as_str());
    let out = git(&fx.repo, &["show", &format!("{integration}:GITV.txt")]);
    assert!(out.contains("git version"), "GITV.txt says: {out}");
    assert_eq!(git(&fx.repo, &["status", "--porcelain"]).trim(), "");
    record_result("real_ask_allow_once", true, started.elapsed().as_secs());
}
