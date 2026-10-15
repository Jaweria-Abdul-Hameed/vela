//! Shared by the real-account tests: the desktop's agent, prompts and approval source, restated here because the Tauri crate cannot be
//! linked into a test.
#![allow(dead_code)]

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use vela_adapters::antigravity::ask::{SpoolReader, asked_commands, grant_rule, install_once_rule};
use vela_adapters::antigravity::profile::ProfileManager;
use vela_adapters::antigravity::prompts::{
    CompletionStatus as AgyStatus, TicketSnapshot, WorkerEnvelope, correction_prompt,
    parse_completion, render_worker_prompt,
};
use vela_adapters::antigravity::session::AgyDriver;
use vela_core::orchestrator::Services;
use vela_core::trust::TrustService;
use vela_core::worker::{AskedCommands, Completion, CompletionStatus, WorkerPrompts, WorkerTask};
use vela_domain::errors::VelaError;
use vela_domain::ids::WorkerId;
use vela_domain::policy::{PolicyEngine, RunMode};
use vela_domain::ports::{
    AgentEventSink, AgentSession, PortFuture, SessionRef, SessionStatus, TurnRequest, TurnResult,
};

/// The desktop's `ProfilingAgent`, restated for the test (the composition root lives in the Tauri crate, which tests cannot link).
pub struct RealAgent {
    pub services: Arc<Services>,
    pub agy: Arc<AgyDriver>,
    pub profiles: Arc<ProfileManager>,
    pub ready: Mutex<HashSet<WorkerId>>,
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
pub struct Prompts;

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

pub fn hook_exe() -> PathBuf {
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

/// The desktop's `AgyAsked`, restated for the test.
pub struct RealAsked {
    pub services: Arc<Services>,
    pub profiles: Arc<ProfileManager>,
    pub agy: Arc<AgyDriver>,
    pub readers: Mutex<std::collections::HashMap<WorkerId, SpoolReader>>,
}

impl RealAsked {
    fn profile_of(
        &self,
        worker: &WorkerId,
    ) -> Result<vela_adapters::antigravity::profile::WorkerProfile, VelaError> {
        let w = worker.as_str().to_owned();
        let run: String = self.services.store.read(|c| {
            c.query_row("SELECT run_id FROM workers WHERE id = ?1", [&w], |r| {
                r.get(0)
            })
            .map_err(|e| VelaError::internal(e.to_string()))
        })?;
        self.profiles.locate(&run, worker)
    }
}

impl AskedCommands for RealAsked {
    fn take_asked(&self, worker: &WorkerId) -> Vec<String> {
        let Ok(profile) = self.profile_of(worker) else {
            return Vec::new();
        };
        let mut readers = self.readers.lock().unwrap();
        let entries = readers
            .entry(worker.clone())
            .or_insert_with(|| SpoolReader::new(profile.spool_dir))
            .poll()
            .unwrap_or_default();
        let steps = self
            .agy
            .last_outcome(worker)
            .map(|o| o.steps)
            .unwrap_or_default();
        asked_commands(&entries, &steps)
    }

    fn grant_once(
        &self,
        worker: &WorkerId,
        executable: &str,
        now_ms: i64,
    ) -> Result<(), VelaError> {
        let profile = self.profile_of(worker)?;
        let rule = grant_rule(
            executable,
            now_ms,
            vela_core::worker::asked::ALLOW_ONCE_TTL_MS,
        )
        .ok_or_else(|| VelaError::internal("not a safe executable name"))?;
        let ledger = profile.table_path.with_file_name("vela-once-rules.json");
        install_once_rule(&profile.settings_path, &ledger, &rule)
    }
}

/// What happened, for a failing real run: the journal and what the agent last said and did.
pub fn diagnose(services: &Arc<Services>, agy: &Arc<AgyDriver>) -> String {
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
                    r.get::<_, String>(1)?.chars().take(300).collect::<String>()
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
    let mut view = String::new();
    for w in workers {
        if let Some(o) = agy.last_outcome(&WorkerId::new(w.clone()).unwrap()) {
            view.push_str(&format!(
                "worker {w}: class {:?}\nresponse: {}\nsteps: {:?}\ndenied: {:?}\nstderr: {}\n",
                o.class, o.response, o.steps, o.denied, o.stderr_excerpt
            ));
        }
    }
    format!("{view}\n{}", journal.join("\n"))
}

/// Every hook record under `profiles`, for a failing run.
pub fn spool_dump(profiles: &std::path::Path) -> String {
    fn walk(dir: &std::path::Path, out: &mut String) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "jsonl") {
                out.push_str(&format!("-- {}\n", p.display()));
                out.push_str(&std::fs::read_to_string(&p).unwrap_or_default());
            }
        }
    }
    let mut s = String::new();
    walk(profiles, &mut s);
    s
}
