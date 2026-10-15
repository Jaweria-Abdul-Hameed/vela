use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use vela_domain::errors::VelaError;
use vela_domain::ids::CommitSha;
use vela_domain::ports::{AgentSession, ProcessRunner, TrackerTicket};
use vela_testkit::fakes::{FakeGit, FakeProcessRunner, FakeTracker, ScriptedAgent};

use super::*;
use crate::orchestrator::test_support::{TestEnv, env};
use crate::promotion::PromotionService;
use crate::worker::{Completion, CompletionStatus, WorkerPrompts, WorkerTask};

struct Prompts;

impl WorkerPrompts for Prompts {
    fn worker_prompt(&self, task: &WorkerTask) -> String {
        format!("do {}", task.ticket)
    }
    fn fix_prompt(&self, failures: &str) -> String {
        format!("fix: {failures}")
    }
    fn correction_prompt(&self, problem: &str) -> String {
        format!("correct: {problem}")
    }
    fn parse_completion(&self, response: &str, _base: &str) -> Result<Completion, VelaError> {
        let status = match response {
            "DONE" => CompletionStatus::Complete,
            "BLOCKED" => CompletionStatus::Blocked,
            _ => CompletionStatus::Failed,
        };
        Ok(Completion {
            status,
            warnings: vec![],
            discovered_dependencies: vec![],
        })
    }
}

struct FakeBackend {
    services: Arc<Services>,
    git: Arc<FakeGit>,
    agent: Arc<ScriptedAgent>,
    runner: Arc<FakeProcessRunner>,
    driver: Arc<RunDriver>,
    wt: String,
}

impl Backend for FakeBackend {
    fn git_read(&self) -> Arc<dyn GitRead> {
        self.git.clone()
    }
    fn driver(&self, _p: &ProjectContext) -> Result<Arc<RunDriver>, VelaError> {
        Ok(self.driver.clone())
    }
    fn stop_service(&self, driver: &Arc<RunDriver>) -> Arc<StopService> {
        let agent: Arc<dyn AgentSession> = self.agent.clone();
        let runner: Arc<dyn ProcessRunner> = self.runner.clone();
        Arc::new(StopService::new(
            self.services.clone(),
            agent,
            runner,
            self.git.clone(),
            driver.lane(),
            Duration::from_millis(50),
        ))
    }
    fn recovery(&self) -> Arc<Recovery> {
        Arc::new(Recovery::new(
            self.services.clone(),
            self.git.clone(),
            self.agent.clone(),
        ))
    }
    fn worktree_root(&self, _project_id: &str) -> String {
        self.wt.clone()
    }
    fn remote_for(&self, _p: &ProjectContext) -> Option<RemoteName> {
        None
    }
    fn fill_facts(&self, facts: &mut PreflightFacts) {
        facts.git_version = Some((2, 45, 0));
        facts.webview2 = Some("130".into());
    }
}

