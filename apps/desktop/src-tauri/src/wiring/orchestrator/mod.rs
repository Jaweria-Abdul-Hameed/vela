//! The orchestrator wiring (O01, W01): the shared services, the application API every command calls, and the background loops
//! that must not depend on a window (notifications).
//!
//! `install` runs once at startup, after the store is open. Everything concrete comes from [`crate::wiring::adapters`]; the API
//! itself is in `vela-core`, where it is tested without a window.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{App, AppHandle, Manager, Runtime};
use vela_adapters::tracker_github::GhTracker;
use vela_adapters::tracker_local::LocalTracker;
use vela_core::diagnostics::{RedactionPort, SharedRedaction};
use vela_core::notifications::NotificationService;
use vela_core::orchestrator::Services;
use vela_core::orchestrator::api::{AppApi, Backend, ProjectContext};
use vela_core::orchestrator::driver::RunDriver;
use vela_core::orchestrator::system::{SystemClock, SystemIds};
use vela_core::preflight::{GhState, PreflightFacts};
use vela_core::promotion::PromotionService;
use vela_core::recovery::Recovery;
use vela_core::stop::StopService;
use vela_domain::errors::VelaError;
use vela_domain::ids::WorktreePath;
use vela_domain::ports::{
    AgentSession, GitRead, IssueTrackerAdapter, Notification, Notifier, PortFuture, ProcessRunner,
    PullRequestPort, RemoteName,
};
use vela_persistence::repositories::s02_settings;
use vela_process::redact::{PatternRedactor, Redactor};
use vela_process::runner::baseline_env;

use crate::wiring::adapters::{self, Adapters};
use crate::wiring::antigravity::{AgyPrompts, ProfilingAgent};
use crate::wiring::store::{StoreHandle, data_root};

/// The application API as managed Tauri state.
#[derive(Clone)]
pub struct ApiHandle(pub Arc<AppApi>);

/// Folders searched for local issues when a project has not chosen a source.
const ISSUE_FOLDERS: &[&str] = &["issues", "docs/issues", ".vela/issues", "tickets"];

/// Whether `<repo>/.git/config` configures the remote `origin`, and the GitHub `owner/name` it points to, if it does.
pub fn origin_of(repo: &Path) -> (bool, Option<String>) {
    let Ok(text) = std::fs::read_to_string(repo.join(".git").join("config")) else {
        return (false, None);
    };
    let mut in_origin = false;
    let mut has = false;
    let mut github = None;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_origin = line == "[remote \"origin\"]";
            has |= in_origin;
        } else if in_origin && let Some(url) = line.strip_prefix("url") {
            let url = url.trim_start_matches([' ', '=']).trim();
            if let Some(rest) = url.split("github.com").nth(1) {
                let path = rest
                    .trim_start_matches([':', '/'])
                    .trim_end_matches(".git")
                    .trim_end_matches('/');
                if path.split('/').count() == 2 && !path.contains(' ') {
                    github = Some(path.to_owned());
                }
            }
        }
    }
    (has, github)
}

struct Redact(PatternRedactor);

impl RedactionPort for Redact {
    fn redact_text(&self, text: &str) -> String {
        self.0.redact(text).into_owned()
    }

    fn redact_value(&self, name: &str, value: &str) -> String {
        self.0.redact_value(name, value).into_owned()
    }
}

/// Desktop notifications through the Tauri notification plugin.
pub struct TauriNotifier<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> Notifier for TauriNotifier<R> {
    fn notify<'a>(&'a self, notification: &'a Notification) -> PortFuture<'a, ()> {
        use tauri_plugin_notification::NotificationExt;
        let shown = self
            .app
            .notification()
            .builder()
            .title(notification.title.clone())
            .body(notification.body.clone())
            .show();
        Box::pin(async move {
            shown.map_err(|e| VelaError::internal(format!("the notification was not shown: {e}")))
        })
    }
}

/// The concrete backend of the application API.
pub struct DesktopBackend {
    services: Arc<Services>,
    adapters: Arc<Adapters>,
    agent: Arc<ProfilingAgent>,
    drivers: Mutex<HashMap<String, Arc<RunDriver>>>,
}

impl DesktopBackend {
    /// A backend over the services and adapters.
    pub fn new(services: Arc<Services>, adapters: Arc<Adapters>) -> Arc<Self> {
        let agent = Arc::new(ProfilingAgent::new(services.clone(), adapters.clone()));
        Arc::new(Self {
            services,
            adapters,
            agent,
            drivers: Mutex::new(HashMap::new()),
        })
    }

