//! R03: the crash-injection recovery matrix. Every declared fault point is armed in a child process running a real scenario over
//! the real Git and process adapters; the child is killed (it aborts, with no unwinding) exactly at the point, then restarted
//! against the state it left. The restart reconciles, resumes what recovery allows, and verifies:
//!
//! * the state store opens and passes its integrity check, and the journal is contiguous;
//! * Git is consistent: no half merge or lock file, every worker branch preserved, no ticket merged twice;
//! * the run is in a state a person or the driver can act on, never one that silently stalls or lost work.
//!
//! A standing completeness test fails when any declared [`FaultPoint`] has no matrix entry, so a point added by a later ticket
//! cannot be forgotten. The matrix owns every entry; tickets that declare points never write here.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use vela_core::analysis::Analysis;
use vela_core::orchestrator::Services;
use vela_core::orchestrator::driver::{DriveOutcome, RunDriver};
use vela_core::orchestrator::runs::Runs;
use vela_core::recovery::{Recovery, RecoveryInput};
use vela_core::stop::StopService;
use vela_core::trust::ProfileCommand;
use vela_domain::fault::{ARMED_ENV_VAR, FaultPoint};
use vela_domain::ids::{RunId, WorktreePath};
use vela_domain::state::RunState;
use vela_e2e_tests::{
    FileAgent, Fixture, PlainPrompts, config, driver, git, git_adapter, import_and_trust, issue,
    runner, services,
};
use vela_testkit::fault_harness::{
    KillRestartHarness, Phase, about_to_reach, child_context, report,
};

/// Which scenario a fault point lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scenario {
    /// Two dependent tickets, built, merged, finalized (and pushed to a bare remote).
    Flow,
    /// One ticket whose integration gate fails after the merge, so the merge is discarded.
    Discard,
    /// The approval broker deciding and delivering one allowed request.
    Approval,
    /// Stop All while the merge lane is still busy when the bounded wait ends.
    StopBusy,
    /// Stop All with an unfinished merge on record.
    StopPending,
}

impl Scenario {
    const fn name(self) -> &'static str {
        match self {
            Scenario::Flow => "flow",
            Scenario::Discard => "discard",
            Scenario::Approval => "approval",
            Scenario::StopBusy => "stop_busy",
            Scenario::StopPending => "stop_pending",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        [
            Scenario::Flow,
            Scenario::Discard,
            Scenario::Approval,
            Scenario::StopBusy,
            Scenario::StopPending,
        ]
        .into_iter()
        .find(|x| x.name() == s)
    }
}

/// Every declared fault point and the scenario that reaches it. `SelfTest` is the harness's own point and is not a product
/// transition.
const MATRIX: &[(FaultPoint, Scenario)] = &[
    (FaultPoint::PersistenceBeforeCommit, Scenario::Flow),
    (FaultPoint::PersistenceAfterCommit, Scenario::Flow),
    (FaultPoint::GitCheckpointBeforeCommit, Scenario::Flow),
    (FaultPoint::GitCheckpointAfterCommit, Scenario::Flow),
    (FaultPoint::GitPushBefore, Scenario::Flow),
    (FaultPoint::GitPushAfter, Scenario::Flow),
    (FaultPoint::BuildBeforeBranch, Scenario::Flow),
    (FaultPoint::BuildAfterWorktree, Scenario::Flow),
    (FaultPoint::BuildBeforeRunning, Scenario::Flow),
    (FaultPoint::WorkerAfterProvisioning, Scenario::Flow),
    (FaultPoint::WorkerAfterImplementing, Scenario::Flow),
    (FaultPoint::WorkerBeforeCheckpoint, Scenario::Flow),
    (FaultPoint::WorkerAfterCheckpoint, Scenario::Flow),
    (FaultPoint::LaneBeforeMerge, Scenario::Flow),
    (FaultPoint::LaneAfterMerge, Scenario::Flow),
    (FaultPoint::LaneAfterValidation, Scenario::Flow),
    (FaultPoint::LaneAfterDiscard, Scenario::Discard),
    (FaultPoint::FinalBeforePush, Scenario::Flow),
    (FaultPoint::FinalAfterPush, Scenario::Flow),
    (FaultPoint::FinalAfterPromotion, Scenario::Flow),
    (FaultPoint::FinalDuringCleanup, Scenario::Flow),
    (FaultPoint::ApprovalDecisionPersisted, Scenario::Approval),
    (FaultPoint::ApprovalDeliveryAttempted, Scenario::Approval),
    (FaultPoint::StopBoundedWaitExpired, Scenario::StopBusy),
    (FaultPoint::StopDiscardDecision, Scenario::StopPending),
];