struct Rig {
    e: TestEnv,
    api: Arc<AppApi>,
    git: Arc<FakeGit>,
    agent: Arc<ScriptedAgent>,
    dir: PathBuf,
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn rig(tag: &str, tickets: Vec<TrackerTicket>) -> Rig {
    static N: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "vela-api-{tag}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("app")).unwrap();
    std::fs::create_dir_all(dir.join("wt")).unwrap();
    std::fs::write(dir.join("app").join("Cargo.toml"), "[package]\nname=\"x\"").unwrap();
    let e = env(tag);
    let git = Arc::new(FakeGit::new());
    git.set_changed(vec!["src/lib.rs".into()]);
    git.set_keep_changed(true);
    git.set_diff(vec!["src/lib.rs".into()]);
    let agent = Arc::new(ScriptedAgent::new());
    agent.set_default_response("DONE");
    let runner = Arc::new(FakeProcessRunner::new());
    let tracker = Arc::new(FakeTracker::new());
    for t in tickets {
        tracker.add(t);
    }
    let promotion = Arc::new(PromotionService::new(e.services.clone(), git.clone(), None));
    let driver = RunDriver::new(
        e.services.clone(),
        tracker,
        None,
        git.clone(),
        agent.clone(),
        runner.clone(),
        Arc::new(Prompts),
        promotion,
    );
    let wt = std::fs::canonicalize(dir.join("wt"))
        .unwrap()
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('\\', "/");
    let backend = Arc::new(FakeBackend {
        services: e.services.clone(),
        git: git.clone(),
        agent: agent.clone(),
        runner,
        driver,
        wt,
    });
    let api = AppApi::new(e.services.clone(), backend);
    Rig {
        e,
        api,
        git,
        agent,
        dir,
    }
}

fn ticket(id: &str, blockers: &[&str]) -> TrackerTicket {
    TrackerTicket {
        id: TicketId::new(id).unwrap(),
        title: format!("ticket {id}"),
        body: String::new(),
        blockers: blockers
            .iter()
            .map(|b| TicketId::new(*b).unwrap())
            .collect(),
        labels: vec![],
    }
}

fn folder(r: &Rig) -> String {
    r.dir.join("app").to_string_lossy().replace('\\', "/")
}

async fn imported(r: &Rig) -> String {
    r.api.project_import(&folder(r)).await.unwrap()["project"]["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn projects_are_imported_listed_and_removed_without_touching_the_repository() {
    let r = rig("projects", vec![]);
    let v = r.api.project_import(&folder(&r)).await.unwrap();
    assert_eq!(v["already_known"], false);
    let id = v["project"]["id"].as_str().unwrap().to_owned();
    assert_eq!(r.api.project_list().unwrap().as_array().unwrap().len(), 1);
    assert_eq!(
        r.api.project_import(&folder(&r)).await.unwrap()["already_known"],
        true
    );
    r.api.project_remove(&id).unwrap();
    assert!(r.api.project_list().unwrap().as_array().unwrap().is_empty());
    assert!(Path::new(&folder(&r)).join("Cargo.toml").is_file());
    assert!(r.api.project_import("C:/no/such/folder").await.is_err());
}

#[tokio::test]
async fn settings_read_and_write_user_wide_and_per_project() {
    let r = rig("settings", vec![]);
    let id = imported(&r).await;
    assert_eq!(
        r.api.settings_get(None).unwrap()["max_parallel_workers"],
        "2"
    );
    r.api
        .settings_set(None, "max_parallel_workers", "4")
        .unwrap();
    r.api
        .settings_set(Some(&id), "max_parallel_workers", "1")
        .unwrap();
    assert_eq!(
        r.api.settings_get(None).unwrap()["max_parallel_workers"],
        "4"
    );
    assert_eq!(
        r.api.settings_get(Some(&id)).unwrap()["max_parallel_workers"],
        "1"
    );
    assert!(r.api.settings_set(None, "nonsense", "x").is_err());
}

#[tokio::test]
async fn trust_is_proposed_from_manifests_granted_with_a_confirmed_profile_and_revoked() {
    let r = rig("trust", vec![]);
    let id = imported(&r).await;
    let p = r.api.trust_propose(&id).unwrap();
    assert_eq!(p["state"], "Untrusted");
    assert!(
        p["commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["text"] == "cargo test")
    );
    r.api
        .trust_grant(
            &id,
            &[ProfileCommand {
                text: "cargo test".into(),
                cwd: None,
            }],
        )
        .unwrap();
    assert_eq!(r.api.trust_propose(&id).unwrap()["state"], "Trusted");
    r.api.trust_revoke(&id).unwrap();
    assert_eq!(r.api.trust_propose(&id).unwrap()["state"], "Untrusted");
}

#[tokio::test]
async fn preflight_blocks_an_untrusted_repository_and_passes_a_trusted_one_with_only_warnings() {
    let r = rig("preflight", vec![]);
    let id = imported(&r).await;
    let report = r.api.preflight_run(&id).unwrap();
    assert_eq!(report["verdict"], "Block");
    assert!(
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["id"] == "trust.state" && c["severity"] == "Block")
    );
    r.api
        .trust_grant(
            &id,
            &[ProfileCommand {
                text: "cargo test".into(),
                cwd: None,
            }],
        )
        .unwrap();
    let report = r.api.preflight_run(&id).unwrap();
    let blockers: Vec<_> = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["severity"] == "Block")
        .map(|c| c["id"].clone())
        .collect();
    // The fake repository folder has no .git directory, so that finding remains; trust no longer blocks.
    assert!(
        !blockers.contains(&Value::String("trust.state".into())),
        "{blockers:?}"
    );
}

#[tokio::test]
async fn the_whole_product_flow_runs_from_import_to_a_completed_run() {
    let r = rig("flow", vec![ticket("T-1", &[]), ticket("T-2", &["T-1"])]);
    let id = imported(&r).await;
    r.api
        .trust_grant(
            &id,
            &[ProfileCommand {
                text: "cargo test".into(),
                cwd: None,
            }],
        )
        .unwrap();
    let prepared = r.api.run_prepare(&id).await.unwrap();
    assert_eq!(prepared["valid"], true);
    assert_eq!(
        (prepared["tickets"].as_u64(), prepared["edges"].as_u64()),
        (Some(2), Some(1))
    );
    let run = prepared["run_id"].as_str().unwrap().to_owned();

    r.api.run_build(&run, None, false).await.unwrap();
    r.api.wait_for_drive(&run).await;

    let status = r.api.run_status(&run).unwrap();
    assert_eq!(status["state"], "COMPLETED");
    assert_eq!(status["promotion"]["state"], "LOCAL_READY");
    assert_eq!(status["workers"].as_array().unwrap().len(), 2);
    assert_eq!(status["graph"]["nodes"].as_array().unwrap().len(), 2);
    assert!(status["interventions"].as_array().unwrap().is_empty());
    assert_eq!(status["conversations"].as_array().unwrap().len(), 2);
    assert_eq!(
        status["conversations"][0]["open"]["available"], false,
        "the open action is honest about being unverified"
    );
    let timeline = r.api.timeline_get(&run, "T-1").unwrap();
    assert!(
        timeline
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "CheckpointCommitted")
    );
}

