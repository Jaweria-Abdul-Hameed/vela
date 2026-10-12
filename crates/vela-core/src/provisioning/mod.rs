//! Worktree provisioning (PV1): a fresh worktree becomes runnable through commands the user confirmed, and a failure is reported
//! before the worker starts.
//!
//! Provisioning runs only in trusted repositories, every command passes the policy engine against the new worktree, files are
//! copied only from an explicit allowlist and never when they look like secrets, and nothing is copied automatically.

use std::num::NonZeroU64;
use std::path::{Component, Path};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::ids::WorktreePath;
use vela_domain::policy::{Decision, Dialect, Operation, PolicyEngine, ProfileEntry, RunMode};
use vela_domain::ports::{CommandSpec, OutputChunk, OutputSink, ProcessRunner};
use vela_persistence::repositories::s02_settings;

use crate::orchestrator::Services;
use crate::trust::{ProfileCommand, TrustService};

/// The project-setting key the profile is stored under (written only by [`ProvisioningService::set_profile`]).
pub const PROFILE_KEY: &str = "provisioning.profile";
/// How long one provisioning command may run.
pub const COMMAND_TIMEOUT_MS: u64 = 15 * 60 * 1000;

/// What the user confirmed for preparing a worktree.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ProvisioningProfile {
    /// Commands run in order inside the worktree.
    pub commands: Vec<ProfileCommand>,
    /// Files copied from the primary checkout into the worktree (relative paths, listed one by one).
    pub copy: Vec<String>,
    /// Shared cache folders the commands may use (relative to the worktree), declared so the scheduler can serialize them.
    pub shared_caches: Vec<String>,
    /// Resource keys that limit how many workers may provision or run at once (for example `ports:3000`).
    pub resource_keys: Vec<String>,
}

/// What provisioning did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProvisionReport {
    /// Files copied, relative.
    pub copied: Vec<String>,
    /// Commands that ran and succeeded.
    pub commands_run: Vec<String>,
    /// Resource keys of the profile, for the scheduler.
    pub resource_keys: Vec<String>,
}

/// True for a path that looks like it holds a secret. Such files are never copied, whatever the allowlist says.
pub fn looks_secret(path: &str) -> bool {
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or("");
    name == ".env"
        || name.starts_with(".env.")
        || name.ends_with(".pem")
        || name.ends_with(".key")
        || name.ends_with(".pfx")
        || name.ends_with(".p12")
        || name.starts_with("id_rsa")
        || name.starts_with("id_ed25519")
        || name == ".npmrc"
        || name == ".netrc"
        || name.contains("credentials")
        || name.contains("secret")
        || lower.contains(".aws/")
        || lower.contains(".ssh/")
}

fn safe_relative(path: &str) -> bool {
    let p = Path::new(path);
    !path.is_empty()
        && p.is_relative()
        && !path.starts_with(['/', '\\'])
        && p.components().all(|c| matches!(c, Component::Normal(_)))
}

fn invalid(msg: impl Into<String>) -> VelaError {
    VelaError::of_class(ErrorClass::ValidationFailure, msg)
        .with_state_preserved("the worker was not started")
        .with_user_action("fix the provisioning profile in the project settings")
}

#[derive(Default)]
struct Tail(Mutex<Vec<String>>);

impl OutputSink for Tail {
    fn on_output(&self, chunk: OutputChunk) {
        let mut lines = self.0.lock().expect("tail lock");
        for line in chunk.text.lines() {
            lines.push(line.to_owned());
        }
        let excess = lines.len().saturating_sub(20);
        lines.drain(..excess);
    }
}

/// Provisioning over the shared services.
pub struct ProvisioningService {
    services: Arc<Services>,
    runner: Arc<dyn ProcessRunner>,
}

impl ProvisioningService {
    /// A service that runs commands through `runner`.
    pub fn new(services: Arc<Services>, runner: Arc<dyn ProcessRunner>) -> Self {
        Self { services, runner }
    }

    /// Stores the profile the user confirmed. Copy entries must be relative, inside the repository, and not secret-like.
    pub fn set_profile(
        &self,
        project_id: &str,
        profile: &ProvisioningProfile,
    ) -> Result<(), VelaError> {
        for p in profile.copy.iter().chain(&profile.shared_caches) {
            if !safe_relative(p) {
                return Err(invalid(format!(
                    "`{p}` must be a relative path inside the repository"
                )));
            }
        }
        if let Some(p) = profile.copy.iter().find(|p| looks_secret(p)) {
            return Err(invalid(format!(
                "`{p}` looks like a secret and is never copied automatically"
            )));
        }
        let scratch = if cfg!(windows) {
            "C:/vela-validate"
        } else {
            "/vela-validate"
        };
        for c in &profile.commands {
            ProfileEntry::parse(&c.text, Dialect::Cmd, c.cwd.as_deref(), scratch)
                .map_err(|e| invalid(format!("the command `{}` is not allowed: {e}", c.text)))?;
        }
        let json =
            serde_json::to_string(profile).map_err(|e| VelaError::internal(e.to_string()))?;
        let (pid, now) = (project_id.to_owned(), self.services.now_ms());
        self.services.store.write(move |tx| {
            s02_settings::set_project_setting(tx.conn(), &pid, PROFILE_KEY, &json, now)
        })
    }