const GATE: &str = "git diff --exit-code main";

fn issues_for(scenario: Scenario) -> Vec<(&'static str, String)> {
    match scenario {
        Scenario::Discard | Scenario::StopBusy | Scenario::StopPending | Scenario::Approval => {
            vec![("T-1", issue("T-1", "only", &[], "write the file"))]
        }
        Scenario::Flow => vec![
            ("T-1", issue("T-1", "first", &[], "write the first file")),
            (
                "T-2",
                issue("T-2", "second", &["T-1"], "write the second file"),
            ),
        ],
    }
}

fn fixture(dir: &Path, scenario: Scenario) -> Fixture {
    let owned = issues_for(scenario);
    let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (*a, b.as_str())).collect();
    Fixture::at(dir, &refs)
}

/// Gives the fixture repository a bare remote called `origin` (idempotent), so the final push and its fault points are reached.
fn with_remote(fx: &Fixture) {
    let remote = fx.base.join("remote.git");
    if remote.exists() {
        return;
    }
    std::fs::create_dir_all(&remote).unwrap();
    git(&remote, &["init", "-q", "--bare", "-b", "main"]);
    git(
        &fx.repo,
        &["remote", "add", "origin", &remote.to_string_lossy()],
    );
    git(&fx.repo, &["push", "-q", "origin", "main"]);
}

fn run_config(
    fx: &Fixture,
    project: &str,
    scenario: Scenario,
) -> vela_core::orchestrator::driver::RunConfig {
    let mut cfg = config(fx, project);
    if scenario == Scenario::Flow {
        cfg.remote = Some(vela_domain::ports::RemoteName::new("origin").unwrap());
    }
    if scenario == Scenario::Discard {
        cfg.validation = vec![ProfileCommand {
            text: GATE.into(),
            cwd: None,
        }];
    }
    cfg
}

// ------------------------------------------------------------------------------------------------------------------ child

async fn drive_flow(dir: &Path, scenario: Scenario) {
    let fx = fixture(dir, scenario);
    if scenario == Scenario::Flow {
        with_remote(&fx);
    }
    let services = services(&fx);
    let git_adapter = git_adapter(&fx);
    register_recipes(&services, &git_adapter);
    let project = import_and_trust_with(&services, &git_adapter, &fx, scenario).await;
    let driver = driver(
        &services,
        &git_adapter,
        Arc::new(FileAgent),
        &fx,
        Arc::new(PlainPrompts),
    );
    let cfg = run_config(&fx, &project, scenario);
    let (run, analysis) = driver.prepare(&cfg).await.expect("prepare");
    assert!(matches!(analysis, Analysis::Ready { .. }));
    driver
        .approve_and_start(&run, &cfg, None, false)
        .await
        .expect("start");
    match scenario {
        Scenario::StopBusy | Scenario::StopPending => {
            let integration = integration_of(&cfg, &run);
            // An unfinished merge on record, as a crash in the lane would leave it.
            if scenario == Scenario::StopPending {
                let head = head_of(&fx, &cfg, &run);
                vela_core::operations::OperationLog::new(&services)
                    .begin(
                        &vela_domain::ids::OperationId::new("op-pending-merge").unwrap(),
                        Some(&run),
                        "merge",
                        &serde_json::json!({"before": head}),
                    )
                    .expect("record the merge");
            }
            let stop = StopService::new(
                services.clone(),
                Arc::new(FileAgent),
                runner(),
                git_adapter.clone(),
                Arc::new(Busy(scenario == Scenario::StopBusy)),
                Duration::from_millis(20),
            );
            let _ = stop.stop_all(&run, &project, &integration).await;
        }
        _ => {
            let _ = driver.drive(&run, &cfg).await;
        }
    }
}

