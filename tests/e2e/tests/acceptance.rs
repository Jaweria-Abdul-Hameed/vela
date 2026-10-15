//! Z03: the acceptance tests that do not need real Antigravity, end to end over real fixture repositories (real Git, real process
//! runner, the whole core) with scripted agents. AT-005 and AT-019 (crash and reboot) live in `crash_matrix.rs`, which owns the
//! kill-and-restart machinery; AT-004 (review) is not run because review is off while building (ADR-023, ADR-024).
//!
//! Covered here: AT-001, AT-002, AT-003, AT-006, AT-007 (capacity and provider policy block), AT-010, AT-018, AT-020, AT-021
//! and AT-022.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use vela_core::analysis::{Analysis, AnalystHook, stored_graph};
use vela_core::capacity::CapacityService;
use vela_core::intervention::Interventions;
use vela_core::orchestrator::Services;
use vela_core::orchestrator::driver::{DriveOutcome, RunConfig, RunDriver};
use vela_core::orchestrator::runs::Runs;
use vela_core::recovery::{Recovery, RecoveryInput};
use vela_core::stop::StopService;
use vela_core::trust::{ProfileCommand, TrustService};
use vela_domain::ids::{RunId, WorktreePath};
use vela_domain::policy::{Decision, Dialect, Operation, PolicyEngine, RunMode};
use vela_domain::state::{RunState, WorkerState};
use vela_e2e_tests::{
    Behavior, Fixture, PlainPrompts, ScenarioAgent, ScriptedAnalyst, config, driver_with, git,
    git_adapter, issue, runner, services,
};

struct World {
    fx: Fixture,
    services: Arc<Services>,
    git: Arc<vela_git::read::GitCli>,
    project: String,
    driver: Arc<RunDriver>,
    cfg: RunConfig,
    agent: Arc<ScenarioAgent>,
}

struct Setup {
    tag: &'static str,
    issues: Vec<(&'static str, String)>,
    /// Files each ticket is predicted to write; absent means no analyst (every pair runs one at a time).
    write_sets: Option<BTreeMap<String, Vec<String>>>,
    remote: bool,
    gates: Vec<&'static str>,
    trust: bool,
}

impl Setup {
    fn new(tag: &'static str, issues: Vec<(&'static str, String)>) -> Self {
        Self {
            tag,
            issues,
            write_sets: None,
            remote: false,
            gates: vec![],
            trust: true,
        }
    }
}

async fn world(setup: Setup, script: impl FnOnce(&ScenarioAgent)) -> World {
    let refs: Vec<(&str, &str)> = setup.issues.iter().map(|(a, b)| (*a, b.as_str())).collect();
    let fx = Fixture::new(setup.tag, &refs);
    if setup.remote {
        let remote = fx.base.join("remote.git");
        std::fs::create_dir_all(&remote).unwrap();
        git(&remote, &["init", "-q", "--bare", "-b", "main"]);
        git(
            &fx.repo,
            &["remote", "add", "origin", &remote.to_string_lossy()],
        );
        git(&fx.repo, &["push", "-q", "origin", "main"]);
    }
    let services = services(&fx);
    vela_core::capacity::register(&services);
    vela_core::broker::register_recipes(&services);
    vela_core::merge_lane::register_recipes(&services);
    let git = git_adapter(&fx);
    vela_core::operations::register_checkpoint_recipe(&services, git.clone());
    let projects = vela_core::project::ProjectService::new(services.clone(), git.clone());
    let imported = projects.import(&fx.repo_path()).await.unwrap();
    let project = imported.project.id;
    if setup.trust {
        TrustService::new(&services)
            .trust(
                &project,
                &imported.project.repo_path,
                &setup
                    .gates
                    .iter()
                    .map(|g| ProfileCommand {
                        text: (*g).to_owned(),
                        cwd: None,
                    })
                    .collect::<Vec<_>>(),
                "e2e",
            )
            .unwrap();
    }
    let agent = Arc::new(ScenarioAgent::new());
    script(&agent);
    let analyst: Option<Arc<dyn AnalystHook>> = setup
        .write_sets
        .map(|w| Arc::new(ScriptedAnalyst(w)) as Arc<dyn AnalystHook>);
    let driver = driver_with(
        &services,
        &git,
        agent.clone(),
        &fx,
        Arc::new(PlainPrompts),
        analyst,
    );
    let mut cfg = config(&fx, &project);
    if setup.remote {
        cfg.remote = Some(vela_domain::ports::RemoteName::new("origin").unwrap());
    }
    cfg.validation = setup
        .gates
        .iter()
        .map(|g| ProfileCommand {
            text: (*g).to_owned(),
            cwd: None,
        })
        .collect();
    World {
        fx,
        services,
        git,
        project,
        driver,
        cfg,
        agent,
    }
}