    fn tracker_for(&self, p: &ProjectContext) -> Result<Arc<dyn IssueTrackerAdapter>, VelaError> {
        let id = p.id.clone();
        let spec = self.services.store.read(|c| {
            s02_settings::get_project_setting(c, &id, vela_core::orchestrator::api::TRACKER_KEY)
        })?;
        let repo = Path::new(&p.repo_path);
        if let Some(name) = spec.as_deref().and_then(|s| s.strip_prefix("github:")) {
            let cwd = WorktreePath::new(p.repo_path.replace('\\', "/")).map_err(|e| {
                VelaError::internal(format!("the repository path is unusable: {e}"))
            })?;
            let tracker: Arc<dyn IssueTrackerAdapter> = Arc::new(GhTracker::new(
                self.adapters.runner.clone(),
                name,
                cwd,
                baseline_env(),
            )?);
            return Ok(tracker);
        }
        let dir = match spec.as_deref().and_then(|s| s.strip_prefix("local:")) {
            Some(d) => repo.join(d),
            None => ISSUE_FOLDERS
                .iter()
                .map(|d| repo.join(d))
                .find(|d| d.is_dir())
                .unwrap_or_else(|| repo.join("issues")),
        };
        let tracker: Arc<dyn IssueTrackerAdapter> = Arc::new(LocalTracker::new(dir));
        Ok(tracker)
    }

    fn pull_requests_for(&self, p: &ProjectContext) -> Option<Arc<dyn PullRequestPort>> {
        let (_, github) = origin_of(Path::new(&p.repo_path));
        let cwd = WorktreePath::new(p.repo_path.replace('\\', "/")).ok()?;
        let tracker =
            GhTracker::new(self.adapters.runner.clone(), &github?, cwd, baseline_env()).ok()?;
        let port: Arc<dyn PullRequestPort> = Arc::new(tracker);
        Some(port)
    }
}

fn command_output(program: &str, args: &[&str]) -> Option<(bool, String)> {
    let out = std::process::Command::new(program)
        .args(args)
        .output()
        .ok()?;
    Some((
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    ))
}

fn parse_git_version(text: &str) -> Option<(u32, u32, u32)> {
    let v = text
        .split_whitespace()
        .find(|w| w.chars().next().is_some_and(|c| c.is_ascii_digit()))?;
    let mut parts = v.split('.').map(|p| {
        p.chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse::<u32>()
            .ok()
    });
    Some((
        parts.next()??,
        parts.next()??,
        parts.next().flatten().unwrap_or(0),
    ))
}

impl Backend for DesktopBackend {
    fn git_read(&self) -> Arc<dyn GitRead> {
        self.adapters.git.clone()
    }

    fn driver(&self, project: &ProjectContext) -> Result<Arc<RunDriver>, VelaError> {
        if let Some(d) = self.drivers.lock().expect("drivers lock").get(&project.id) {
            return Ok(d.clone());
        }
        let promotion = Arc::new(PromotionService::new(
            self.services.clone(),
            self.adapters.git.clone(),
            self.pull_requests_for(project),
        ));
        let agent: Arc<dyn AgentSession> = self.agent.clone();
        let runner: Arc<dyn ProcessRunner> = self.adapters.runner.clone();
        let driver = RunDriver::new(
            self.services.clone(),
            self.tracker_for(project)?,
            None,
            self.adapters.git.clone(),
            agent,
            runner,
            Arc::new(AgyPrompts),
            promotion,
        );
        self.drivers
            .lock()
            .expect("drivers lock")
            .insert(project.id.clone(), driver.clone());
        Ok(driver)
    }

    fn forget_driver(&self, project_id: &str) {
        self.drivers
            .lock()
            .expect("drivers lock")
            .remove(project_id);
    }

    fn stop_service(&self, driver: &Arc<RunDriver>) -> Arc<StopService> {
        let agent: Arc<dyn AgentSession> = self.agent.clone();
        let runner: Arc<dyn ProcessRunner> = self.adapters.runner.clone();
        Arc::new(StopService::new(
            self.services.clone(),
            agent,
            runner,
            self.adapters.git.clone(),
            driver.lane(),
            Duration::from_secs(30),
        ))
    }

    fn recovery(&self) -> Arc<Recovery> {
        Arc::new(Recovery::new(
            self.services.clone(),
            self.adapters.git.clone(),
            self.agent.clone(),
        ))
    }