struct Busy(bool);

impl vela_core::stop::LaneProbe for Busy {
    fn busy(&self, _run: &RunId) -> bool {
        self.0
    }
}

fn integration_of(cfg: &vela_core::orchestrator::driver::RunConfig, run: &RunId) -> WorktreePath {
    WorktreePath::new(format!(
        "{}/{}/integration",
        cfg.worktree_root.trim_end_matches(['/', '\\']),
        run.as_str()
    ))
    .unwrap()
}

fn head_of(_fx: &Fixture, cfg: &vela_core::orchestrator::driver::RunConfig, run: &RunId) -> String {
    let dir = integration_of(cfg, run);
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir.as_str())
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

async fn import_and_trust_with(
    services: &Arc<Services>,
    git_adapter: &Arc<vela_git::read::GitCli>,
    fx: &Fixture,
    scenario: Scenario,
) -> String {
    if scenario != Scenario::Discard {
        return import_and_trust(services, git_adapter, fx).await;
    }
    let svc = vela_core::project::ProjectService::new(services.clone(), git_adapter.clone());
    let imported = svc.import(&fx.repo_path()).await.unwrap();
    vela_core::trust::TrustService::new(services)
        .trust(
            &imported.project.id,
            &imported.project.repo_path,
            &[ProfileCommand {
                text: GATE.into(),
                cwd: None,
            }],
            "e2e",
        )
        .unwrap();
    imported.project.id
}

async fn drive_approval(dir: &Path) {
    let fx = fixture(dir, Scenario::Approval);
    let services = services(&fx);
    let git_adapter = git_adapter(&fx);
    register_recipes(&services, &git_adapter);
    let project = import_and_trust(&services, &git_adapter, &fx).await;
    // A run with one worker, through the real services, then one allowed request through the broker.
    let driver = driver(
        &services,
        &git_adapter,
        Arc::new(FileAgent),
        &fx,
        Arc::new(PlainPrompts),
    );
    let cfg = config(&fx, &project);
    let (run, _) = driver.prepare(&cfg).await.unwrap();
    driver
        .approve_and_start(&run, &cfg, None, false)
        .await
        .unwrap();
    let runs = Runs::new(&services);
    let ticket = vela_domain::ids::TicketId::new("T-1").unwrap();
    let worker = vela_domain::ids::WorkerId::new("w-approval").unwrap();
    runs.create_worker(
        &run,
        worker.clone(),
        &ticket,
        vela_domain::ids::BranchName::new(format!("vela/{}/t-1", run.as_str())).unwrap(),
        WorktreePath::new(format!("{}/t-1", cfg.worktree_root)).unwrap(),
        vela_domain::ids::CommitSha::new("a".repeat(40)).unwrap(),
        "default",
        vela_domain::state::IntegrationHealth::Healthy,
    )
    .await
    .unwrap();
    // Trust with a profile that contains the command being approved.
    vela_core::trust::TrustService::new(&services)
        .trust(
            &project,
            &fx.repo_path(),
            &[ProfileCommand {
                text: "cargo test".into(),
                cwd: None,
            }],
            "e2e2",
        )
        .unwrap();
    let broker =
        vela_core::broker::Broker::new(services.clone(), Some(Arc::new(AllowOnce)), "native");
    let _ = broker
        .handle(&vela_core::broker::Intake {
            run,
            worker,
            ticket,
            project_id: project,
            worktree_root: cfg.worktree_root.clone(),
            mode: vela_domain::policy::RunMode::Supervised,
            observation: vela_domain::ports::ApprovalObservation {
                source_id: "hook-spool".into(),
                session_identity: Some("conv-1".into()),
                operation: "cargo test".into(),
                working_directory: Some(cfg.worktree_root.clone()),
                options: vec!["allow once".into()],
                evidence: vela_domain::ports::EvidenceTier::Bound,
            },
        })
        .await;
}