impl World {
    async fn start(&self) -> RunId {
        let (run, analysis) = self.driver.prepare(&self.cfg).await.expect("prepare");
        assert!(
            matches!(analysis, Analysis::Ready { .. }),
            "the graph is valid"
        );
        self.driver
            .approve_and_start(&run, &self.cfg, None, false)
            .await
            .expect("start");
        run
    }

    async fn run_to_end(&self) -> (RunId, DriveOutcome) {
        let run = self.start().await;
        let outcome =
            tokio::time::timeout(Duration::from_secs(180), self.driver.drive(&run, &self.cfg))
                .await
                .expect("the run finishes in time")
                .expect("drive");
        (run, outcome)
    }

    fn state(&self, run: &RunId) -> String {
        Runs::new(&self.services)
            .run_state(run)
            .unwrap()
            .name()
            .to_owned()
    }

    fn integration(&self, run: &RunId) -> PathBuf {
        PathBuf::from(format!(
            "{}/{}/integration",
            self.cfg.worktree_root.trim_end_matches('/'),
            run.as_str()
        ))
    }

    fn out(&self, dir: &std::path::Path, args: &[&str]) -> String {
        let o = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .unwrap();
        String::from_utf8_lossy(&o.stdout).trim().to_owned()
    }

    fn merges(&self, run: &RunId) -> usize {
        self.out(&self.integration(run), &["log", "--merges", "--format=%H"])
            .lines()
            .filter(|l| !l.is_empty())
            .count()
    }

    fn main_head(&self) -> String {
        self.out(&self.fx.repo, &["rev-parse", "main"])
    }

    fn open_interventions(&self, run: &RunId) -> Vec<String> {
        Interventions::new(&self.services)
            .open_for_run(run)
            .unwrap()
            .iter()
            .map(|i| format!("{i:?}"))
            .collect()
    }

    fn workers(&self, run: &RunId) -> Vec<(String, WorkerState)> {
        let r = run.as_str().to_owned();
        self.services
            .store
            .read(|c| vela_persistence::repositories::o01_workers::list_for_run(c, &r))
            .unwrap()
            .into_iter()
            .map(|w| (w.ticket_id, serde_json::from_str(&w.state).unwrap()))
            .collect()
    }
}

fn two(a: &str, b: &str) -> Vec<(&'static str, String)> {
    vec![
        ("T-1", issue("T-1", "first", &[], a)),
        ("T-2", issue("T-2", "second", &[], b)),
    ]
}

fn writes(pairs: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
    pairs
        .iter()
        .map(|(t, f)| ((*t).to_owned(), f.iter().map(|x| (*x).to_owned()).collect()))
        .collect()
}

// ------------------------------------------------------------------------------------------------------------------ AT-001

