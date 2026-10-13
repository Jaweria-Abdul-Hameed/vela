//! The application API: everything the user interface asks of the core, as plain methods returning JSON-ready values.
//!
//! The desktop shell's commands call these and do nothing else, so the behaviour of every command is tested here without a window.
//! What needs concrete adapters (Git, the agent, trackers, the process runner) comes from a [`Backend`] the composition root
//! provides.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tokio::task::JoinHandle;
use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::ids::{BranchName, RunId, TicketId, WorkerId, WorktreePath};
use vela_domain::ports::{GitRead, RemoteName};
use vela_domain::state::RunState;
use vela_persistence::repositories::{o01_workers, s08_build, s15_interventions, t04_promotions};

use super::Services;
use super::driver::{DriveOutcome, RunConfig, RunDriver};
use super::runs::Runs;
use crate::analysis::{Analysis, stored_graph};
use crate::intervention::Interventions;
use crate::preflight::{PreflightEngine, PreflightFacts, collect_local};
use crate::project::ProjectService;
use crate::recovery::{Recovery, RecoveryInput};
use crate::settings::{SettingsService, Source};
use crate::stop::StopService;
use crate::trust::{ProfileCommand, TrustService, propose};

/// The project-setting key holding the issue source.
pub const TRACKER_KEY: &str = "tracker";

/// What the API needs to know about a project to build things for it.
#[derive(Debug, Clone)]
pub struct ProjectContext {
    /// The project.
    pub id: String,
    /// The repository folder.
    pub repo_path: String,
    /// The branch checked out when it was imported.
    pub default_branch: Option<String>,
}

/// The concrete pieces behind the API.
pub trait Backend: Send + Sync {
    /// Git reads (project import).
    fn git_read(&self) -> Arc<dyn GitRead>;
    /// The driver for a project (it owns that project's tracker).
    fn driver(&self, project: &ProjectContext) -> Result<Arc<RunDriver>, VelaError>;
    /// Stop All.
    fn stop_service(&self, driver: &Arc<RunDriver>) -> Arc<StopService>;
    /// Reconciliation.
    fn recovery(&self) -> Arc<Recovery>;
    /// Where worktrees go for a project.
    fn worktree_root(&self, project_id: &str) -> String;
    /// The remote to push to, when the repository has one.
    fn remote_for(&self, project: &ProjectContext) -> Option<RemoteName>;
    /// Drops a cached driver (its tracker changed).
    fn forget_driver(&self, _project_id: &str) {}
    /// Fills the platform facts preflight needs (Git version, GitHub CLI, WebView2, free disk). The default adds nothing.
    fn fill_facts(&self, _facts: &mut PreflightFacts) {}
}

/// The API.
pub struct AppApi {
    services: Arc<Services>,
    backend: Arc<dyn Backend>,
    drives: Mutex<HashMap<String, JoinHandle<()>>>,
}

fn not_found(what: &str) -> VelaError {
    VelaError::of_class(
        ErrorClass::ValidationFailure,
        format!("there is no such {what}"),
    )
}

impl AppApi {
    /// An API over the shared services and the concrete backend.
    pub fn new(services: Arc<Services>, backend: Arc<dyn Backend>) -> Arc<Self> {
        Arc::new(Self {
            services,
            backend,
            drives: Mutex::new(HashMap::new()),
        })
    }

    fn project(&self, id: &str) -> Result<ProjectContext, VelaError> {
        let id = id.to_owned();
        let row = self
            .services
            .store
            .read(|c| vela_persistence::repositories::s01_projects::get(c, &id))?
            .ok_or_else(|| not_found("project"))?;
        Ok(ProjectContext {
            id: row.id,
            repo_path: row.repo_path,
            default_branch: row.default_branch,
        })
    }

    // ---- projects ----

    /// Imports a repository folder.
    pub async fn project_import(&self, folder: &str) -> Result<Value, VelaError> {
        let svc = ProjectService::new(self.services.clone(), self.backend.git_read());
        let result = svc.import(folder).await?;
        Ok(
            json!({ "project": project_json(&result.project), "already_known": result.already_known }),
        )
    }