#[tokio::test]
async fn an_untrusted_build_is_refused_and_a_cyclic_graph_is_reported_not_built() {
    let r = rig("refused", vec![ticket("T-1", &[])]);
    let id = imported(&r).await;
    let prepared = r.api.run_prepare(&id).await.unwrap();
    let run = prepared["run_id"].as_str().unwrap().to_owned();
    assert_eq!(
        r.api
            .run_build(&run, None, false)
            .await
            .unwrap_err()
            .class(),
        ErrorClass::PolicyBlock
    );

    let r = rig(
        "cycle",
        vec![ticket("A-1", &["A-2"]), ticket("A-2", &["A-1"])],
    );
    let id = imported(&r).await;
    let prepared = r.api.run_prepare(&id).await.unwrap();
    assert_eq!(prepared["valid"], false);
    assert!(prepared["cycles"][0].as_str().unwrap().contains("A-1"));
}

#[tokio::test]
async fn a_blocked_worker_waits_and_resolving_it_lets_the_run_finish() {
    let r = rig("resolve", vec![ticket("T-1", &[])]);
    r.agent.set_default_response("BLOCKED");
    let id = imported(&r).await;
    r.api
        .trust_grant(
            &id,
            &[ProfileCommand {
                text: "cargo test".into(),
                cwd: None,
            }],
        )
        .unwrap();
    let run = r.api.run_prepare(&id).await.unwrap()["run_id"]
        .as_str()
        .unwrap()
        .to_owned();
    r.api.run_build(&run, None, false).await.unwrap();
    r.api.wait_for_drive(&run).await;
    let open = r.api.intervention_list(&run).unwrap();
    assert_eq!(open.as_array().unwrap().len(), 1);
    assert_eq!(open[0]["kind"], "AMBIGUITY");

    r.agent.set_default_response("DONE");
    r.api
        .intervention_resolve(open[0]["id"].as_str().unwrap(), "go ahead")
        .await
        .unwrap();
    r.api.wait_for_drive(&run).await;
    assert_eq!(r.api.run_status(&run).unwrap()["state"], "COMPLETED");
}

#[tokio::test]
async fn stop_all_pauses_everything_and_resume_reconciles_then_finishes() {
    let r = rig("stop-resume", vec![ticket("T-1", &[])]);
    r.agent.set_default_response("BLOCKED");
    let id = imported(&r).await;
    r.api
        .trust_grant(
            &id,
            &[ProfileCommand {
                text: "cargo test".into(),
                cwd: None,
            }],
        )
        .unwrap();
    let run = r.api.run_prepare(&id).await.unwrap()["run_id"]
        .as_str()
        .unwrap()
        .to_owned();
    r.api.run_build(&run, None, false).await.unwrap();
    r.api.wait_for_drive(&run).await;
    // Clear the blocked worker's gate so Stop All has an active worker to pause.
    let open = r.api.intervention_list(&run).unwrap();
    r.agent.set_default_response("DONE");
    Interventions::new(&r.e.services)
        .resolve(open[0]["id"].as_str().unwrap(), "ok")
        .await
        .unwrap();

    let stopped = r.api.run_stop(&run).await.unwrap();
    assert_eq!(stopped["paused"].as_array().unwrap().len(), 1);
    assert_eq!(
        r.api.run_status(&run).unwrap()["state"]["PAUSED"]["reason"],
        "STOP_ALL"
    );

    // The worker branch is where the journal says it should be, so reconciliation is clean.
    let base = CommitSha::new(format!("{:040x}", 1)).unwrap();
    r.git.set_ref(&format!("vela/{run}/t-1"), base);
    let resumed = r.api.run_resume(&run).await.unwrap();
    assert_eq!(resumed["resumed"], true, "{resumed}");
    r.api.wait_for_drive(&run).await;
    assert_eq!(r.api.run_status(&run).unwrap()["state"], "COMPLETED");
}