#[tokio::test]
async fn at_001_one_ticket_runs_unattended_to_completion_without_approvals() {
    let w = world(
        Setup::new(
            "at001",
            vec![("T-1", issue("T-1", "only", &[], "write the file"))],
        ),
        |_| {},
    )
    .await;
    let main_before = w.main_head();
    let (run, outcome) = w.run_to_end().await;
    assert!(matches!(outcome, DriveOutcome::Completed(_)), "{outcome:?}");
    assert_eq!(w.state(&run), "COMPLETED");
    assert!(w.integration(&run).join("t-1.txt").exists());
    assert_eq!(w.merges(&run), 1);
    // No ordinary command approvals were asked for.
    let asked: i64 = w
        .services
        .store
        .read(|c| {
            c.query_row("SELECT COUNT(*) FROM approval_requests", [], |r| r.get(0))
                .map_err(|e| vela_domain::errors::VelaError::internal(e.to_string()))
        })
        .unwrap_or(0);
    assert_eq!(asked, 0);
    // AT-021: Vela does not merge the default branch.
    assert_eq!(w.main_head(), main_before);
    assert_eq!(w.workers(&run).len(), 1);
}

// ------------------------------------------------------------------------------------------------------------------ AT-002 / AT-003

#[tokio::test]
async fn at_002_two_independent_tickets_run_at_once_in_separate_worktrees_and_merge_serially() {
    let mut s = Setup::new("at002", two("write a", "write b"));
    s.write_sets = Some(writes(&[("T-1", &["src/a.txt"]), ("T-2", &["src/b.txt"])]));
    let w = world(s, |a| {
        a.set("T-1", Behavior::Slow(1200));
        a.set("T-2", Behavior::Slow(1200));
    })
    .await;
    let (run, outcome) = w.run_to_end().await;
    assert!(matches!(outcome, DriveOutcome::Completed(_)), "{outcome:?}");
    assert_eq!(
        w.agent.max_concurrent(),
        2,
        "both tickets ran at the same time"
    );
    let trees: Vec<String> = w.workers(&run).into_iter().map(|(t, _)| t).collect();
    assert_eq!(trees.len(), 2);
    // Serial merges, no lost change.
    assert_eq!(w.merges(&run), 2);
    assert!(w.integration(&run).join("t-1.txt").exists());
    assert!(w.integration(&run).join("t-2.txt").exists());
}

#[tokio::test]
async fn at_003_two_tickets_touching_a_shared_schema_are_not_run_together_and_the_reason_is_recorded()
 {
    let mut s = Setup::new("at003", two("change the schema", "also change the schema"));
    s.write_sets = Some(writes(&[
        ("T-1", &["db/schema.sql"]),
        ("T-2", &["db/schema.sql"]),
    ]));
    let w = world(s, |a| {
        a.set("T-1", Behavior::Slow(400));
        a.set("T-2", Behavior::Slow(400));
    })
    .await;
    let (run, outcome) = w.run_to_end().await;
    assert!(matches!(outcome, DriveOutcome::Completed(_)), "{outcome:?}");
    assert_eq!(w.agent.max_concurrent(), 1, "never at the same time");
    let graph = stored_graph(&w.services, &run)
        .unwrap()
        .expect("a stored graph");
    let pair = graph.verdicts.first().expect("the pair has a verdict");
    let reasons = format!("{:?}", pair.reasons);
    assert!(
        reasons.contains("SharedFile") || reasons.contains("Schema"),
        "the UI can say why: {reasons}"
    );
}

// ------------------------------------------------------------------------------------------------------------------ AT-010

#[tokio::test]
async fn at_010_a_contradictory_ticket_stops_for_a_human_with_no_speculative_change() {
    let w = world(
        Setup::new(
            "at010",
            vec![(
                "T-1",
                issue("T-1", "contradiction", &[], "make it both fast and slow"),
            )],
        ),
        |a| {
            a.set("T-1", Behavior::Blocked);
        },
    )
    .await;
    let main_before = w.main_head();
    let (run, outcome) = w.run_to_end().await;
    assert!(matches!(outcome, DriveOutcome::Waiting(_)), "{outcome:?}");
    let open = w.open_interventions(&run);
    assert_eq!(open.len(), 1, "{open:?}");
    assert!(
        open[0].to_uppercase().contains("AMBIGUITY") || open[0].to_uppercase().contains("OTHER"),
        "{}",
        open[0]
    );
    assert_eq!(w.merges(&run), 0, "nothing was merged");
    assert!(!w.integration(&run).join("t-1.txt").exists());
    assert_eq!(w.main_head(), main_before);
}