    fn worktree_root(&self, project_id: &str) -> String {
        self.adapters
            .data_root
            .join("worktrees")
            .join(project_id)
            .to_string_lossy()
            .replace('\\', "/")
    }

    fn remote_for(&self, project: &ProjectContext) -> Option<RemoteName> {
        origin_of(Path::new(&project.repo_path))
            .0
            .then(|| RemoteName::new("origin").ok())
            .flatten()
    }

    fn fill_facts(&self, facts: &mut PreflightFacts) {
        facts.git_version =
            command_output("git", &["--version"]).and_then(|(_, t)| parse_git_version(&t));
        facts.gh = match command_output("gh", &["auth", "status"]) {
            None => GhState::Missing,
            Some((true, _)) => GhState::Ready,
            Some((false, _)) => GhState::Unauthenticated,
        };
        // The application is running inside its WebView2 window, so the runtime is present.
        facts.webview2 = Some("installed".to_owned());
    }
}

/// Builds the services, registers what must be registered, creates the API and starts the notification loop.
pub fn install<R: Runtime>(app: &mut App<R>) -> Result<(), Box<dyn std::error::Error>> {
    let store = app.state::<StoreHandle>().0.clone();
    let services = Services::new(store, Arc::new(SystemClock), Arc::new(SystemIds::new()));
    vela_core::capacity::register(&services);
    vela_core::broker::register_recipes(&services);
    vela_core::merge_lane::register_recipes(&services);

    let adapters = Arc::new(adapters::build(&data_root()));
    // What the policy asked about, and one-time approvals, for Antigravity workers.
    services.register(Arc::new(vela_core::worker::AskedCommandsService(Arc::new(
        crate::wiring::antigravity::AgyAsked::new(services.clone(), adapters.clone()),
    ))));
    services.register(Arc::new(vela_core::worker::ApprovedNotes::new()));
    // An interrupted checkpoint is found again by the trailer on its commit (or redone), never guessed at.
    vela_core::operations::register_checkpoint_recipe(&services, adapters.git.clone());
    let backend = DesktopBackend::new(services.clone(), adapters);
    let api = AppApi::new(services.clone(), backend);
    app.manage(ApiHandle(api.clone()));
    // Startup reconciliation: finish what a crash interrupted and apply the persisted continuation choice.
    {
        let api = api.clone();
        tauri::async_runtime::spawn(async move {
            match api.startup_recovery().await {
                Ok(v) => tracing::info!(recovered = %v, "startup recovery finished"),
                Err(e) => tracing::warn!(error = %e.what_failed(), "startup recovery failed"),
            }
        });
    }

    let notifier = Arc::new(TauriNotifier {
        app: app.handle().clone(),
    });
    let redactor: SharedRedaction = Arc::new(Redact(PatternRedactor::new()));
    let notifications = NotificationService::new(services, notifier, Some(redactor));
    tauri::async_runtime::spawn(async move {
        loop {
            if let Err(e) = notifications.deliver_new().await {
                tracing::warn!(error = %e.what_failed(), "notification delivery failed");
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_origin_remote_and_its_github_name_are_read_from_the_git_config() {
        let dir = std::env::temp_dir().join(format!("vela-origin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        assert_eq!(origin_of(&dir), (false, None));
        std::fs::write(dir.join(".git/config"), "[core]\n\tbare = false\n[remote \"origin\"]\n\turl = https://github.com/acme/widgets.git\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n").unwrap();
        assert_eq!(origin_of(&dir), (true, Some("acme/widgets".to_owned())));
        std::fs::write(
            dir.join(".git/config"),
            "[remote \"origin\"]\n\turl = git@github.com:acme/widgets.git\n",
        )
        .unwrap();
        assert_eq!(origin_of(&dir).1.as_deref(), Some("acme/widgets"));
        std::fs::write(dir.join(".git/config"), "[remote \"origin\"]\n\turl = https://example.com/x/y.git\n[remote \"other\"]\n\turl = https://github.com/a/b\n").unwrap();
        assert_eq!(origin_of(&dir), (true, None));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn git_versions_are_parsed_from_the_version_line() {
        assert_eq!(
            parse_git_version("git version 2.45.1.windows.1"),
            Some((2, 45, 1))
        );
        assert_eq!(parse_git_version("git version 2.39"), Some((2, 39, 0)));
        assert_eq!(parse_git_version("nonsense"), None);
    }
}
