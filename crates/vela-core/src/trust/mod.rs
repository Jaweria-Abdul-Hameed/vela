//! Repository trust (S03, ADR-013): a repository is untrusted until the user explicitly trusts it. Untrusted means data-only:
//! Vela may analyze it but refuses to build or provision. Trust, its consent text version and the confirmed command profile are
//! persisted and journaled together, and only a user action can change them (repository text has no path to this module).

use std::path::Path;

use serde::{Deserialize, Serialize};
use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::events::{Event, ProjectTrustRevokedPayload, ProjectTrustedPayload};
use vela_domain::policy::{
    CommandProfile, Dialect, PolicyContext, ProfileEntry, RunMode, TrustState,
};
use vela_domain::ports::RepoTrust;
use vela_persistence::journal::NewEvent;
use vela_persistence::repositories::s03_trust::{self, TrustRow};

use crate::orchestrator::Services;

/// One command of the profile as the user confirmed it: the text and a directory relative to the checkout root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileCommand {
    /// The command line (a single simple command).
    pub text: String,
    /// Directory relative to the worktree root; the root when absent.
    pub cwd: Option<String>,
}

/// The consent the user saw when trusting a repository.
pub const TRUST_CONSENT_VERSION: &str = "2026-10-v1";

/// Everything the trust sheet lists: what becomes executable once the repository is trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustProposal {
    /// Commands found in manifests (read as data; nothing was run).
    pub commands: Vec<ProfileCommand>,
    /// Files that would become executable: scripts, hooks, provisioning.
    pub executable_surfaces: Vec<String>,
}

fn command(text: &str) -> ProfileCommand {
    ProfileCommand {
        text: text.to_owned(),
        cwd: None,
    }
}

/// Reads the manifests of the checkout at `root` as data and proposes a command profile. No script is run.
pub fn propose(root: &Path) -> TrustProposal {
    let mut commands = Vec::new();
    let mut surfaces = Vec::new();
    if let Ok(text) = std::fs::read_to_string(root.join("package.json")) {
        surfaces.push("package.json scripts".to_owned());
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text)
            && let Some(scripts) = value.get("scripts").and_then(|s| s.as_object())
        {
            for name in ["test", "build", "lint", "typecheck"] {
                if scripts.contains_key(name) {
                    commands.push(command(&if name == "test" {
                        "npm test".to_owned()
                    } else {
                        format!("npm run {name}")
                    }));
                }
            }
        }
    }
    if root.join("Cargo.toml").is_file() {
        surfaces.push("Cargo.toml (build scripts)".to_owned());
        for c in ["cargo build", "cargo test", "cargo clippy"] {
            commands.push(command(c));
        }
    }
    if root.join("pyproject.toml").is_file() || root.join("pytest.ini").is_file() {
        surfaces.push("Python project configuration".to_owned());
        commands.push(command("pytest"));
    }
    if root.join("go.mod").is_file() {
        commands.push(command("go build ./..."));
        commands.push(command("go test ./..."));
    }
    for hook_dir in [".husky", ".githooks"] {
        if root.join(hook_dir).is_dir() {
            surfaces.push(format!("{hook_dir} hooks"));
        }
    }
    if root.join(".gitmodules").is_file() {
        surfaces.push("submodules".to_owned());
    }
    TrustProposal {
        commands,
        executable_surfaces: surfaces,
    }
}

/// Trust over the shared services.
pub struct TrustService<'a> {
    services: &'a Services,
}

fn untrusted(what: &str) -> VelaError {
    VelaError::of_class(
        ErrorClass::PolicyBlock,
        format!("{what} is not allowed: this repository is not trusted yet"),
    )
    .with_state_preserved("nothing was changed")
    .with_user_action("review what would become executable and choose Trust, or keep using the repository as data only")
}

impl<'a> TrustService<'a> {
    /// Trust over `services`.
    pub fn new(services: &'a Services) -> Self {
        Self { services }
    }

    fn row(&self, project_id: &str) -> Result<Option<TrustRow>, VelaError> {
        self.services.store.read(|c| s03_trust::get(c, project_id))
    }

    /// The trust state: untrusted unless the user trusted it.
    pub fn state(&self, project_id: &str) -> Result<TrustState, VelaError> {
        Ok(match self.row(project_id)? {
            Some(r) if r.state == "TRUSTED" => TrustState::Trusted,
            _ => TrustState::Untrusted,
        })
    }

    /// The trust level Git operations on this project run under.
    pub fn git_trust(&self, project_id: &str) -> Result<RepoTrust, VelaError> {
        Ok(match self.state(project_id)? {
            TrustState::Trusted => RepoTrust::Trusted,
            TrustState::Untrusted => RepoTrust::Untrusted,
        })
    }