struct AllowOnce;

impl vela_domain::ports::ApprovalDeliveryAdapter for AllowOnce {
    fn detect(&self) -> vela_domain::ports::PortFuture<'_, vela_domain::ports::DeliveryCapability> {
        Box::pin(async {
            Ok(vela_domain::ports::DeliveryCapability {
                tier: vela_domain::ports::DeliveryTier::Native,
                available: true,
            })
        })
    }
    fn deliver<'a>(
        &'a self,
        _r: &'a vela_domain::ports::DeliveryRequest,
    ) -> vela_domain::ports::PortFuture<'a, vela_domain::ports::DeliveryResult> {
        Box::pin(async {
            Ok(vela_domain::ports::DeliveryResult {
                verified: true,
                detail: "delivered".into(),
            })
        })
    }
    fn cancel<'a>(
        &'a self,
        _o: &'a vela_domain::ports::ApprovalObservation,
    ) -> vela_domain::ports::PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

// ------------------------------------------------------------------------------------------------------------------ restart

/// What the composition root registers: the recipes that reconcile interrupted operations.
fn register_recipes(services: &Arc<Services>, git: &Arc<vela_git::read::GitCli>) {
    vela_core::capacity::register(services);
    vela_core::broker::register_recipes(services);
    vela_core::merge_lane::register_recipes(services);
    vela_core::operations::register_checkpoint_recipe(services, git.clone());
}

fn runs_in(services: &Arc<Services>) -> Vec<(String, String)> {
    services
        .store
        .read(|c| {
            let mut s = c
                .prepare("SELECT id, project_id FROM build_runs ORDER BY id")
                .unwrap();
            Ok(s.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .map(Result::unwrap)
                .collect())
        })
        .unwrap()
}

fn out(key: &str, value: impl std::fmt::Display) {
    report(&format!("{key}={value}"));
}

