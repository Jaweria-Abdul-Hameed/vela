//! Real-environment A05/A04 check: an isolated worker profile with a generated allow rule and the real `vela-hook` binary,
//! driven by the real `agy`. Gated by `VELA_REAL_ANTIGRAVITY=1`; spends a little real quota; holds `RK-AGY-REAL-ACCOUNT`.

use std::num::NonZeroU64;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use vela_adapters::antigravity::events::Executed;
use vela_adapters::antigravity::profile::ProfileManager;
use vela_adapters::antigravity::session::AgyDriver;
use vela_antigravity_compat::{GATE_ENV, gate_enabled};
use vela_domain::ids::{WorkerId, WorktreePath};
use vela_domain::policy::{
    CommandProfile, Dialect, PolicyContext, PolicyEngine, ProfileEntry, RunMode, TrustState,
};
use vela_domain::ports::{AgentEvent, AgentEventSink, AgentSession, TurnRequest};
use vela_process::redact::PatternRedactor;
use vela_process::runner::{TokioProcessRunner, baseline_env};
use vela_process::tree::TreeRegistry;

#[derive(Default)]
struct Events(Mutex<Vec<AgentEvent>>);

impl AgentEventSink for Events {
    fn on_event(&self, event: AgentEvent) {
        self.0.lock().unwrap().push(event);
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Builds `vela-hook` (a no-op when it is current) and returns the executable's path.
fn build_hook() -> PathBuf {
    let status = Command::new("cargo")
        .args(["build", "-p", "vela-hook"])
        .current_dir(workspace_root())
        .status()
        .expect("run cargo");
    assert!(status.success(), "building vela-hook failed");
    let target = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| workspace_root().join("target"));
    let exe = target.join("debug").join("vela-hook.exe");
    assert!(exe.exists(), "{} was not built", exe.display());
    exe
}

fn slash(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

#[tokio::test]
async fn a_real_worker_runs_the_confirmed_command_through_its_isolated_profile_and_the_hook() {
    if !gate_enabled(std::env::var(GATE_ENV).ok().as_deref()) {
        eprintln!("skipped: set {GATE_ENV}=1 to run the real-environment suite");
        return;
    }
    let base = std::env::temp_dir().join(format!("vela-real-profile-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let worktree = base.join("wt");
    std::fs::create_dir_all(&worktree).unwrap();
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(&worktree)
            .args(args)
            .output()
            .unwrap();
    };
    git(&["init", "-q"]);
    std::fs::write(worktree.join("note.txt"), "hello\n").unwrap();

    let root = slash(&worktree);
    let profile = CommandProfile::new(vec![
        ProfileEntry::parse("git status --short", Dialect::PowerShell, None, &root).unwrap(),
    ])
    .unwrap();
    let engine = PolicyEngine::new(
        PolicyContext::new(&root, TrustState::Trusted, RunMode::Autonomous, profile).unwrap(),
    );

    let manager = ProfileManager::new(base.join("profiles"), build_hook());
    let worker = WorkerId::new("w-real").unwrap();
    let wp = manager.create("run-real", &worker, &engine).unwrap();

    let runner = TokioProcessRunner::new(
        Arc::new(PatternRedactor::new()),
        Arc::new(TreeRegistry::new()),
    );
    let driver = AgyDriver::new(Arc::new(runner), baseline_env());
    driver.set_worker_env(worker.clone(), wp.env());

    let request = TurnRequest {
        worker: worker.clone(),
        worktree: WorktreePath::new(root.clone()).unwrap(),
        prompt: "Run the shell command: git status --short. Then report its output. Do not run any other command and do not edit any file.".to_owned(),
        conversation: None,
        timeout_ms: NonZeroU64::new(180_000).unwrap(),
    };
    driver.run_turn(&request, &Events::default()).await.unwrap();
    let outcome = driver.last_outcome(&worker).unwrap();
    let step = outcome
        .steps
        .iter()
        .find(|s| s.tool == "run_command")
        .unwrap_or_else(|| panic!("the agent never ran a command: {outcome:?}"));
    assert_eq!(
        step.executed,
        Executed::Yes,
        "the confirmed command must run: {outcome:?}"
    );
    assert!(
        outcome.response.contains("note.txt"),
        "the output reaches the agent: {}",
        outcome.response
    );

    // The hook saw the call, allowed it, and recorded it without any command text.
    let spooled: String = std::fs::read_dir(&wp.spool_dir)
        .unwrap()
        .flatten()
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
        .collect();
    assert!(
        spooled.contains("\"tool\":\"run_command\"") && spooled.contains("\"decision\":\"allow\""),
        "{spooled}"
    );
    assert!(
        !spooled.contains("git status"),
        "the spool must carry no command text: {spooled}"
    );

    // The profile goes away with the worker; nothing of the user's own home was touched.
    manager.remove("run-real", &worker).unwrap();
    assert!(!wp.home.exists());
    let _ = std::fs::remove_dir_all(&base);
}

/// A skill the user has globally is invisible to an isolated profile until Vela copies it in; once copied, the real `agy`
/// resolves it as a slash command in print mode (probe P4).
#[tokio::test]
async fn a_real_worker_resolves_a_skill_that_vela_provided_to_its_profile() {
    use vela_adapters::antigravity::skills::{detect, provide_to_profile};
    if !gate_enabled(std::env::var(GATE_ENV).ok().as_deref()) {
        eprintln!("skipped: set {GATE_ENV}=1 to run the real-environment suite");
        return;
    }
    let base = std::env::temp_dir().join(format!("vela-real-skill-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let worktree = base.join("wt");
    std::fs::create_dir_all(&worktree).unwrap();
    std::process::Command::new("git")
        .arg("-C")
        .arg(&worktree)
        .args(["init", "-q"])
        .output()
        .unwrap();

    // A skill in a stand-in for the user's global folder.
    let global = base.join("global-home");
    let skill = global.join(".agents").join("skills").join("vela-probe");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("SKILL.md"),
        "---\nname: vela-probe\ndescription: Replies with a fixed marker to prove skills reach worker profiles\n---\n\nReply with exactly the text SKILL-PROBE-OK and nothing else. Do not use any tools.\n",
    )
    .unwrap();

    let root = slash(&worktree);
    let profile = CommandProfile::new(vec![
        ProfileEntry::parse("git status", Dialect::PowerShell, None, &root).unwrap(),
    ])
    .unwrap();
    let engine = PolicyEngine::new(
        PolicyContext::new(&root, TrustState::Trusted, RunMode::Autonomous, profile).unwrap(),
    );
    let manager = ProfileManager::new(base.join("profiles"), build_hook());
    let worker = WorkerId::new("w-skill").unwrap();
    let wp = manager.create("run-skill", &worker, &engine).unwrap();
    let provided = provide_to_profile(
        &wp.home,
        &detect(&worktree, &global, None).found,
        &["vela-probe"],
    )
    .unwrap();
    assert_eq!(provided, vec!["vela-probe".to_owned()]);

    let runner = TokioProcessRunner::new(
        Arc::new(PatternRedactor::new()),
        Arc::new(TreeRegistry::new()),
    );
    let driver = AgyDriver::new(Arc::new(runner), baseline_env());
    driver.set_worker_env(worker.clone(), wp.env());
    let request = TurnRequest {
        worker: worker.clone(),
        worktree: WorktreePath::new(root).unwrap(),
        prompt: "/vela-probe".to_owned(),
        conversation: None,
        timeout_ms: NonZeroU64::new(180_000).unwrap(),
    };
    driver.run_turn(&request, &Events::default()).await.unwrap();
    let outcome = driver.last_outcome(&worker).unwrap();
    assert!(
        outcome.response.contains("SKILL-PROBE-OK"),
        "the skill must run: {outcome:?}"
    );
    manager.remove("run-skill", &worker).unwrap();
    let _ = std::fs::remove_dir_all(&base);
}