    /// The stored profile (empty when none was confirmed).
    pub fn profile(&self, project_id: &str) -> Result<ProvisioningProfile, VelaError> {
        let pid = project_id.to_owned();
        let text = self
            .services
            .store
            .read(|c| s02_settings::get_project_setting(c, &pid, PROFILE_KEY))?;
        match text {
            None => Ok(ProvisioningProfile::default()),
            Some(t) => serde_json::from_str(&t).map_err(|e| {
                VelaError::internal(format!("the provisioning profile is unreadable: {e}"))
            }),
        }
    }

    /// Prepares `worktree` (a managed worktree of the project checked out at `repo_root`). Any failure is a validation failure
    /// and the worker must not start.
    pub async fn provision(
        &self,
        project_id: &str,
        repo_root: &Path,
        worktree: &Path,
    ) -> Result<ProvisionReport, VelaError> {
        let trust = TrustService::new(&self.services);
        trust.require_trusted(project_id, "Provisioning")?;
        let profile = self.profile(project_id)?;
        let mut report = ProvisionReport {
            resource_keys: profile.resource_keys.clone(),
            ..ProvisionReport::default()
        };
        // Short 8.3 names and verbatim prefixes are resolved so the policy engine sees the real location.
        let worktree_buf =
            std::fs::canonicalize(worktree).unwrap_or_else(|_| worktree.to_path_buf());
        let worktree = worktree_buf.as_path();
        let root = worktree
            .to_string_lossy()
            .trim_start_matches(r"\\?\")
            .replace('\\', "/");

        // Every command must pass the policy engine against this worktree before anything is copied or run.
        let confirmed = ProvisioningProfileAsContext::new(&trust, project_id, &root, &profile)?;
        let engine = PolicyEngine::new(confirmed.context);
        for entry in &confirmed.entries {
            let op = Operation::Command {
                dialect: Dialect::Cmd,
                text: entry.canonical_text(),
                cwd: Some(entry.cwd().to_owned()),
            };
            let decision = engine.evaluate(&op);
            if decision.decision != Decision::Allow {
                return Err(invalid(format!(
                    "the policy refuses `{}`: {}",
                    entry.canonical_text(),
                    decision.reason
                )));
            }
        }

        for rel in &profile.copy {
            self.copy_one(repo_root, worktree, rel)?;
            report.copied.push(rel.clone());
        }

        for (i, entry) in confirmed.entries.iter().enumerate() {
            let cwd = WorktreePath::new(entry.cwd())
                .map_err(|e| invalid(format!("the command directory is not usable: {e}")))?;
            let spec = CommandSpec {
                id: format!("provision-{project_id}-{i}"),
                program: entry.executable().to_owned(),
                args: entry.args().to_vec(),
                cwd,
                env: Vec::new(),
                timeout_ms: NonZeroU64::new(COMMAND_TIMEOUT_MS).expect("non-zero"),
            };
            let tail = Tail::default();
            let outcome = self.runner.run(&spec, &tail).await?;
            let text = entry.canonical_text();
            if outcome.timed_out || outcome.exit_code != Some(0) {
                let why = if outcome.timed_out {
                    "timed out".to_owned()
                } else {
                    format!(
                        "exited with {}",
                        outcome
                            .exit_code
                            .map_or("no code".to_owned(), |c| c.to_string())
                    )
                };
                let lines = tail.0.lock().expect("tail lock").join("\n");
                return Err(invalid(format!(
                    "provisioning command `{text}` {why}\n{lines}"
                )));
            }
            report.commands_run.push(text);
        }
        Ok(report)
    }

    fn copy_one(&self, repo_root: &Path, worktree: &Path, rel: &str) -> Result<(), VelaError> {
        if !safe_relative(rel) || looks_secret(rel) {
            return Err(invalid(format!("`{rel}` cannot be copied")));
        }
        let from = repo_root.join(rel);
        let meta = from
            .symlink_metadata()
            .map_err(|_| invalid(format!("`{rel}` to copy does not exist in the repository")))?;
        if !meta.is_file() {
            return Err(invalid(format!("`{rel}` is not a regular file")));
        }
        let to = worktree.join(rel);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| invalid(format!("cannot prepare the folder for `{rel}`: {e}")))?;
        }
        std::fs::copy(&from, &to).map_err(|e| invalid(format!("cannot copy `{rel}`: {e}")))?;
        Ok(())
    }
}

/// The profile commands resolved against one worktree, with the policy context built from them.
struct ProvisioningProfileAsContext {
    entries: Vec<ProfileEntry>,
    context: vela_domain::policy::PolicyContext,
}

impl ProvisioningProfileAsContext {
    fn new(
        trust: &TrustService<'_>,
        project_id: &str,
        root: &str,
        profile: &ProvisioningProfile,
    ) -> Result<Self, VelaError> {
        let entries = profile
            .commands
            .iter()
            .map(|c| ProfileEntry::parse(&c.text, Dialect::Cmd, c.cwd.as_deref(), root))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| invalid(format!("a provisioning command is not valid here: {e}")))?;
        // Trust state comes from the store; the provisioning commands themselves are the confirmed profile for this check.
        let state = trust.state(project_id)?;
        let context = vela_domain::policy::PolicyContext::new(
            root,
            state,
            RunMode::Supervised,
            vela_domain::policy::CommandProfile::new(entries.clone())
                .map_err(|e| invalid(format!("the profile is too large: {e}")))?,
        )
        .map_err(|e| invalid(format!("the worktree root is not usable: {e}")))?;
        Ok(Self { entries, context })
    }
}

#[cfg(test)]
mod tests;
