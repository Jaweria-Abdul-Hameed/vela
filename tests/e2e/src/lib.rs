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
    /// Keep the folder when dropped (a crash-matrix child leaves its state for the restart).
    keep: bool,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if !self.keep {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }
}

impl Fixture {
    /// Creates the repository with the issues given as `(file stem, body)` pairs. `blocked_by` lines go in the body.
    pub fn new(tag: &str, issues: &[(&str, &str)]) -> Self {
        let base = std::env::temp_dir().join(format!("vela-e2e-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        Self::build(base, issues, false)
    }

    /// The fixture rooted at `base`, which survives being dropped: created on the first launch of a crash-matrix child and
    /// reopened, untouched, by the restart.
    pub fn at(base: &Path, issues: &[(&str, &str)]) -> Self {
        if base.join("repo").join(".git").exists() {
            return Self {
                base: base.to_path_buf(),
                repo: base.join("repo"),
                data: base.join("data"),
                keep: true,
            };
        }
        Self::build(base.to_path_buf(), issues, true)
    }

    fn build(base: PathBuf, issues: &[(&str, &str)], keep: bool) -> Self {
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
        Self {
            base,
            repo,
            data,
            keep,
        }
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
    driver_with(services, git_adapter, agent, fx, prompts, None)
}

/// [`driver`] with an analyst, so a scenario can say which tickets write which files.
pub fn driver_with(
    services: &Arc<Services>,
    git_adapter: &Arc<GitCli>,
    agent: Arc<dyn AgentSession>,
    fx: &Fixture,
    prompts: Arc<dyn WorkerPrompts>,
    analyst: Option<Arc<dyn vela_core::analysis::AnalystHook>>,
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
        analyst,
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

/// An agent that does what the ticket asks by writing one file named after the ticket into the worktree.
pub struct FileAgent;

impl vela_domain::ports::AgentSession for FileAgent {
    fn run_turn<'a>(
        &'a self,
        request: &'a vela_domain::ports::TurnRequest,
        sink: &'a dyn vela_domain::ports::AgentEventSink,
    ) -> vela_domain::ports::PortFuture<'a, vela_domain::ports::TurnResult> {
        Box::pin(async move {
            let ticket = request
                .prompt
                .split_whitespace()
                .nth(1)
                .unwrap_or("x")
                .trim_end_matches(':')
                .to_ascii_lowercase();
            std::fs::write(
                Path::new(request.worktree.as_str()).join(format!("{ticket}.txt")),
                format!(
                    "done by Vela for {ticket}
"
                ),
            )
            .map_err(|e| VelaError::internal(e.to_string()))?;
            sink.on_event(vela_domain::ports::AgentEvent::TurnFinished);
            Ok(vela_domain::ports::TurnResult {
                conversation: vela_domain::ports::ConversationId::new("conv-e2e").ok(),
                finished: true,
                response: "DONE".to_owned(),
            })
        })
    }
    fn cancel<'a>(
        &'a self,
        _s: &'a vela_domain::ports::SessionRef,
    ) -> vela_domain::ports::PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
    fn status<'a>(
        &'a self,
        _s: &'a vela_domain::ports::SessionRef,
    ) -> vela_domain::ports::PortFuture<'a, vela_domain::ports::SessionStatus> {
        Box::pin(async { Ok(vela_domain::ports::SessionStatus::Idle) })
    }
}

/// What a scripted ticket does when its worker's turn runs.
#[derive(Debug, Clone)]
pub enum Behavior {
    /// Writes `<ticket>.txt`.
    Write,
    /// Writes `shared.txt` with this content (two tickets doing this conflict when merged).
    WriteShared(String),
    /// Reports it cannot proceed (an ambiguous or contradictory ticket).
    Blocked,
    /// The first turn reports exhausted capacity (with this reset time); later turns succeed.
    CapacityOnce(Option<i64>),
    /// Every turn reports a provider policy block.
    PolicyBlock,
    /// Writes its file after this many milliseconds, so concurrency is observable.
    Slow(u64),
}

/// A scripted agent with a behaviour per ticket, that records how many turns ran at once.
pub struct ScenarioAgent {
    by_ticket: std::sync::Mutex<std::collections::HashMap<String, Behavior>>,
    used_once: std::sync::Mutex<std::collections::HashSet<String>>,
    current: std::sync::atomic::AtomicU32,
    max: std::sync::atomic::AtomicU32,
    turns: std::sync::Mutex<Vec<String>>,
}

