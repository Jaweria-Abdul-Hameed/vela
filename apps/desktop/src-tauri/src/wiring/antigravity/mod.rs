//! Antigravity wiring (W01): the agent the worker driver talks to, and its prompts.
//!
//! Each worker gets an **isolated profile** (its own home with the allow rules, the deny hook and the rule table) before its first
//! turn, so headless runs are governed by Vela's policy and never by the person's own Antigravity settings or prompts.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use vela_adapters::antigravity::prompts::{
    CompletionStatus as AgyStatus, TicketSnapshot, WorkerEnvelope, correction_prompt,
    parse_completion, render_worker_prompt,
};
use vela_adapters::antigravity::session::AgyDriver;
use vela_core::orchestrator::Services;
use vela_core::trust::TrustService;
use vela_core::worker::{Completion, CompletionStatus, WorkerPrompts, WorkerTask};
use vela_domain::errors::VelaError;
use vela_domain::ids::{WorkerId, WorktreePath};
use vela_domain::policy::{PolicyEngine, RunMode};
use vela_domain::ports::{
    AgentEventSink, AgentSession, PortFuture, SessionRef, SessionStatus, TurnRequest, TurnResult,
};

use crate::wiring::adapters::Adapters;

/// The agent: creates the worker's isolated profile once, then drives `agy` with it.
pub struct ProfilingAgent {
    services: Arc<Services>,
    adapters: Arc<Adapters>,
    ready: Mutex<HashSet<WorkerId>>,
}

impl ProfilingAgent {
    /// An agent over the services and the concrete adapters.
    pub fn new(services: Arc<Services>, adapters: Arc<Adapters>) -> Self {
        Self {
            services,
            adapters,
            ready: Mutex::new(HashSet::new()),
        }
    }

    fn driver(&self) -> &Arc<AgyDriver> {
        &self.adapters.agy
    }

    fn ensure_profile(&self, worker: &WorkerId, worktree: &WorktreePath) -> Result<(), VelaError> {
        if self.ready.lock().expect("ready lock").contains(worker) {
            return Ok(());
        }
        let w = worker.as_str().to_owned();
        let (run_id, project_id): (String, String) = self.services.store.read(|c| {
            c.query_row(
                "SELECT w.run_id, r.project_id FROM workers w JOIN build_runs r ON r.id = w.run_id WHERE w.id = ?1",
                [&w],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| VelaError::internal(format!("the worker's run is not recorded: {e}")))
        })?;
        let context = TrustService::new(&self.services).policy_context(
            &project_id,
            worktree.as_str(),
            RunMode::Supervised,
        )?;
        let profile =
            self.adapters
                .profiles
                .create(&run_id, worker, &PolicyEngine::new(context))?;
        self.driver().set_worker_env(worker.clone(), profile.env());
        self.ready
            .lock()
            .expect("ready lock")
            .insert(worker.clone());
        Ok(())
    }
}

impl AgentSession for ProfilingAgent {
    fn run_turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        sink: &'a dyn AgentEventSink,
    ) -> PortFuture<'a, TurnResult> {
        Box::pin(async move {
            self.ensure_profile(&request.worker, &request.worktree)?;
            self.driver().run_turn(request, sink).await
        })
    }

    fn cancel<'a>(&'a self, session: &'a SessionRef) -> PortFuture<'a, ()> {
        self.driver().cancel(session)
    }

    fn status<'a>(&'a self, session: &'a SessionRef) -> PortFuture<'a, SessionStatus> {
        self.driver().status(session)
    }
}

/// What the hook asked about, and one-time approvals, for Antigravity workers (ADR-018 section 4). The executable comes from the
/// hook's own spool records; a granted approval is an exact native rule with an expiry in that worker's isolated profile.
pub struct AgyAsked {
    services: Arc<Services>,
    adapters: Arc<Adapters>,
    readers:
        Mutex<std::collections::HashMap<WorkerId, vela_adapters::antigravity::ask::SpoolReader>>,
}