async fn verify_restart(dir: &Path, scenario: Scenario) {
    let fx = fixture(dir, scenario);
    let services = services(&fx); // opening the store runs its integrity check
    out("store", "opened");
    let issues = {
        let conn_issues = vela_persistence::integrity::check;
        services
            .store
            .read(conn_issues)
            .expect("integrity check runs")
    };
    out("integrity_issues", issues.len());
    let git_adapter = git_adapter(&fx);
    register_recipes(&services, &git_adapter);
    let runs = runs_in(&services);
    out("runs", runs.len());
    let mut outcome = "no_run".to_owned();
    for (id, project) in runs {
        let run = RunId::new(id).unwrap();
        let cfg = run_config(&fx, &project, scenario);
        let drv: Arc<RunDriver> = driver(
            &services,
            &git_adapter,
            Arc::new(FileAgent),
            &fx,
            Arc::new(PlainPrompts),
        );
        if let Ok(choice) = std::env::var("VELA_MATRIX_CONTINUATION") {
            vela_core::settings::SettingsService::new(&services)
                .set_user(
                    vela_core::settings::Source::User,
                    "recovery_continuation",
                    &choice,
                )
                .expect("the continuation setting is stored");
        }
        let started = drv.recover_start(&run, &cfg).await;
        out("build_start_completed", started.as_ref().is_ok_and(|b| *b));
        let stopper = StopService::new(
            services.clone(),
            Arc::new(FileAgent),
            runner(),
            git_adapter.clone(),
            drv.lane(),
            Duration::from_secs(5),
        );
        let finished = stopper
            .finish_interrupted(&run, &project, &integration_of(&cfg, &run))
            .await;
        out(
            "stop_completed",
            finished.as_ref().is_ok_and(|s| s.is_some()),
        );
        let recovery = Recovery::new(services.clone(), git_adapter.clone(), Arc::new(FileAgent));
        let report_r = recovery
            .reconcile(&RecoveryInput {
                run: run.clone(),
                project_id: project.clone(),
                repo: cfg.repo.clone(),
                reason: "startup".into(),
            })
            .await;
        let Ok(rr) = report_r else {
            outcome = "reconcile_failed".into();
            continue;
        };
        out("discrepancies", rr.discrepancies.len());
        let runs_svc = Runs::new(&services);
        let state = runs_svc.run_state(&run).unwrap();
        out("state_after_reconcile", state.name());
        if scenario == Scenario::Approval {
            // The approval scenario has no driver behind it: what matters is that a persisted decision is never replayed.
            let r = run.as_str().to_owned();
            let (states, deliveries): (Vec<String>, i64) = services
                .store
                .read(|c| {
                    let mut s = c
                        .prepare("SELECT state FROM approval_requests WHERE run_id = ?1")
                        .unwrap();
                    let states = s
                        .query_map([&r], |row| row.get::<_, String>(0))
                        .unwrap()
                        .map(Result::unwrap)
                        .collect();
                    let d = c
                        .query_row("SELECT COUNT(*) FROM approval_deliveries", [], |row| {
                            row.get(0)
                        })
                        .unwrap_or(0);
                    Ok((states, d))
                })
                .unwrap();
            out("approval_states", states.join(","));
            out("approval_deliveries", deliveries);
            outcome = "approval_checked".into();
            continue;
        }
        if let RunState::Paused { reason, .. } = &state {
            out("paused_reason", format!("{reason:?}"));
        }
        outcome = match state {
            RunState::Active(vela_domain::state::RunPhase::Running) => "running".into(),
            RunState::Paused { .. } => {
                let stop = StopService::new(
                    services.clone(),
                    Arc::new(FileAgent),
                    runner(),
                    git_adapter.clone(),
                    drv.lane(),
                    Duration::from_secs(5),
                );
                match stop.resume(&run, &rr).await {
                    Ok(_) => "resumed".into(),
                    Err(_) => "paused_held".into(),
                }
            }
            other => format!("{other:?}")
                .split_whitespace()
                .next()
                .unwrap_or("other")
                .to_owned(),
        };
        if outcome == "running" || outcome == "resumed" {
            outcome =
                match tokio::time::timeout(Duration::from_secs(120), drv.drive(&run, &cfg)).await {
                    Ok(Ok(DriveOutcome::Completed(_))) => "completed".into(),
                    Ok(Ok(other)) => format!("drive_{other:?}").to_lowercase(),
                    Ok(Err(e)) => format!(
                        "drive_error:{}",
                        e.what_failed()
                            .chars()
                            .take(60)
                            .collect::<String>()
                            .replace('\n', " ")
                    ),
                    Err(_) => "drive_timeout".into(),
                };
        }
    }
    out("outcome", outcome);

    // Git: no lock files or half merges; worker branches preserved; no ticket merged twice.
    let git_dir = fx.repo.join(".git");
    let locks: Vec<_> = walk(&git_dir)
        .into_iter()
        .filter(|p| p.ends_with("index.lock") || p.ends_with("HEAD.lock"))
        .collect();
    out("lock_files", locks.len());
    let worktrees = fx.data.join("worktrees");
    let half_merges = walk(&fx.repo.join(".git").join("worktrees"))
        .into_iter()
        .filter(|p| p.ends_with("MERGE_HEAD"))
        .count();
    out("half_merges", half_merges);
    let _ = worktrees;
    let branches = git_out(&fx.repo, &["branch", "--list", "vela/*"]);
    out(
        "vela_branches",
        branches.lines().filter(|l| !l.trim().is_empty()).count(),
    );
    let merges = git_out(
        &fx.repo,
        &[
            "log",
            "--all",
            "--merges",
            "--format=%s%n%b%x00",
            "--grep=Vela-Ticket",
        ],
    );
    let mut seen = std::collections::BTreeMap::<String, u32>::new();
    for chunk in merges.split('\0') {
        for line in chunk.lines() {
            if let Some(t) = line.strip_prefix("Vela-Ticket:") {
                *seen.entry(t.trim().to_owned()).or_default() += 1;
            }
        }
    }
    let dup = seen.values().filter(|n| **n > 1).count();
    out("duplicate_merges", dup);
}

