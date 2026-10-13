//! End-to-end support: a real repository, the real Git and process adapters, the whole core. The agent is the only thing replaced
//! (by a scripted one that writes a file) unless the real-account suite is switched on.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use vela_adapters::tracker_local::LocalTracker;
use vela_core::orchestrator::Services;
use vela_core::orchestrator::driver::{RunConfig, RunDriver};
use vela_core::orchestrator::system::{SystemClock, SystemIds};
use vela_core::promotion::PromotionService;
use vela_core::trust::TrustService;
use vela_core::worker::{Completion, CompletionStatus, WorkerPrompts, WorkerTask};
use vela_domain::errors::VelaError;
use vela_domain::ids::{BranchName, WorktreePath};
use vela_domain::ports::{AgentSession, IssueTrackerAdapter};
use vela_git::read::GitCli;
use vela_persistence::writer::{StateStore, StoreOptions};
use vela_process::redact::PatternRedactor;
use vela_process::runner::{TokioProcessRunner, baseline_env};
use vela_process::tree::TreeRegistry;

/// The environment variable that switches the real-account tests on.
pub const GATE_ENV: &str = "VELA_REAL_ANTIGRAVITY";

/// True when real-account tests were asked for.
pub fn real_enabled() -> bool {
    std::env::var(GATE_ENV).is_ok_and(|v| v == "1")
}

/// Runs `git` in `dir` and panics with its output when it fails.
pub fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("git can be started");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A throwaway project: a Git repository with one issue, and a data root beside it.
pub struct Fixture {
    /// The base folder (removed on drop).
    pub base: PathBuf,
    /// The repository.
    pub repo: PathBuf,
    /// Vela's data root.
    pub data: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

impl Fixture {
    /// Creates the repository with the issues given as `(file stem, body)` pairs. `blocked_by` lines go in the body.
    pub fn new(tag: &str, issues: &[(&str, &str)]) -> Self {
        let base = std::env::temp_dir().join(format!("vela-e2e-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let repo = base.join("repo");
        let data = base.join("data");
        std::fs::create_dir_all(repo.join("issues")).unwrap();
        std::fs::create_dir_all(&data).unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["config", "user.name", "Vela Test"]);
        git(&repo, &["config", "user.email", "test@vela.invalid"]);
        git(&repo, &["config", "commit.gpgsign", "false"]);
        std::fs::write(repo.join("README.md"), "# fixture\n").unwrap();
        for (stem, body) in issues {
            std::fs::write(repo.join("issues").join(format!("{stem}.md")), body).unwrap();
        }
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "initial"]);
        Self { base, repo, data }
    }