impl AgyAsked {
    /// Over the services and the concrete adapters.
    pub fn new(services: Arc<Services>, adapters: Arc<Adapters>) -> Self {
        Self {
            services,
            adapters,
            readers: Mutex::new(std::collections::HashMap::new()),
        }
    }

    fn profile_of(
        &self,
        worker: &WorkerId,
    ) -> Result<vela_adapters::antigravity::profile::WorkerProfile, VelaError> {
        let w = worker.as_str().to_owned();
        let run: String = self.services.store.read(|c| {
            c.query_row("SELECT run_id FROM workers WHERE id = ?1", [&w], |row| {
                row.get(0)
            })
            .map_err(|e| VelaError::internal(format!("the worker is not recorded: {e}")))
        })?;
        self.adapters.profiles.locate(&run, worker)
    }
}

impl vela_core::worker::AskedCommands for AgyAsked {
    fn take_asked(&self, worker: &WorkerId) -> Vec<String> {
        let Ok(profile) = self.profile_of(worker) else {
            return Vec::new();
        };
        let mut readers = self.readers.lock().expect("readers lock");
        let reader = readers.entry(worker.clone()).or_insert_with(|| {
            vela_adapters::antigravity::ask::SpoolReader::new(profile.spool_dir)
        });
        reader
            .poll()
            .unwrap_or_default()
            .into_iter()
            .filter(|e| e.decision == "ask")
            .filter_map(|e| e.executable)
            .collect()
    }

    fn grant_once(
        &self,
        worker: &WorkerId,
        executable: &str,
        now_ms: i64,
    ) -> Result<(), VelaError> {
        let profile = self.profile_of(worker)?;
        let rule = vela_adapters::antigravity::ask::once_rule(
            executable,
            now_ms,
            vela_core::worker::asked::ALLOW_ONCE_TTL_MS,
        )
        .ok_or_else(|| {
            VelaError::of_class(
                vela_domain::errors::ErrorClass::ValidationFailure,
                "that executable name cannot be turned into a safe rule",
            )
        })?;
        let ledger = profile.settings_path.with_file_name("vela-once-rules.json");
        vela_adapters::antigravity::ask::install_once_rule(&profile.settings_path, &ledger, &rule)
    }
}

/// The worker prompts, from the Antigravity prompt templates.
pub struct AgyPrompts;

const POLICY_SUMMARY: &str = "commands in the project's confirmed command profile run without asking; everything else is asked about or refused";

impl WorkerPrompts for AgyPrompts {
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
            policy_summary: POLICY_SUMMARY.to_owned(),
            skills: Vec::new(),
        })
    }

    fn fix_prompt(&self, failures: &str) -> String {
        format!(
            "The checks failed:\n{failures}\nFind the cause in your worktree and fix it. Then end with the vela-completion block, as before."
        )
    }

    fn correction_prompt(&self, problem: &str) -> String {
        correction_prompt(problem)
    }

    fn parse_completion(&self, response: &str, base: &str) -> Result<Completion, VelaError> {
        let report = parse_completion(response, base)?;
        Ok(Completion {
            status: match report.status {
                AgyStatus::Complete => CompletionStatus::Complete,
                AgyStatus::Blocked => CompletionStatus::Blocked,
                AgyStatus::Failed => CompletionStatus::Failed,
            },
            warnings: report.warnings,
            discovered_dependencies: report.discovered_dependencies,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_blocks_are_translated_to_the_cores_statuses() {
        let base = "a".repeat(40);
        let head = "b".repeat(40);
        let reply = format!(
            "done\n```vela-completion\n{{\"status\":\"blocked\",\"base_sha\":\"{base}\",\"head_sha\":\"{head}\",\"warnings\":[\"x\"]}}\n```\n"
        );
        let c = AgyPrompts.parse_completion(&reply, &base).unwrap();
        assert_eq!(c.status, CompletionStatus::Blocked);
        assert_eq!(c.warnings, ["x"]);
        assert!(AgyPrompts.parse_completion("no block", &base).is_err());
        assert!(AgyPrompts.fix_prompt("boom").contains("boom"));
    }
}