// ------------------------------------------------------------------------------------------------------------------ AT-018

#[tokio::test]
async fn at_018_safe_parallel_tickets_merge_through_the_lane_with_merge_commits_and_no_force_push()
{
    let mut s = Setup::new("at018a", two("a", "b"));
    s.write_sets = Some(writes(&[("T-1", &["a.txt"]), ("T-2", &["b.txt"])]));
    s.remote = true;
    let w = world(s, |_| {}).await;
    let (run, outcome) = w.run_to_end().await;
    assert!(matches!(outcome, DriveOutcome::Completed(_)), "{outcome:?}");
    assert_eq!(w.merges(&run), 2, "one merge commit per ticket");
    // The remote received the integration branch by an ordinary push: its history is a fast-forward of what it had.
    let remote = w.fx.base.join("remote.git");
    let branch = format!("vela/{}/integration", run.as_str());
    let local = w.out(&w.fx.repo, &["rev-parse", &branch]);
    assert_eq!(w.out(&remote, &["rev-parse", &branch]), local);
    let reflog_force = w.out(&remote, &["reflog", "show", &branch]);
    assert!(
        !reflog_force.to_lowercase().contains("forced"),
        "{reflog_force}"
    );
}

#[tokio::test]
async fn at_018_an_injected_conflict_is_not_resolved_automatically() {
    let mut s = Setup::new("at018b", two("write shared", "write shared too"));
    // The analyst wrongly believes they are independent, so they run together and collide at merge time.
    s.write_sets = Some(writes(&[("T-1", &["one.txt"]), ("T-2", &["two.txt"])]));
    let w = world(s, |a| {
        a.set("T-1", Behavior::WriteShared("from ticket one\n".into()));
        a.set("T-2", Behavior::WriteShared("from ticket two\n".into()));
    })
    .await;
    let (run, outcome) = w.run_to_end().await;
    assert!(matches!(outcome, DriveOutcome::Waiting(_)), "{outcome:?}");
    // One merged, one waiting for a decision; the shared file holds exactly one side, never a blend.
    assert_eq!(w.merges(&run), 1);
    let shared = std::fs::read_to_string(w.integration(&run).join("shared.txt")).unwrap();
    assert!(
        shared == "from ticket one\n" || shared == "from ticket two\n",
        "{shared:?}"
    );
    assert!(!shared.contains("<<<<<<<"));
    assert!(
        !w.open_interventions(&run).is_empty()
            || w.workers(&run)
                .iter()
                .any(|(_, s)| !matches!(s, WorkerState::Complete))
    );
    // The integration worktree is clean and not mid-merge.
    assert!(!w.integration(&run).join(".git").is_dir());
    assert_eq!(w.out(&w.integration(&run), &["status", "--porcelain"]), "");
}

#[tokio::test]
async fn at_018_an_integration_validation_failure_discards_the_unpublished_merge() {
    let mut s = Setup::new(
        "at018c",
        vec![("T-1", issue("T-1", "only", &[], "write the file"))],
    );
    // The gate passes on the worker's uncommitted work and fails on the committed merge result.
    s.gates = vec!["git diff --exit-code main"];
    let w = world(s, |_| {}).await;
    let run = w.start().await;
    let before = w.out(&w.integration(&run), &["rev-parse", "HEAD"]);
    let outcome = tokio::time::timeout(Duration::from_secs(120), w.driver.drive(&run, &w.cfg))
        .await
        .expect("finishes")
        .expect("drive");
    assert!(
        !matches!(outcome, DriveOutcome::Completed(_)),
        "a failing gate is never success"
    );
    // The merge was discarded: the integration branch is where it was, with nothing half merged.
    assert_eq!(w.out(&w.integration(&run), &["rev-parse", "HEAD"]), before);
    assert_eq!(w.out(&w.integration(&run), &["status", "--porcelain"]), "");
    assert!(!w.integration(&run).join("t-1.txt").exists());
}