fn git_out(dir: &Path, args: &[&str]) -> String {
    let o = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p);
            }
        }
    }
    out
}

/// The child: a no-op in a normal run. The harness launches it armed (phase `run`) and again unarmed (phase `restart`).
#[tokio::test(flavor = "multi_thread")]
async fn matrix_child() {
    let Some(ctx) = child_context() else { return };
    let scenario = std::env::var("VELA_MATRIX_SCENARIO")
        .ok()
        .and_then(|s| Scenario::parse(&s))
        .expect("scenario");
    match ctx.phase {
        Phase::Run => {
            if let Some(p) = std::env::var(ARMED_ENV_VAR)
                .ok()
                .and_then(|n| FaultPoint::from_name(&n))
            {
                about_to_reach(p);
            }
            match scenario {
                Scenario::Approval => drive_approval(&ctx.state_dir).await,
                _ => drive_flow(&ctx.state_dir, scenario).await,
            }
        }
        Phase::Restart => verify_restart(&ctx.state_dir, scenario).await,
    }
}

// ------------------------------------------------------------------------------------------------------------------ parent

/// Every declared fault point has exactly one matrix entry; a point added later without one fails this test.
#[test]
fn every_declared_fault_point_has_a_matrix_entry() {
    let covered: Vec<FaultPoint> = MATRIX.iter().map(|(p, _)| *p).collect();
    for p in FaultPoint::ALL {
        if *p == FaultPoint::SelfTest {
            continue;
        }
        assert!(
            covered.contains(p),
            "fault point {} has no entry in the crash matrix",
            p.name()
        );
    }
    for (i, p) in covered.iter().enumerate() {
        assert!(!covered[..i].contains(p), "{} appears twice", p.name());
    }
    assert_eq!(covered.len(), FaultPoint::ALL.len() - 1);
}

fn kill_and_verify(point: FaultPoint, scenario: Scenario) -> Vec<String> {
    let h = KillRestartHarness::new("matrix_child")
        .expect("harness")
        .with_timeout(Duration::from_secs(240))
        .with_env("VELA_MATRIX_SCENARIO", scenario.name());
    let r = h
        .kill_and_restart(point)
        .unwrap_or_else(|e| panic!("{}: {e}", point.name()));
    assert!(
        r.killed.crashed(),
        "{}: the child did not crash",
        point.name()
    );
    assert!(
        r.restart.success,
        "{}: restart failed:\n{}\n{}",
        point.name(),
        r.restart.stdout,
        r.restart.stderr
    );
    r.restart.reports.clone()
}

fn value<'a>(reports: &'a [String], key: &str) -> Option<&'a str> {
    let prefix = format!("{key}=");
    reports
        .iter()
        .rev()
        .find_map(|l| l.strip_prefix(prefix.as_str()))
}