    /// The repository as a Git path (forward slashes).
    pub fn repo_path(&self) -> String {
        let p = std::fs::canonicalize(&self.repo).unwrap();
        p.to_string_lossy()
            .trim_start_matches(r"\\?\")
            .replace('\\', "/")
    }
}

/// An issue file body in the local tracker's format (a "Dependencies / blocked by" section lists the blockers).
pub fn issue(id: &str, title: &str, blocked_by: &[&str], objective: &str) -> String {
    let blockers = if blocked_by.is_empty() {
        "None".to_owned()
    } else {
        blocked_by
            .iter()
            .map(|b| format!("- {b}"))
            .collect::<Vec<_>>()
            .join(
                "
",
            )
    };
    format!(
        "# {id}: {title}

## Objective
{objective}

## Dependencies / blocked by
{blockers}
"
    )
}

/// The real process runner with its own redactor.
pub fn runner() -> Arc<TokioProcessRunner> {
    Arc::new(TokioProcessRunner::new(
        Arc::new(PatternRedactor::new()),
        Arc::new(TreeRegistry::new()),
    ))
}

/// Services over a real store in the fixture's data root, with the real clock and identifiers.
pub fn services(fx: &Fixture) -> Arc<Services> {
    let store =
        Arc::new(StateStore::open(&fx.data.join("state.db"), StoreOptions::default()).unwrap());
    Services::new(store, Arc::new(SystemClock), Arc::new(SystemIds::new()))
}

/// The worker prompts used by the scripted-agent tests: the agent's reply is `DONE`, `BLOCKED` or anything else (failed).
pub struct PlainPrompts;

impl WorkerPrompts for PlainPrompts {
    fn worker_prompt(&self, task: &WorkerTask) -> String {
        format!("Implement {}: {}\n{}", task.ticket, task.title, task.body)
    }
    fn fix_prompt(&self, failures: &str) -> String {
        format!("Fix this: {failures}")
    }
    fn correction_prompt(&self, problem: &str) -> String {
        format!("Correct your reply: {problem}")
    }
    fn parse_completion(&self, response: &str, _base: &str) -> Result<Completion, VelaError> {
        let status = match response.trim() {
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

/// The run configuration for a fixture project `project_id`.
pub fn config(fx: &Fixture, project_id: &str) -> RunConfig {
    RunConfig {
        project_id: project_id.to_owned(),
        repo: WorktreePath::new(fx.repo_path()).unwrap(),
        repo_root: fx.repo.clone(),
        base_branch: BranchName::new("main").unwrap(),
        worktree_root: format!(
            "{}/worktrees/{project_id}",
            std::fs::canonicalize(&fx.data)
                .unwrap()
                .to_string_lossy()
                .trim_start_matches(r"\\?\")
                .replace('\\', "/")
        ),
        validation: Vec::new(),
        remote: None,
        title: "e2e".to_owned(),
        body: "fixture run".to_owned(),
        spec_pointers: vec!["README.md".to_owned()],
        max_fix_attempts: 1,
    }
}

/// Imports the fixture repository as a project and trusts it with an empty confirmed profile. Returns the project id.
pub async fn import_and_trust(
    services: &Arc<Services>,
    git_adapter: &Arc<GitCli>,
    fx: &Fixture,
) -> String {
    let svc = vela_core::project::ProjectService::new(services.clone(), git_adapter.clone());
    let imported = svc
        .import(&fx.repo_path())
        .await
        .expect("the fixture imports");
    TrustService::new(services)
        .trust(
            &imported.project.id,
            &imported.project.repo_path,
            &[],
            "e2e",
        )
        .expect("trust");
    imported.project.id
}

/// The driver over the real Git adapter and the given agent.
pub fn driver(
    services: &Arc<Services>,
    git_adapter: &Arc<GitCli>,
    agent: Arc<dyn AgentSession>,
    fx: &Fixture,
    prompts: Arc<dyn WorkerPrompts>,
) -> Arc<RunDriver> {
    let tracker: Arc<dyn IssueTrackerAdapter> = Arc::new(LocalTracker::new(fx.repo.join("issues")));
    let promotion = Arc::new(PromotionService::new(
        services.clone(),
        git_adapter.clone(),
        None,
    ));
    RunDriver::new(
        services.clone(),
        tracker,
        None,
        git_adapter.clone(),
        agent,
        runner(),
        prompts,
        promotion,
    )
}

/// The real Git adapter confined to the fixture's worktree root.
pub fn git_adapter(fx: &Fixture) -> Arc<GitCli> {
    let data = std::fs::canonicalize(&fx.data).unwrap();
    let data = PathBuf::from(
        data.to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_owned(),
    );
    Arc::new(GitCli::new(runner(), baseline_env()).with_managed_root(data.join("worktrees")))
}

/// The machine-wide lock for the resource key `RK-AGY-REAL-ACCOUNT`: two real-account runs at once would confound each other (and
/// share one quota), so the second waits. A lock left by a crashed run is taken over after thirty minutes.
pub struct RealAccountLock(PathBuf);

impl RealAccountLock {
    /// Waits up to ten minutes for the lock.
    pub fn acquire() -> Self {
        let path = std::env::temp_dir().join("vela-RK-AGY-REAL-ACCOUNT.lock");
        let started = std::time::Instant::now();
        loop {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut f) => {
                    use std::io::Write;
                    let _ = write!(f, "{}", std::process::id());
                    return Self(path);
                }
                Err(_) => {
                    let stale = std::fs::metadata(&path)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| t.elapsed().ok())
                        .is_some_and(|age| age > std::time::Duration::from_secs(30 * 60));
                    if stale {
                        let _ = std::fs::remove_file(&path);
                    } else if started.elapsed() > std::time::Duration::from_secs(600) {
                        panic!(
                            "another real-account run holds RK-AGY-REAL-ACCOUNT ({})",
                            path.display()
                        );
                    } else {
                        std::thread::sleep(std::time::Duration::from_secs(2));
                    }
                }
            }
        }
    }
}

impl Drop for RealAccountLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// The installed Antigravity version, for stamping a result.
pub fn agy_version() -> String {
    Command::new("agy")
        .arg("--version")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

/// Appends one version-stamped line to `target/real-suite-results.jsonl` (outside the repository's tracked files).
pub fn record_result(name: &str, passed: bool, seconds: u64) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target");
    let _ = std::fs::create_dir_all(&dir);
    let line = serde_json::json!({
        "test": name,
        "passed": passed,
        "seconds": seconds,
        "agy_version": agy_version(),
        "vela": env!("CARGO_PKG_VERSION"),
    });
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("real-suite-results.jsonl"))
    {
        use std::io::Write;
        let _ = writeln!(f, "{line}");
    }
}