// ------------------------------------------------------------------------------------------------------------------ AT-020

#[tokio::test]
async fn at_020_an_untrusted_repository_is_analyzed_as_data_and_build_is_refused_until_trusted() {
    let mut s = Setup::new(
        "at020",
        vec![("T-1", issue("T-1", "only", &[], "write the file"))],
    );
    s.trust = false;
    let w = world(s, |_| {}).await;
    // A repository-controlled script that would leave a marker if anything ran it.
    std::fs::write(
        w.fx.repo.join("package.json"),
        r#"{"scripts":{"test":"node -e \"require('fs').writeFileSync('MARKER','ran')\""}}"#,
    )
    .unwrap();
    // Preflight and graph analysis work as data only.
    let (run, analysis) = w.driver.prepare(&w.cfg).await.expect("analysis is allowed");
    assert!(matches!(analysis, Analysis::Ready { .. }));
    // Build is refused.
    let err = w
        .driver
        .approve_and_start(&run, &w.cfg, None, false)
        .await
        .expect_err("an untrusted repository cannot be built");
    assert!(
        err.what_failed().to_lowercase().contains("trust"),
        "{}",
        err.what_failed()
    );
    assert!(
        !w.fx.repo.join("MARKER").exists(),
        "no repository script ran"
    );
    // A script merely named `test` outside the confirmed profile is ASK even after trusting.
    TrustService::new(&w.services)
        .trust(&w.project, &w.fx.repo_path(), &[], "e2e")
        .unwrap();
    let ctx = TrustService::new(&w.services)
        .policy_context(&w.project, &w.fx.repo_path(), RunMode::Autonomous)
        .unwrap();
    let d = PolicyEngine::new(ctx).evaluate(&Operation::Command {
        dialect: Dialect::Cmd,
        text: "npm test".into(),
        cwd: None,
    });
    assert_eq!(d.decision, Decision::Ask);
}

// ------------------------------------------------------------------------------------------------------------------ AT-021

#[tokio::test]
async fn at_021_with_a_remote_the_run_leaves_a_validated_integration_branch_and_never_merges_the_default_branch()
 {
    let mut s = Setup::new(
        "at021a",
        vec![("T-1", issue("T-1", "only", &[], "write the file"))],
    );
    s.remote = true;
    let w = world(s, |_| {}).await;
    let remote = w.fx.base.join("remote.git");
    let remote_main = w.out(&remote, &["rev-parse", "main"]);
    let main_before = w.main_head();
    let (run, outcome) = w.run_to_end().await;
    assert!(matches!(outcome, DriveOutcome::Completed(_)), "{outcome:?}");
    let branch = format!("vela/{}/integration", run.as_str());
    assert_eq!(
        w.out(&remote, &["rev-parse", &branch]),
        w.out(&w.fx.repo, &["rev-parse", &branch]),
        "the validated integration branch is on the remote"
    );
    assert_eq!(
        w.out(&remote, &["rev-parse", "main"]),
        remote_main,
        "the remote default branch is untouched"
    );
    assert_eq!(w.main_head(), main_before);
    let promo = w
        .services
        .store
        .read(|c| vela_persistence::repositories::t04_promotions::get(c, run.as_str()))
        .unwrap()
        .expect("a promotion record");
    assert_ne!(promo.state, "MERGED");
}