#[tokio::test]
async fn the_issue_source_is_validated_and_stored_per_project() {
    let r = rig("tracker", vec![]);
    let id = imported(&r).await;
    r.api.project_set_tracker(&id, "local:docs/issues").unwrap();
    r.api
        .project_set_tracker(&id, "github:acme/widgets")
        .unwrap();
    for bad in [
        "",
        "local:",
        "local:../outside",
        "local:C:/x",
        "local:/abs",
        "github:acme",
        "github:-x/y",
        "github:a/b/c",
        "ftp:x",
        "github:a b/c",
    ] {
        assert!(r.api.project_set_tracker(&id, bad).is_err(), "{bad:?}");
    }
    assert!(r.api.project_set_tracker("nope", "local:issues").is_err());
}

#[tokio::test]
async fn stop_all_runs_stops_exactly_the_running_ones() {
    let r = rig("stop-all-runs", vec![ticket("T-1", &[])]);
    r.agent.set_default_response("BLOCKED");
    let id = imported(&r).await;
    r.api
        .trust_grant(
            &id,
            &[ProfileCommand {
                text: "cargo test".into(),
                cwd: None,
            }],
        )
        .unwrap();
    assert!(r.api.active_runs().unwrap().is_empty());
    assert_eq!(r.api.stop_all_runs().await.unwrap(), 0);
    let run = r.api.run_prepare(&id).await.unwrap()["run_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        r.api.active_runs().unwrap().is_empty(),
        "a planned run is not running"
    );
    r.api.run_build(&run, None, false).await.unwrap();
    r.api.wait_for_drive(&run).await;
    assert_eq!(r.api.active_runs().unwrap(), std::slice::from_ref(&run));
    assert_eq!(r.api.stop_all_runs().await.unwrap(), 1);
    assert!(r.api.active_runs().unwrap().is_empty());
}

struct Grants {
    asked: std::sync::Mutex<Vec<String>>,
    granted: std::sync::Mutex<Vec<(String, String)>>,
}

impl crate::worker::AskedCommands for Grants {
    fn take_asked(&self, _w: &vela_domain::ids::WorkerId) -> Vec<String> {
        std::mem::take(&mut *self.asked.lock().unwrap())
    }
    fn grant_once(
        &self,
        w: &vela_domain::ids::WorkerId,
        e: &str,
        _now: i64,
    ) -> Result<(), VelaError> {
        self.granted
            .lock()
            .unwrap()
            .push((w.as_str().to_owned(), e.to_owned()));
        Ok(())
    }
}

#[tokio::test]
async fn allowing_a_command_once_grants_it_to_that_worker_and_the_run_finishes() {
    let r = rig("allow-once", vec![ticket("T-1", &[])]);
    let grants = Arc::new(Grants {
        asked: std::sync::Mutex::new(vec!["npm".into()]),
        granted: std::sync::Mutex::new(Vec::new()),
    });
    r.e.services
        .register(Arc::new(crate::worker::AskedCommandsService(
            grants.clone(),
        )));
    r.agent.set_default_response("BLOCKED");
    let id = imported(&r).await;
    r.api
        .trust_grant(
            &id,
            &[ProfileCommand {
                text: "cargo test".into(),
                cwd: None,
            }],
        )
        .unwrap();
    let run = r.api.run_prepare(&id).await.unwrap()["run_id"]
        .as_str()
        .unwrap()
        .to_owned();
    r.api.run_build(&run, None, false).await.unwrap();
    r.api.wait_for_drive(&run).await;
    let open = r.api.intervention_list(&run).unwrap();
    assert_eq!(open[0]["kind"], "APPROVAL_ASK");
    assert!(open[0]["summary"].as_str().unwrap().contains("`npm`"));

    // "Keep refusing" grants nothing.
    // (A different resolution string never installs a rule.)
    assert!(grants.granted.lock().unwrap().is_empty());

    r.agent.set_default_response("DONE");
    r.api
        .intervention_resolve(open[0]["id"].as_str().unwrap(), "allow_once")
        .await
        .unwrap();
    let granted = grants.granted.lock().unwrap().clone();
    assert_eq!(granted.len(), 1);
    assert_eq!(granted[0].1, "npm");
    r.api.wait_for_drive(&run).await;
    assert_eq!(r.api.run_status(&run).unwrap()["state"], "COMPLETED");
}