    /// Recent projects.
    pub fn project_list(&self) -> Result<Value, VelaError> {
        let svc = ProjectService::new(self.services.clone(), self.backend.git_read());
        Ok(Value::Array(svc.list()?.iter().map(project_json).collect()))
    }

    /// Removes a project from Vela (never touches the repository).
    pub fn project_remove(&self, id: &str) -> Result<(), VelaError> {
        ProjectService::new(self.services.clone(), self.backend.git_read()).remove(id)
    }

    /// Chooses where a project's issues live: `local:<folder inside the repository>` or `github:<owner>/<name>`.
    pub fn project_set_tracker(&self, project_id: &str, spec: &str) -> Result<(), VelaError> {
        self.project(project_id)?;
        let local_ok = |d: &str| {
            let d = d.trim();
            !d.is_empty()
                && !d.starts_with(['/', '\\'])
                && !d.contains(':')
                && !d.split(['/', '\\']).any(|c| c == "..")
        };
        let github_ok = |r: &str| {
            let mut parts = r.split('/');
            let (Some(o), Some(n), None) = (parts.next(), parts.next(), parts.next()) else {
                return false;
            };
            [o, n].iter().all(|p| {
                !p.is_empty()
                    && !p.starts_with(['-', '.'])
                    && p.chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            })
        };
        let ok = spec.strip_prefix("local:").is_some_and(local_ok)
            || spec.strip_prefix("github:").is_some_and(github_ok);
        if !ok {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "the issue source must be `local:<folder in the repository>` or `github:<owner>/<name>`",
            ));
        }
        let (p, s, now) = (
            project_id.to_owned(),
            spec.trim().to_owned(),
            self.services.now_ms(),
        );
        self.services.store.write(move |tx| {
            vela_persistence::repositories::s02_settings::set_project_setting(
                tx.conn(),
                &p,
                TRACKER_KEY,
                &s,
                now,
            )
        })?;
        self.backend.forget_driver(project_id);
        Ok(())
    }

    // ---- settings ----

    /// Effective settings, for a project or globally.
    pub fn settings_get(&self, project_id: Option<&str>) -> Result<Value, VelaError> {
        Ok(json!(
            SettingsService::new(&self.services).effective(project_id)?
        ))
    }

    /// Sets one setting (user-wide, or for a project when `project_id` is given).
    pub fn settings_set(
        &self,
        project_id: Option<&str>,
        key: &str,
        value: &str,
    ) -> Result<(), VelaError> {
        let s = SettingsService::new(&self.services);
        match project_id {
            Some(p) => s.set_project(Source::User, p, key, value),
            None => s.set_user(Source::User, key, value),
        }
    }

    // ---- trust ----

    /// What trusting a repository would make executable, read from its manifests as data.
    pub fn trust_propose(&self, project_id: &str) -> Result<Value, VelaError> {
        let p = self.project(project_id)?;
        let proposal = propose(std::path::Path::new(&p.repo_path));
        let state = TrustService::new(&self.services).state(project_id)?;
        Ok(json!({
            "state": format!("{state:?}"),
            "commands": proposal.commands.iter().map(|c| json!({"text": c.text, "cwd": c.cwd})).collect::<Vec<_>>(),
            "executable_surfaces": proposal.executable_surfaces,
            "consent_text_version": crate::trust::TRUST_CONSENT_VERSION,
        }))
    }

    /// The user trusts the repository and confirms the command profile.
    pub fn trust_grant(
        &self,
        project_id: &str,
        commands: &[ProfileCommand],
    ) -> Result<(), VelaError> {
        let p = self.project(project_id)?;
        TrustService::new(&self.services).trust(
            project_id,
            &p.repo_path,
            commands,
            crate::trust::TRUST_CONSENT_VERSION,
        )
    }

    /// Revokes trust.
    pub fn trust_revoke(&self, project_id: &str) -> Result<(), VelaError> {
        TrustService::new(&self.services).revoke(project_id)
    }

    // ---- preflight ----

    /// Runs preflight for a project and stores the report.
    pub fn preflight_run(&self, project_id: &str) -> Result<Value, VelaError> {
        let p = self.project(project_id)?;
        let mut facts = collect_local(std::path::Path::new(&p.repo_path));
        facts.trust = TrustService::new(&self.services).state(project_id)?;
        facts.has_remote = self.backend.remote_for(&p).is_some();
        facts.default_branch = p.default_branch.clone();
        self.backend.fill_facts(&mut facts);
        let report = PreflightEngine::new(self.services.clone()).run(project_id, &facts)?;
        serde_json::to_value(&report).map_err(|e| VelaError::internal(e.to_string()))
    }

    // ---- runs ----

    fn config(&self, p: &ProjectContext) -> Result<RunConfig, VelaError> {
        let trust = TrustService::new(&self.services);
        let settings = SettingsService::new(&self.services).effective(Some(&p.id))?;
        let bad = |e: vela_domain::ids::IdError| {
            VelaError::of_class(ErrorClass::ValidationFailure, e.to_string())
        };
        Ok(RunConfig {
            project_id: p.id.clone(),
            repo: WorktreePath::new(p.repo_path.replace('\\', "/")).map_err(bad)?,
            repo_root: PathBuf::from(&p.repo_path),
            base_branch: BranchName::new(p.default_branch.clone().unwrap_or_else(|| "main".to_owned())).map_err(bad)?,
            worktree_root: self.backend.worktree_root(&p.id),
            validation: trust.profile_commands(&p.id)?,
            remote: self.backend.remote_for(p),
            title: format!("Vela: {}", std::path::Path::new(&p.repo_path).file_name().map_or("project".into(), |n| n.to_string_lossy())),
            body: "Built by Vela. Every ticket was implemented in its own worktree, checked, and merged by Vela.".to_owned(),
            spec_pointers: ["AGENTS.md", "CLAUDE.md"].iter().map(|s| (*s).to_owned()).collect(),
            max_fix_attempts: settings.get("integration_fix_attempts").and_then(|v| v.parse().ok()).unwrap_or(3),
        })
    }

    /// Creates a run and analyzes the issues into a graph. Returns the run and what the user needs to approve it.
    pub async fn run_prepare(&self, project_id: &str) -> Result<Value, VelaError> {
        let p = self.project(project_id)?;
        let driver = self.backend.driver(&p)?;
        let cfg = self.config(&p)?;
        let (run, analysis) = driver.prepare(&cfg).await?;
        // The settings this run will use are fixed now.
        SettingsService::new(&self.services).capture_run_snapshot(&run, Some(project_id))?;
        Ok(match analysis {
            Analysis::Ready {
                snapshot,
                warnings,
                state,
                ..
            } => json!({
                "run_id": run.as_str(),
                "valid": true,
                "tickets": snapshot.tickets.len(),
                "edges": snapshot.edges.len(),
                "layers": snapshot.layers.len(),
                "analyzed": matches!(state, crate::analysis::AnalysisState::Analyzed),
                "warnings": warnings,
            }),
            Analysis::Invalid {
                cycles,
                intervention_id,
            } => json!({
                "run_id": run.as_str(),
                "valid": false,
                "cycles": cycles,
                "intervention_id": intervention_id,
            }),
        })
    }

    /// The user approved the graph and pressed Build: starts the run and drives it in the background.
    pub async fn run_build(
        self: &Arc<Self>,
        run_id: &str,
        expected_base: Option<&str>,
        acknowledge: bool,
    ) -> Result<(), VelaError> {
        let run = RunId::new(run_id)
            .map_err(|e| VelaError::of_class(ErrorClass::ValidationFailure, e.to_string()))?;
        let project_id = self.project_of_run(&run)?;
        let p = self.project(&project_id)?;
        let driver = self.backend.driver(&p)?;
        let cfg = self.config(&p)?;
        let base = expected_base
            .map(|s| {
                vela_domain::ids::CommitSha::new(s)
                    .map_err(|e| VelaError::of_class(ErrorClass::ValidationFailure, e.to_string()))
            })
            .transpose()?;
        driver
            .approve_and_start(&run, &cfg, base, acknowledge)
            .await?;
        self.spawn_drive(run, driver, cfg);
        Ok(())
    }

    fn spawn_drive(self: &Arc<Self>, run: RunId, driver: Arc<RunDriver>, cfg: RunConfig) {
        let key = run.as_str().to_owned();
        let handle = tokio::spawn(async move {
            match driver.drive(&run, &cfg).await {
                Ok(
                    DriveOutcome::Completed(_)
                    | DriveOutcome::Waiting(_)
                    | DriveOutcome::FinalNeedsHuman { .. },
                ) => {}
                Err(e) => {
                    tracing::warn!(run = %run, error = %e.what_failed(), "the run driver stopped")
                }
            }
        });
        self.drives.lock().expect("drives lock").insert(key, handle);
    }

    fn project_of_run(&self, run: &RunId) -> Result<String, VelaError> {
        let r = run.as_str().to_owned();
        self.services.store.read(|c| {
            c.query_row(
                "SELECT project_id FROM build_runs WHERE id = ?1",
                [&r],
                |row| row.get::<_, String>(0),
            )
            .map_err(|_| not_found("run"))
        })
    }

    /// The runs that are running right now.
    pub fn active_runs(&self) -> Result<Vec<String>, VelaError> {
        let rows: Vec<(String, String)> = self.services.store.read(|c| {
            let mut stmt = c
                .prepare("SELECT id, state FROM build_runs ORDER BY created_ms")
                .map_err(|e| VelaError::internal(e.to_string()))?;
            let rows = stmt
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
                .map_err(|e| VelaError::internal(e.to_string()))?;
            Ok(rows.filter_map(Result::ok).collect())
        })?;
        Ok(rows
            .into_iter()
            .filter(|(_, s)| {
                serde_json::from_str::<RunState>(s)
                    .is_ok_and(|s| s == RunState::Active(vela_domain::state::RunPhase::Running))
            })
            .map(|(id, _)| id)
            .collect())
    }

    /// Stop All for every running run (the tray, and quitting). Returns how many runs were stopped; a run that cannot be stopped
    /// is reported in the error text after the others were still attempted.
    pub async fn stop_all_runs(&self) -> Result<usize, VelaError> {
        let mut stopped = 0;
        let mut failures = Vec::new();
        for run in self.active_runs()? {
            match self.run_stop(&run).await {
                Ok(_) => stopped += 1,
                Err(e) => failures.push(format!("{run}: {}", e.what_failed())),
            }
        }
        if failures.is_empty() {
            Ok(stopped)
        } else {
            Err(VelaError::of_class(
                ErrorClass::InternalBug,
                format!("some runs could not be stopped: {}", failures.join("; ")),
            ))
        }
    }

    /// Waits for a run's background driver to stop (tests and shutdown).
    pub async fn wait_for_drive(&self, run_id: &str) {
        let handle = self.drives.lock().expect("drives lock").remove(run_id);
        if let Some(h) = handle {
            let _ = h.await;
        }
    }

    /// Continues a run after an intervention was resolved (the driver resumes workers that have no session).
    pub async fn run_continue(self: &Arc<Self>, run_id: &str) -> Result<(), VelaError> {
        let run = RunId::new(run_id)
            .map_err(|e| VelaError::of_class(ErrorClass::ValidationFailure, e.to_string()))?;
        let p = self.project(&self.project_of_run(&run)?)?;
        let driver = self.backend.driver(&p)?;
        let cfg = self.config(&p)?;
        if self
            .drives
            .lock()
            .expect("drives lock")
            .get(run_id)
            .is_some_and(|h| !h.is_finished())
        {
            return Ok(());
        }
        self.spawn_drive(run, driver, cfg);
        Ok(())
    }

    /// Stop All.
    pub async fn run_stop(&self, run_id: &str) -> Result<Value, VelaError> {
        let run = RunId::new(run_id)
            .map_err(|e| VelaError::of_class(ErrorClass::ValidationFailure, e.to_string()))?;
        let p = self.project(&self.project_of_run(&run)?)?;
        let driver = self.backend.driver(&p)?;
        let r = run.as_str().to_owned();
        let base = self
            .services
            .store
            .read(|c| s08_build::get_run_base(c, &r))?
            .ok_or_else(|| {
                VelaError::of_class(ErrorClass::ValidationFailure, "the run has not started")
            })?;
        let integration = WorktreePath::new(base.integration_worktree)
            .map_err(|e| VelaError::internal(e.to_string()))?;
        if let Some(h) = self.drives.lock().expect("drives lock").remove(run_id) {
            h.abort();
        }
        let report = self
            .backend
            .stop_service(&driver)
            .stop_all(&run, &p.id, &integration)
            .await?;
        Ok(json!({
            "paused": report.paused.iter().map(WorkerId::as_str).collect::<Vec<_>>(),
            "forced": report.forced.iter().map(WorkerId::as_str).collect::<Vec<_>>(),
            "lane": format!("{:?}", report.lane),
        }))
    }

    /// Resume after Stop All or a restart: reconcile first, then continue only if nothing needs a decision.
    pub async fn run_resume(self: &Arc<Self>, run_id: &str) -> Result<Value, VelaError> {
        let run = RunId::new(run_id)
            .map_err(|e| VelaError::of_class(ErrorClass::ValidationFailure, e.to_string()))?;
        let p = self.project(&self.project_of_run(&run)?)?;
        let driver = self.backend.driver(&p)?;
        let cfg = self.config(&p)?;
        let report = self
            .backend
            .recovery()
            .reconcile(&RecoveryInput {
                run: run.clone(),
                project_id: p.id.clone(),
                repo: cfg.repo.clone(),
                reason: "resume".to_owned(),
            })
            .await?;
        let state = Runs::new(&self.services).run_state(&run)?;
        let resumed = if matches!(state, RunState::Paused { .. }) {
            match self
                .backend
                .stop_service(&driver)
                .resume(&run, &report)
                .await
            {
                Ok(w) => w.len(),
                Err(e) => {
                    return Ok(
                        json!({"resumed": false, "reason": e.what_failed(), "discrepancies": report.discrepancies.len(), "held": report.held.len()}),
                    );
                }
            }
        } else {
            0
        };
        self.spawn_drive(run, driver, cfg);
        Ok(
            json!({"resumed": true, "workers": resumed, "discrepancies": report.discrepancies.len()}),
        )
    }

    /// Everything the run screen shows.
    pub fn run_status(&self, run_id: &str) -> Result<Value, VelaError> {
        let run = RunId::new(run_id)
            .map_err(|e| VelaError::of_class(ErrorClass::ValidationFailure, e.to_string()))?;
        let state = Runs::new(&self.services).run_state(&run)?;
        let r = run.as_str().to_owned();
        let (workers, promotion) = self.services.store.read(|c| {
            Ok((
                o01_workers::list_for_run(c, &r)?,
                t04_promotions::get(c, &r)?,
            ))
        })?;
        let graph = stored_graph(&self.services, &run)?.map(|g| {
            let positions: HashMap<String, (i64, i64)> = g.layout.nodes.iter().map(|n| (n.ticket.as_str().to_owned(), (n.x, n.y))).collect();
            json!({
                "nodes": g.snapshot.tickets.iter().map(|t| {
                    let (x, y) = positions.get(t.as_str()).copied().unwrap_or((0, 0));
                    json!({"id": t.as_str(), "x": x, "y": y})
                }).collect::<Vec<_>>(),
                "edges": g.snapshot.edges.iter().map(|e| json!({"from": e.blocker().as_str(), "to": e.blocked().as_str()})).collect::<Vec<_>>(),
                "pairs": g.verdicts.iter().map(|v| json!({
                    "a": v.a.as_str(), "b": v.b.as_str(),
                    "label": serde_json::to_value(v.label).unwrap_or(Value::Null),
                    "reasons": v.reasons.iter().map(|x| format!("{x:?}")).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
                "analyzed": g.analyzed,
                "warnings": g.warnings,
            })
        });
        let base = self
            .services
            .store
            .read(|c| s08_build::get_run_base(c, &r))?;
        Ok(json!({
            "run_id": run_id,
            "state": serde_json::to_value(state).unwrap_or(Value::Null),
            "graph": graph,
            "base": base.map(|b| json!({"sha": b.base_sha, "branch": b.base_branch, "integration_branch": b.integration_branch})),
            "workers": workers.iter().map(|w| json!({
                "id": w.id, "ticket": w.ticket_id,
                "state": serde_json::from_str::<Value>(&w.state).unwrap_or(Value::Null),
                "branch": w.branch, "worktree": w.worktree_path,
            })).collect::<Vec<_>>(),
            "interventions": self.interventions_json(&run)?,
            "promotion": promotion.map(|p| json!({"mode": p.mode, "state": p.state, "pr_url": p.pr_url, "note": p.note})),
        }))
    }

    fn interventions_json(&self, run: &RunId) -> Result<Value, VelaError> {
        let open = Interventions::new(&self.services).open_for_run(run)?;
        Ok(Value::Array(open.iter().map(intervention_json).collect()))
    }

    /// Open interventions of a run.
    pub fn intervention_list(&self, run_id: &str) -> Result<Value, VelaError> {
        let run = RunId::new(run_id)
            .map_err(|e| VelaError::of_class(ErrorClass::ValidationFailure, e.to_string()))?;
        self.interventions_json(&run)
    }

    /// The user resolves an intervention; the run then continues.
    pub async fn intervention_resolve(
        self: &Arc<Self>,
        intervention_id: &str,
        resolution: &str,
    ) -> Result<(), VelaError> {
        let ivs = Interventions::new(&self.services);
        let run_id = ivs
            .get(intervention_id)?
            .ok_or_else(|| not_found("intervention"))?
            .run_id;
        ivs.resolve(intervention_id, resolution).await?;
        if matches!(
            Runs::new(&self.services).run_state(
                &RunId::new(run_id.clone()).map_err(|e| VelaError::internal(e.to_string()))?
            )?,
            RunState::Active(_)
        ) {
            self.run_continue(&run_id).await?;
        }
        Ok(())
    }

    /// A ticket's timeline.
    pub fn timeline_get(&self, run_id: &str, ticket: &str) -> Result<Value, VelaError> {
        let run = RunId::new(run_id)
            .map_err(|e| VelaError::of_class(ErrorClass::ValidationFailure, e.to_string()))?;
        let ticket = TicketId::new(ticket)
            .map_err(|e| VelaError::of_class(ErrorClass::ValidationFailure, e.to_string()))?;
        let entries = crate::worker::timeline(&self.services, &run, &ticket)?;
        Ok(Value::Array(
            entries
                .iter()
                .map(|e| json!({"seq": e.seq, "ts": e.ts, "kind": e.kind}))
                .collect(),
        ))
    }
}

fn project_json(p: &crate::project::ProjectView) -> Value {
    json!({
        "id": p.id, "name": p.name, "repo_path": p.repo_path,
        "default_branch": p.default_branch, "head_at_import": p.head_at_import, "last_opened_ms": p.last_opened_ms,
    })
}

fn intervention_json(i: &s15_interventions::Intervention) -> Value {
    json!({
        "id": i.id, "run_id": i.run_id, "worker_id": i.worker_id, "kind": i.kind, "summary": i.summary,
        "evidence": serde_json::from_str::<Value>(&i.evidence).unwrap_or(Value::Null),
        "preserved_work": i.preserved_work, "last_safe_commit": i.last_safe_commit,
        "actions": serde_json::from_str::<Value>(&i.actions).unwrap_or(Value::Null),
        "consequences": i.consequences,
    })
}

#[cfg(test)]
mod tests;