#[tokio::test]
async fn at_021_without_a_remote_the_integration_branch_is_marked_ready_for_human_promotion() {
    let w = world(
        Setup::new(
            "at021b",
            vec![("T-1", issue("T-1", "only", &[], "write the file"))],
        ),
        |_| {},
    )
    .await;
    let main_before = w.main_head();
    let (run, outcome) = w.run_to_end().await;
    assert!(matches!(outcome, DriveOutcome::Completed(_)), "{outcome:?}");
    let promo = w
        .services
        .store
        .read(|c| vela_persistence::repositories::t04_promotions::get(c, run.as_str()))
        .unwrap()
        .expect("a promotion record");
    assert!(
        promo.state.to_uppercase().contains("LOCAL"),
        "{}",
        promo.state
    );
    assert_eq!(w.main_head(), main_before);
    assert!(
        !w.out(
            &w.fx.repo,
            &[
                "branch",
                "--list",
                &format!("vela/{}/integration", run.as_str())
            ]
        )
        .is_empty()
    );
}

// ------------------------------------------------------------------------------------------------------------------ AT-006

#[tokio::test]
async fn at_006_stop_all_prevents_new_work_preserves_worktrees_and_allows_a_later_resume() {
    let mut s = Setup::new("at006", two("write a", "write b"));
    s.write_sets = Some(writes(&[("T-1", &["a.txt"]), ("T-2", &["b.txt"])]));
    let w = world(s, |a| {
        a.set("T-1", Behavior::Slow(2500));
        a.set("T-2", Behavior::Slow(2500));
    })
    .await;
    let run = w.start().await;
    let driver = w.driver.clone();
    let cfg = w.cfg.clone();
    let run2 = run.clone();
    let task = tokio::spawn(async move { driver.drive(&run2, &cfg).await });
    let abort = task.abort_handle();
    // Let both workers get going, then stop everything.
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let stop = StopService::new(
        w.services.clone(),
        w.agent.clone(),
        runner(),
        w.git.clone(),
        w.driver.lane(),
        Duration::from_secs(5),
    );
    let integration =
        WorktreePath::new(w.integration(&run).to_string_lossy().replace('\\', "/")).unwrap();
    // Like the application, the run's driver task is cancelled first, then everything is stopped.
    abort.abort();
    let _ = task.await;
    let report = stop
        .stop_all(&run, &w.project, &integration)
        .await
        .expect("stop all");
    assert!(
        matches!(
            Runs::new(&w.services).run_state(&run).unwrap(),
            RunState::Paused { .. }
        ),
        "the run is paused: {}",
        w.state(&run)
    );
    assert!(!report.paused.is_empty() || !w.workers(&run).is_empty());
    // Worktrees and branches are preserved; nothing was merged while stopped.
    assert_eq!(w.merges(&run), 0);
    assert!(
        !w.out(
            &w.fx.repo,
            &["branch", "--list", &format!("vela/{}/*", run.as_str())]
        )
        .is_empty()
    );
    // Later: reconcile, resume and finish.
    let recovery = Recovery::new(w.services.clone(), w.git.clone(), w.agent.clone());
    let rr = recovery
        .reconcile(&RecoveryInput {
            run: run.clone(),
            project_id: w.project.clone(),
            repo: w.cfg.repo.clone(),
            reason: "resume".into(),
        })
        .await
        .expect("reconcile");
    stop.resume(&run, &rr).await.expect("resume");
    let outcome = tokio::time::timeout(Duration::from_secs(180), w.driver.drive(&run, &w.cfg))
        .await
        .expect("finishes")
        .expect("drive");
    if !matches!(outcome, DriveOutcome::Completed(_)) {
        let failures: Vec<String> = w
            .services
            .store
            .read(|c| {
                let mut s = c
                    .prepare("SELECT kind, payload FROM event_journal ORDER BY seq")
                    .unwrap();
                Ok(s.query_map([], |r| {
                    Ok(format!(
                        "{} {}",
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?.chars().take(110).collect::<String>()
                    ))
                })
                .unwrap()
                .map(Result::unwrap)
                .collect())
            })
            .unwrap();
        panic!("{outcome:?} {failures:#?}");
    }
    assert_eq!(w.merges(&run), 2);
}