/// The whole matrix. Each entry is killed and restarted in its own pair of processes.
#[test]
fn the_process_killed_at_every_declared_point_recovers_to_a_consistent_state() {
    let mut failures = Vec::new();
    let mut table = Vec::new();
    for (point, scenario) in MATRIX {
        let reports = kill_and_verify(*point, *scenario);
        let get = |k| value(&reports, k).unwrap_or("?").to_owned();
        table.push(format!(
            "{:<34} {:<9} -> {}",
            point.name(),
            scenario.name(),
            get("outcome")
        ));
        if get("integrity_issues") != "0" {
            failures.push(format!(
                "{}: store integrity issues {}",
                point.name(),
                get("integrity_issues")
            ));
        }
        if get("lock_files") != "0" || get("half_merges") != "0" {
            failures.push(format!(
                "{}: leftover Git locks or half merge",
                point.name()
            ));
        }
        if get("duplicate_merges") != "0" {
            failures.push(format!("{}: a ticket was merged twice", point.name()));
        }
        if *scenario == Scenario::Approval
            && get("approval_deliveries").parse::<i64>().unwrap_or(99) > 1
        {
            failures.push(format!("{}: an approval was delivered twice", point.name()));
        }
        if get("outcome") == "reconcile_failed" {
            failures.push(format!("{}: reconciliation failed", point.name()));
        }
    }
    eprintln!("crash matrix:\n{}", table.join("\n"));
    assert!(failures.is_empty(), "{failures:#?}\n{}", table.join("\n"));
}

fn kill_with(point: FaultPoint, scenario: Scenario, continuation: Option<&str>) -> Vec<String> {
    let mut h = KillRestartHarness::new("matrix_child")
        .expect("harness")
        .with_timeout(Duration::from_secs(240))
        .with_env("VELA_MATRIX_SCENARIO", scenario.name());
    if let Some(c) = continuation {
        h = h.with_env("VELA_MATRIX_CONTINUATION", c);
    }
    let r = h
        .kill_and_restart(point)
        .unwrap_or_else(|e| panic!("{}: {e}", point.name()));
    assert!(
        r.restart.success,
        "restart failed:
{}
{}",
        r.restart.stdout, r.restart.stderr
    );
    r.restart.reports.clone()
}

/// AT-005: terminate after the checkpoint but before the worker completion is recorded; the relaunch detects the commit and
/// resumes without duplicate implementation.
#[test]
fn at_005_a_crash_after_the_checkpoint_resumes_without_implementing_again() {
    let reports = kill_with(FaultPoint::WorkerAfterCheckpoint, Scenario::Flow, None);
    assert_eq!(value(&reports, "duplicate_merges"), Some("0"));
    assert_eq!(value(&reports, "integrity_issues"), Some("0"));
    assert_eq!(value(&reports, "lock_files"), Some("0"));
    // The checkpoint commit is found and used: nothing is left half done.
    assert_ne!(value(&reports, "outcome"), Some("reconcile_failed"));
}

/// AT-019 (reboot): reconciliation runs first, then continuation follows the persisted setting. With `ask` the run waits for the
/// user; with `auto_safe` it continues by itself only when reconciliation found nothing.
#[test]
fn at_019_after_a_reboot_the_continuation_setting_decides() {
    let ask = kill_with(
        FaultPoint::WorkerAfterCheckpoint,
        Scenario::Flow,
        Some("ask"),
    );
    assert_eq!(
        value(&ask, "state_after_reconcile"),
        Some("PAUSED"),
        "{ask:?}"
    );
    assert_eq!(
        value(&ask, "paused_reason"),
        Some("AwaitingRecoveryConfirmation"),
        "{ask:?}"
    );
    let auto = kill_with(
        FaultPoint::WorkerAfterCheckpoint,
        Scenario::Flow,
        Some("auto_safe"),
    );
    let state = value(&auto, "state_after_reconcile").unwrap_or("?");
    let reason = value(&auto, "paused_reason");
    // Either it carried on (RUNNING), or reconciliation found something and it paused for the user; it never waits on the
    // blanket confirmation that only `ask` uses.
    assert!(
        state == "RUNNING" || (state == "PAUSED" && reason != Some("AwaitingRecoveryConfirmation")),
        "{auto:?}"
    );
}