    /// Fails with a typed policy block unless the project is trusted. Build and provisioning call this first.
    pub fn require_trusted(&self, project_id: &str, what: &str) -> Result<(), VelaError> {
        match self.state(project_id)? {
            TrustState::Trusted => Ok(()),
            TrustState::Untrusted => Err(untrusted(what)),
        }
    }

    /// The user trusts the repository, confirming `commands` as its command profile. The commands are validated now (each
    /// must be a single simple command) so a bad profile never gets stored. State and journal commit together.
    pub fn trust(
        &self,
        project_id: &str,
        repository_identity: &str,
        commands: &[ProfileCommand],
        consent_text_version: &str,
    ) -> Result<(), VelaError> {
        // Validate against a scratch root: parsing only checks the command and that the directory stays inside the root.
        let scratch = if cfg!(windows) {
            "C:/vela-validate"
        } else {
            "/vela-validate"
        };
        for c in commands {
            ProfileEntry::parse(&c.text, Dialect::Cmd, c.cwd.as_deref(), scratch).map_err(|e| {
                VelaError::of_class(
                    ErrorClass::ValidationFailure,
                    format!(
                        "the command `{}` cannot be part of the profile: {e}",
                        c.text
                    ),
                )
                .with_user_action("use a single simple command without pipes or redirection")
            })?;
        }
        if consent_text_version.trim().is_empty() {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "trust needs the version of the consent text the user saw",
            ));
        }
        let profile =
            serde_json::to_string(commands).map_err(|e| VelaError::internal(e.to_string()))?;
        let now = self.services.now_ms();
        let row = TrustRow {
            project_id: project_id.to_owned(),
            state: "TRUSTED".to_owned(),
            repository_identity: repository_identity.to_owned(),
            consent_text_version: Some(consent_text_version.to_owned()),
            command_profile: profile,
            changed_ms: now,
        };
        let event = NewEvent {
            ts: now,
            run_id: None,
            ticket_id: None,
            worker_id: None,
            event: Event::ProjectTrusted(ProjectTrustedPayload {
                repository_identity: repository_identity.to_owned(),
                consent_text_version: consent_text_version.to_owned(),
            }),
        };
        self.services.store.write(move |tx| {
            s03_trust::upsert(tx.conn(), &row)?;
            tx.append(&event).map(|_| ())
        })
    }

    /// Revokes trust. The confirmed profile is cleared; the project is data-only again. Revoking an untrusted project is a
    /// no-op that journals nothing.
    pub fn revoke(&self, project_id: &str) -> Result<(), VelaError> {
        let Some(existing) = self.row(project_id)?.filter(|r| r.state == "TRUSTED") else {
            return Ok(());
        };
        let now = self.services.now_ms();
        let row = TrustRow {
            state: "UNTRUSTED".to_owned(),
            consent_text_version: None,
            command_profile: "[]".to_owned(),
            changed_ms: now,
            ..existing.clone()
        };
        let event = NewEvent {
            ts: now,
            run_id: None,
            ticket_id: None,
            worker_id: None,
            event: Event::ProjectTrustRevoked(ProjectTrustRevokedPayload {
                repository_identity: existing.repository_identity,
            }),
        };
        self.services.store.write(move |tx| {
            s03_trust::upsert(tx.conn(), &row)?;
            tx.append(&event).map(|_| ())
        })
    }

    /// The confirmed command profile as stored.
    pub fn profile_commands(&self, project_id: &str) -> Result<Vec<ProfileCommand>, VelaError> {
        match self.row(project_id)? {
            Some(r) if r.state == "TRUSTED" => {
                serde_json::from_str(&r.command_profile).map_err(|e| {
                    VelaError::internal(format!("the stored command profile is unreadable: {e}"))
                })
            }
            _ => Ok(Vec::new()),
        }
    }

    /// The policy context for a worktree of this project: the trust state, and the confirmed profile resolved against the
    /// worktree root. An untrusted project yields an empty profile, so no command can be allowed.
    pub fn policy_context(
        &self,
        project_id: &str,
        worktree_root: &str,
        mode: RunMode,
    ) -> Result<PolicyContext, VelaError> {
        let trust = self.state(project_id)?;
        let entries = self
            .profile_commands(project_id)?
            .iter()
            .map(|c| ProfileEntry::parse(&c.text, Dialect::Cmd, c.cwd.as_deref(), worktree_root))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                VelaError::internal(format!("the stored command profile is invalid: {e}"))
            })?;
        let profile = CommandProfile::new(entries).map_err(|e| {
            VelaError::internal(format!("the stored command profile is invalid: {e}"))
        })?;
        PolicyContext::new(worktree_root, trust, mode, profile).map_err(|e| {
            VelaError::of_class(
                ErrorClass::ValidationFailure,
                format!("the worktree root is not usable: {e}"),
            )
        })
    }
}

#[cfg(test)]
mod tests;