// ------------------------------------------------------------------------------------------------------------------ AT-007

#[tokio::test]
async fn at_007_a_capacity_pause_stops_only_the_affected_worker_and_it_resumes_when_the_profile_returns()
 {
    let mut s = Setup::new("at007a", two("write a", "write b"));
    s.write_sets = Some(writes(&[("T-1", &["a.txt"]), ("T-2", &["b.txt"])]));
    let w = world(s, |a| {
        a.set("T-1", Behavior::CapacityOnce(Some(1500)));
    })
    .await;
    let run = w.start().await;
    // The first drive: T-2 completes while T-1 is paused for capacity.
    let outcome = tokio::time::timeout(Duration::from_secs(120), w.driver.drive(&run, &w.cfg))
        .await
        .expect("finishes")
        .expect("drive");
    let states = w.workers(&run);
    let t1 = states
        .iter()
        .find(|(t, _)| t == "T-1")
        .expect("T-1 has a worker")
        .1
        .clone();
    assert!(
        matches!(t1, WorkerState::Paused { .. }),
        "T-1 is paused for capacity, not failed: {t1:?} ({outcome:?})"
    );
    let t2 = states
        .iter()
        .find(|(t, _)| t == "T-2")
        .expect("T-2 has a worker")
        .1
        .clone();
    assert_eq!(
        t2,
        WorkerState::Complete,
        "the unaffected ticket carried on"
    );
    // The profile is cooling down; when the time passes the paused worker resumes where it was.
    let cap = CapacityService::new(w.services.clone());
    assert_eq!(cap.profile("default").unwrap().state, "COOLDOWN");
    tokio::time::sleep(Duration::from_millis(1700)).await;
    let resumed = cap.tick(&run, "default").await.expect("tick");
    assert_eq!(resumed.len(), 1);
    let outcome = tokio::time::timeout(Duration::from_secs(120), w.driver.drive(&run, &w.cfg))
        .await
        .expect("finishes")
        .expect("drive");
    assert!(matches!(outcome, DriveOutcome::Completed(_)), "{outcome:?}");
}

#[tokio::test]
async fn at_007_a_provider_policy_block_is_held_never_retried_and_survives_a_restart() {
    let w = world(
        Setup::new(
            "at007b",
            vec![("T-1", issue("T-1", "only", &[], "write the file"))],
        ),
        |a| {
            a.set("T-1", Behavior::PolicyBlock);
        },
    )
    .await;
    let (run, outcome) = w.run_to_end().await;
    assert!(matches!(outcome, DriveOutcome::Waiting(_)), "{outcome:?}");
    let open = w.open_interventions(&run);
    assert!(
        open.iter().any(|i| i.to_uppercase().contains("POLICY")),
        "{open:?}"
    );
    // No retry: the provider was asked exactly once, and no work merged.
    assert_eq!(w.agent.turns(), vec!["t-1".to_owned()]);
    assert_eq!(w.merges(&run), 0);
    // A fresh process over the same store: reconciliation leaves the block in place and starts no agent work.
    drop(w.driver);
    let reopened = Arc::new(
        vela_persistence::writer::StateStore::open(
            &w.fx.data.join("state.db"),
            vela_persistence::writer::StoreOptions::default(),
        )
        .unwrap(),
    );
    let services2 = Services::new(
        reopened,
        Arc::new(vela_core::orchestrator::system::SystemClock),
        Arc::new(vela_core::orchestrator::system::SystemIds::new()),
    );
    vela_core::capacity::register(&services2);
    let agent2 = Arc::new(ScenarioAgent::new());
    let recovery = Recovery::new(services2.clone(), w.git.clone(), agent2.clone());
    let _ = recovery
        .reconcile(&RecoveryInput {
            run: run.clone(),
            project_id: w.project.clone(),
            repo: w.cfg.repo.clone(),
            reason: "startup".into(),
        })
        .await
        .expect("reconcile");
    assert!(agent2.turns().is_empty(), "recovery started no agent work");
    let still = Interventions::new(&services2).open_for_run(&run).unwrap();
    assert!(
        still
            .iter()
            .any(|i| format!("{i:?}").to_uppercase().contains("POLICY"))
    );
}