impl Default for ScenarioAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl ScenarioAgent {
    /// An agent where every ticket writes its file.
    pub fn new() -> Self {
        Self {
            by_ticket: std::sync::Mutex::new(std::collections::HashMap::new()),
            used_once: std::sync::Mutex::new(std::collections::HashSet::new()),
            current: std::sync::atomic::AtomicU32::new(0),
            max: std::sync::atomic::AtomicU32::new(0),
            turns: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Sets what `ticket` (for example `T-1`) does.
    pub fn set(&self, ticket: &str, behavior: Behavior) -> &Self {
        self.by_ticket
            .lock()
            .unwrap()
            .insert(ticket.to_ascii_lowercase(), behavior);
        self
    }

    /// The most turns that ran at the same moment.
    pub fn max_concurrent(&self) -> u32 {
        self.max.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// The tickets whose turns ran, in order.
    pub fn turns(&self) -> Vec<String> {
        self.turns.lock().unwrap().clone()
    }
}

impl vela_domain::ports::AgentSession for ScenarioAgent {
    fn run_turn<'a>(
        &'a self,
        request: &'a vela_domain::ports::TurnRequest,
        sink: &'a dyn vela_domain::ports::AgentEventSink,
    ) -> vela_domain::ports::PortFuture<'a, vela_domain::ports::TurnResult> {
        use std::sync::atomic::Ordering;
        use vela_domain::errors::{ErrorClass, ErrorCode};
        use vela_domain::ports::{AgentEvent, ConversationId, TurnResult};
        Box::pin(async move {
            let ticket = request
                .prompt
                .split_whitespace()
                .nth(1)
                .unwrap_or("x")
                .trim_end_matches(':')
                .to_ascii_lowercase();
            self.turns.lock().unwrap().push(ticket.clone());
            let now = self.current.fetch_add(1, Ordering::SeqCst) + 1;
            self.max.fetch_max(now, Ordering::SeqCst);
            let behavior = self
                .by_ticket
                .lock()
                .unwrap()
                .get(&ticket)
                .cloned()
                .unwrap_or(Behavior::Write);
            let result = async {
                let dir = Path::new(request.worktree.as_str());
                let finished = |response: &str| TurnResult {
                    conversation: ConversationId::new(format!("conv-{ticket}")).ok(),
                    finished: true,
                    response: response.to_owned(),
                };
                match behavior {
                    Behavior::Write | Behavior::Slow(_) | Behavior::CapacityOnce(_) => {
                        if let Behavior::Slow(ms) = behavior {
                            tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
                        }
                        if let Behavior::CapacityOnce(reset) = behavior
                            && self.used_once.lock().unwrap().insert(ticket.clone())
                        {
                            sink.on_event(AgentEvent::CapacityError {
                                message: "RESOURCE_EXHAUSTED: capacity exhausted".into(),
                                reset_after_ms: reset,
                            });
                            return Err(VelaError::of_class(
                                ErrorClass::ExternalTemporary,
                                "the provider's capacity is exhausted",
                            ));
                        }
                        std::fs::write(
                            dir.join(format!("{ticket}.txt")),
                            format!("done by Vela for {ticket}\n"),
                        )
                        .map_err(|e| VelaError::internal(e.to_string()))?;
                        sink.on_event(AgentEvent::TurnFinished);
                        Ok(finished("DONE"))
                    }
                    Behavior::WriteShared(content) => {
                        std::fs::write(dir.join("shared.txt"), content)
                            .map_err(|e| VelaError::internal(e.to_string()))?;
                        sink.on_event(AgentEvent::TurnFinished);
                        Ok(finished("DONE"))
                    }
                    Behavior::Blocked => {
                        sink.on_event(AgentEvent::TurnFinished);
                        Ok(finished("BLOCKED"))
                    }
                    Behavior::PolicyBlock => {
                        sink.on_event(AgentEvent::ProviderPolicyBlock {
                            evidence: "SYNTHETIC: the provider blocked the account".into(),
                        });
                        Err(VelaError::from_code(
                            ErrorCode::ProviderPolicyBlock,
                            "the provider blocked the account or service",
                        ))
                    }
                }
            }
            .await;
            self.current.fetch_sub(1, Ordering::SeqCst);
            result
        })
    }
    fn cancel<'a>(
        &'a self,
        _s: &'a vela_domain::ports::SessionRef,
    ) -> vela_domain::ports::PortFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
    fn status<'a>(
        &'a self,
        _s: &'a vela_domain::ports::SessionRef,
    ) -> vela_domain::ports::PortFuture<'a, vela_domain::ports::SessionStatus> {
        Box::pin(async { Ok(vela_domain::ports::SessionStatus::Idle) })
    }
}

/// An analyst that says exactly which files each ticket writes (and proposes nothing else).
pub struct ScriptedAnalyst(pub std::collections::BTreeMap<String, Vec<String>>);

impl vela_core::analysis::AnalystHook for ScriptedAnalyst {
    fn analyze<'a>(
        &'a self,
        _tickets: &'a [vela_domain::ports::TrackerTicket],
        _graph: &'a vela_domain::graph::DependencyGraph,
    ) -> vela_domain::ports::PortFuture<'a, vela_core::analysis::AnalystOutput> {
        let mut out = vela_core::analysis::AnalystOutput::default();
        for (ticket, files) in &self.0 {
            let Ok(id) = vela_domain::ids::TicketId::new(ticket.clone()) else {
                continue;
            };
            out.write_sets.insert(
                id,
                vela_domain::parallel::PredictedWriteSet {
                    files: files.iter().cloned().collect(),
                    ..Default::default()
                },
            );
        }
        out.version = "scripted".to_owned();
        Box::pin(async move { Ok(out) })
    }
}