// ------------------------------------------------------------------------------------------------------------------ AT-022

#[tokio::test]
async fn at_022_a_corrupt_state_store_restores_from_backup_or_falls_back_to_a_read_only_git_inventory()
 {
    use vela_persistence::recovery::{StoreRecovery, backup, backup_dir, recover_store};
    let w = world(
        Setup::new(
            "at022",
            vec![("T-1", issue("T-1", "only", &[], "write the file"))],
        ),
        |_| {},
    )
    .await;
    let (run, outcome) = w.run_to_end().await;
    assert!(matches!(outcome, DriveOutcome::Completed(_)), "{outcome:?}");
    let db = w.fx.data.join("state.db");
    // A backup exists (made on a clean shutdown), then the live store is damaged.
    {
        let conn = rusqlite_open(&db);
        backup(&conn, &db, 1, 1).unwrap();
    }
    let branches_before = w.out(&w.fx.repo, &["branch", "--list", "vela/*"]);
    let integration = w.integration(&run);
    // Every holder of the store is released, as a process exit would, before the file is damaged.
    let World {
        fx,
        services,
        git,
        driver,
        cfg,
        agent,
        ..
    } = w;
    drop((services, driver, agent));
    std::fs::write(&db, b"this is not a database any more").unwrap();
    for ext in ["-wal", "-shm"] {
        let mut p = db.as_os_str().to_os_string();
        p.push(ext);
        let _ = std::fs::remove_file(PathBuf::from(p));
    }
    // 1. A good backup is restored; nothing auto-resumes and no branch or worktree is lost.
    let r = recover_store(&db).unwrap();
    assert!(matches!(r, StoreRecovery::Restored(_)), "{r:?}");
    assert_eq!(
        git_out(&fx.repo, &["branch", "--list", "vela/*"]),
        branches_before
    );
    assert!(integration.exists());
    // 2. No usable backup: the store is unrecoverable and the inventory is rebuilt from Git, read-only, every run NEEDS_HUMAN.
    std::fs::write(&db, b"corrupt again").unwrap();
    let _ = std::fs::remove_dir_all(backup_dir(&db));
    assert_eq!(recover_store(&db).unwrap(), StoreRecovery::Unrecoverable);
    let ctx = vela_domain::ports::GitContext {
        trust: vela_domain::ports::RepoTrust::Trusted,
        operation: vela_domain::ids::OperationId::new("op-inventory").unwrap(),
    };
    let inv = vela_core::recovery::inventory::rebuild(&*git, &ctx, &cfg.repo)
        .await
        .unwrap();
    eprintln!(
        "branches: {branches_before:?} inventory: {inv:?} raw: {:?}",
        vela_domain::ports::GitHistory::list_branches(&*git, &ctx, &cfg.repo, "vela/").await
    );
    assert!(!inv.auto_resumes());
    assert_eq!(inv.runs.len(), 1);
    assert_eq!(inv.runs[0].run_id, run.as_str());
    assert_eq!(
        inv.runs[0].state,
        vela_core::recovery::inventory::Reconstructed::NeedsHuman
    );
    assert!(inv.runs[0].integration.is_some());
    assert_eq!(
        git_out(&fx.repo, &["branch", "--list", "vela/*"]),
        branches_before,
        "branches preserved"
    );
}

fn rusqlite_open(path: &std::path::Path) -> rusqlite::Connection {
    rusqlite::Connection::open(path).unwrap()
}

fn git_out(dir: &std::path::Path, args: &[&str]) -> String {
    let o = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    String::from_utf8_lossy(&o.stdout).trim().to_owned()
}
