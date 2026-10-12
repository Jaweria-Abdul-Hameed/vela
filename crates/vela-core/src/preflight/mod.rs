//! Preflight (S04): a PASS/WARN/BLOCK report on whether a project is ready to Build.
//!
//! Checks are pure functions over [`PreflightFacts`], so the engine never runs a repository's code: manifests are read as data
//! and platform facts (Git version, WebView2, free disk) are collected by the caller. Providers are registered with the engine,
//! so later tickets (Antigravity, skills, posture, unattended readiness) add checks without editing it.

use std::path::Path;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use vela_domain::errors::VelaError;
use vela_domain::policy::TrustState;
use vela_persistence::repositories::s04_preflight::{self, ReportRow};

use crate::orchestrator::Services;

pub mod antigravity;

/// How serious a finding is. The order is the severity order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    /// Fine.
    Pass,
    /// Worth knowing; Build can proceed.
    Warn,
    /// Build cannot start until this is fixed.
    Block,
}

impl Severity {
    /// The wire and storage name.
    pub fn name(self) -> &'static str {
        match self {
            Severity::Pass => "PASS",
            Severity::Warn => "WARN",
            Severity::Block => "BLOCK",
        }
    }
}

/// One finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    /// Stable identifier (`git.version`).
    pub id: String,
    /// Short title.
    pub title: String,
    /// How serious it is.
    pub severity: Severity,
    /// What was found.
    pub detail: String,
    /// What the user can do about it.
    pub remediation: Option<String>,
}

impl Check {
    fn new(
        id: &str,
        title: &str,
        severity: Severity,
        detail: impl Into<String>,
        fix: Option<&str>,
    ) -> Self {
        Self {
            id: id.to_owned(),
            title: title.to_owned(),
            severity,
            detail: detail.into(),
            remediation: fix.map(str::to_owned),
        }
    }

    fn pass(id: &str, title: &str, detail: impl Into<String>) -> Self {
        Self::new(id, title, Severity::Pass, detail, None)
    }
}

/// The state of the GitHub CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GhState {
    /// Installed and logged in.
    Ready,
    /// Not installed.
    Missing,
    /// Installed, not logged in.
    Unauthenticated,
}

/// Everything the checks look at. Collect-able without running repository code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreflightFacts {
    /// The repository path.
    pub repo_path: String,
    /// Whether the folder is a Git repository.
    pub is_git_repo: bool,
    /// HEAD is not on a branch.
    pub head_detached: bool,
    /// A merge or rebase is in progress.
    pub operation_in_progress: bool,
    /// Number of uncommitted paths.
    pub dirty_paths: usize,
    /// The default branch, when resolved.
    pub default_branch: Option<String>,
    /// A remote is configured.
    pub has_remote: bool,
    /// Installed Git version.
    pub git_version: Option<(u32, u32, u32)>,
    /// GitHub CLI state.
    pub gh: GhState,
    /// Documentation manifest paths.
    pub docs: Vec<String>,
    /// Trust state.
    pub trust: TrustState,
    /// Toolchain markers found (`package.json`, `Cargo.toml`, ...).
    pub toolchains: Vec<String>,
    /// The repository sits in a folder a sync client manages.
    pub in_synced_folder: bool,
    /// Longest path (in characters) of a managed worktree file, estimated.
    pub longest_worktree_path: usize,
    /// `.gitmodules` exists.
    pub has_submodules: bool,
    /// `core.autocrlf` value, when set.
    pub autocrlf: Option<String>,
    /// A `.gitattributes` file exists.
    pub has_gitattributes: bool,
    /// WebView2 runtime version, when installed.
    pub webview2: Option<String>,
    /// Free bytes on the volume that will hold worktrees.
    pub free_disk_bytes: Option<u64>,
    /// Worktrees the plan needs.
    pub planned_worktrees: u32,
    /// Estimated bytes per worktree.
    pub bytes_per_worktree: u64,
}

/// Collects the facts that can be read from the folder itself, as data. Platform facts start empty for the caller to fill.
pub fn collect_local(root: &Path) -> PreflightFacts {
    let is_git_repo = root.join(".git").exists();
    let git_dir = root.join(".git");
    let operation = [
        "MERGE_HEAD",
        "rebase-merge",
        "rebase-apply",
        "CHERRY_PICK_HEAD",
    ]
    .iter()
    .any(|n| git_dir.join(n).exists());
    let lowered = root
        .to_string_lossy()
        .to_ascii_lowercase()
        .replace('\\', "/");
    let synced = [
        "/onedrive",
        "/dropbox",
        "/google drive",
        "/googledrive",
        "/icloud",
        "/box sync",
    ]
    .iter()
    .any(|m| lowered.contains(m));
    let toolchains = [
        "package.json",
        "Cargo.toml",
        "pyproject.toml",
        "go.mod",
        "pom.xml",
        "build.gradle",
        "Gemfile",
    ]
    .iter()
    .filter(|m| root.join(m).is_file())
    .map(|m| (*m).to_owned())
    .collect();
    let docs = crate::context::discover(root, &[])
        .entries
        .into_iter()
        .map(|e| e.path)
        .collect();
    PreflightFacts {
        repo_path: root.to_string_lossy().into_owned(),
        is_git_repo,
        head_detached: std::fs::read_to_string(git_dir.join("HEAD"))
            .map(|h| !h.starts_with("ref:"))
            .unwrap_or(false),
        operation_in_progress: operation,
        dirty_paths: 0,
        default_branch: None,
        has_remote: false,
        git_version: None,
        gh: GhState::Missing,
        docs,
        trust: TrustState::Untrusted,
        toolchains,
        in_synced_folder: synced,
        longest_worktree_path: lowered.len() + 120,
        has_submodules: root.join(".gitmodules").is_file(),
        autocrlf: None,
        has_gitattributes: root.join(".gitattributes").is_file(),
        webview2: None,
        free_disk_bytes: None,
        planned_worktrees: 1,
        bytes_per_worktree: 0,
    }
}

/// A source of checks. Providers are pure: same facts, same findings.
pub trait CheckProvider: Send + Sync {
    /// Identifier of the provider.
    fn id(&self) -> &'static str;
    /// The findings for `facts`.
    fn run(&self, facts: &PreflightFacts) -> Vec<Check>;
}

struct Repository;
struct TrustCheck;
struct Toolchain;
struct GitChecks;
struct Docs;
struct WindowsChecks;

impl CheckProvider for Repository {
    fn id(&self) -> &'static str {
        "repository"
    }
    fn run(&self, f: &PreflightFacts) -> Vec<Check> {
        let mut out = Vec::new();
        if !f.is_git_repo {
            out.push(Check::new(
                "repo.git",
                "Git repository",
                Severity::Block,
                "the folder is not a Git repository",
                Some("choose the folder that contains the .git folder"),
            ));
            return out;
        }
        out.push(Check::pass("repo.git", "Git repository", "found"));
        out.push(if f.operation_in_progress {
            Check::new(
                "repo.operation",
                "No merge or rebase in progress",
                Severity::Block,
                "a merge, rebase or cherry-pick is unfinished",
                Some("finish or abort it in the repository, then run preflight again"),
            )
        } else {
            Check::pass("repo.operation", "No merge or rebase in progress", "clean")
        });
        out.push(if f.head_detached {
            Check::new(
                "repo.head",
                "On a branch",
                Severity::Warn,
                "HEAD is detached",
                Some("check out the branch the run should start from"),
            )
        } else {
            Check::pass("repo.head", "On a branch", "attached")
        });
        out.push(if f.dirty_paths > 0 {
            Check::new("repo.dirty", "Clean working tree", Severity::Warn, format!("{} uncommitted paths; the run starts from the last commit and leaves them alone", f.dirty_paths), Some("commit or stash them if the run should include them"))
        } else {
            Check::pass("repo.dirty", "Clean working tree", "clean")
        });
        out.push(match &f.default_branch {
            Some(b) => Check::pass("repo.default_branch", "Default branch", b.clone()),
            None => Check::new(
                "repo.default_branch",
                "Default branch",
                Severity::Warn,
                "the default branch could not be determined",
                Some("set a remote or name the branch in the project settings"),
            ),
        });
        out
    }
}

impl CheckProvider for TrustCheck {
    fn id(&self) -> &'static str {
        "trust"
    }
    fn run(&self, f: &PreflightFacts) -> Vec<Check> {
        vec![match f.trust {
            TrustState::Trusted => Check::pass(
                "trust.state",
                "Repository trusted",
                "the repository is trusted",
            ),
            TrustState::Untrusted => Check::new(
                "trust.state",
                "Repository trusted",
                Severity::Block,
                "the repository is untrusted: it can be analyzed as data but not built",
                Some("review what becomes executable and trust the repository"),
            ),
        }]
    }
}

impl CheckProvider for Toolchain {
    fn id(&self) -> &'static str {
        "toolchain"
    }
    fn run(&self, f: &PreflightFacts) -> Vec<Check> {
        vec![if f.toolchains.is_empty() {
            Check::new(
                "toolchain.manifest",
                "Project toolchain",
                Severity::Warn,
                "no known manifest (package.json, Cargo.toml, ...) was found",
                Some("confirm the build and test commands in the command profile"),
            )
        } else {
            Check::pass(
                "toolchain.manifest",
                "Project toolchain",
                f.toolchains.join(", "),
            )
        }]
    }
}

impl CheckProvider for GitChecks {
    fn id(&self) -> &'static str {
        "git"
    }
    fn run(&self, f: &PreflightFacts) -> Vec<Check> {
        let mut out = Vec::new();
        out.push(match f.git_version {
            None => Check::new(
                "git.version",
                "Git version",
                Severity::Block,
                "Git was not found",
                Some("install Git for Windows"),
            ),
            Some(v) if v < (2, 25, 0) => Check::new(
                "git.version",
                "Git version",
                Severity::Block,
                format!("Git {}.{}.{} is too old for worktrees", v.0, v.1, v.2),
                Some("update Git for Windows"),
            ),
            Some(v) if v < (2, 39, 0) => Check::new(
                "git.version",
                "Git version",
                Severity::Warn,
                format!(
                    "Git {}.{}.{} works but a newer version is recommended",
                    v.0, v.1, v.2
                ),
                Some("update Git for Windows"),
            ),
            Some(v) => Check::pass(
                "git.version",
                "Git version",
                format!("{}.{}.{}", v.0, v.1, v.2),
            ),
        });
        out.push(match (f.has_remote, f.gh) {
            (false, _) => Check::new(
                "git.remote",
                "Remote",
                Severity::Warn,
                "no remote: results stay local-ready",
                Some("add a remote to open a pull request at the end"),
            ),
            (true, GhState::Ready) => Check::pass("git.remote", "Remote and GitHub CLI", "ready"),
            (true, GhState::Missing) => Check::new(
                "git.gh",
                "GitHub CLI",
                Severity::Warn,
                "the GitHub CLI is not installed; the run continues local-only",
                Some("install `gh` to open pull requests"),
            ),
            (true, GhState::Unauthenticated) => Check::new(
                "git.gh",
                "GitHub CLI",
                Severity::Warn,
                "the GitHub CLI is not logged in; the run continues local-only",
                Some("run `gh auth login`"),
            ),
        });
        out
    }
}

impl CheckProvider for Docs {
    fn id(&self) -> &'static str {
        "docs"
    }
    fn run(&self, f: &PreflightFacts) -> Vec<Check> {
        vec![
            if f.docs.iter().any(|d| d.eq_ignore_ascii_case("AGENTS.md")) {
                Check::pass("docs.agents", "Agent instructions", "AGENTS.md found")
            } else {
                Check::new(
                    "docs.agents",
                    "Agent instructions",
                    Severity::Warn,
                    "no AGENTS.md: workers get no project instructions",
                    Some("add an AGENTS.md describing how to build and test"),
                )
            },
        ]
    }
}

impl CheckProvider for WindowsChecks {
    fn id(&self) -> &'static str {
        "windows"
    }
    fn run(&self, f: &PreflightFacts) -> Vec<Check> {
        let mut out = Vec::new();
        out.push(if f.in_synced_folder {
            Check::new("win.synced", "Not in a synced folder", Severity::Block, "the repository is inside a folder a sync client manages; worktrees there corrupt or conflict", Some("move the repository outside OneDrive, Dropbox or similar"))
        } else {
            Check::pass("win.synced", "Not in a synced folder", "ok")
        });
        out.push(if f.longest_worktree_path > 240 {
            Check::new(
                "win.path",
                "Path length",
                Severity::Block,
                format!(
                    "worktree paths would reach about {} characters",
                    f.longest_worktree_path
                ),
                Some("move the repository to a shorter path"),
            )
        } else if f.longest_worktree_path > 200 {
            Check::new(
                "win.path",
                "Path length",
                Severity::Warn,
                format!(
                    "worktree paths reach about {} characters",
                    f.longest_worktree_path
                ),
                Some("move the repository to a shorter path"),
            )
        } else {
            Check::pass("win.path", "Path length", "ok")
        });
        if f.has_submodules {
            out.push(Check::new(
                "win.submodules",
                "Submodules",
                Severity::Warn,
                "submodules are not initialized in worktrees automatically",
                Some("add the initialization command to the provisioning profile"),
            ));
        }
        if f.autocrlf.as_deref() == Some("true") && !f.has_gitattributes {
            out.push(Check::new("win.eol", "Line endings", Severity::Warn, "autocrlf is on and there is no .gitattributes: line endings may differ between worktrees", Some("add a .gitattributes with the line-ending policy")));
        }
        out.push(match &f.webview2 {
            Some(v) => Check::pass("win.webview2", "WebView2 runtime", v.clone()),
            None => Check::new(
                "win.webview2",
                "WebView2 runtime",
                Severity::Block,
                "the WebView2 runtime is not installed",
                Some("install the Evergreen WebView2 runtime"),
            ),
        });
        if let Some(free) = f.free_disk_bytes {
            let need = u64::from(f.planned_worktrees) * f.bytes_per_worktree;
            out.push(if free < need.saturating_mul(3) / 2 {
                Check::new(
                    "win.disk",
                    "Disk space",
                    Severity::Block,
                    format!("{free} bytes free, about {need} needed for the planned worktrees"),
                    Some("free disk space or lower the worker count"),
                )
            } else if free < need.saturating_mul(3) {
                Check::new(
                    "win.disk",
                    "Disk space",
                    Severity::Warn,
                    format!("{free} bytes free for about {need} of worktrees"),
                    Some("free disk space"),
                )
            } else {
                Check::pass("win.disk", "Disk space", "enough")
            });
        }
        out
    }
}

/// The report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    /// Identifier.
    pub id: String,
    /// The worst severity found.
    pub verdict: Severity,
    /// Every finding, grouped by provider order.
    pub checks: Vec<Check>,
    /// When it ran.
    pub created_ms: i64,
}

impl Report {
    /// Findings that block Build.
    pub fn blockers(&self) -> impl Iterator<Item = &Check> {
        self.checks.iter().filter(|c| c.severity == Severity::Block)
    }
}

/// Runs registered providers and keeps the history.
pub struct PreflightEngine {
    services: Arc<Services>,
    providers: Mutex<Vec<Arc<dyn CheckProvider>>>,
}

impl PreflightEngine {
    /// An engine with the built-in providers.
    pub fn new(services: Arc<Services>) -> Self {
        let builtin: Vec<Arc<dyn CheckProvider>> = vec![
            Arc::new(Repository),
            Arc::new(TrustCheck),
            Arc::new(Toolchain),
            Arc::new(GitChecks),
            Arc::new(Docs),
            Arc::new(WindowsChecks),
        ];
        Self {
            services,
            providers: Mutex::new(builtin),
        }
    }

    /// Adds a provider (a second one with the same id replaces the first).
    pub fn register(&self, provider: Arc<dyn CheckProvider>) {
        let mut providers = self.providers.lock().expect("providers lock");
        providers.retain(|p| p.id() != provider.id());
        providers.push(provider);
    }

    /// Runs every provider over `facts`, stores the report and returns it.
    pub fn run(&self, project_id: &str, facts: &PreflightFacts) -> Result<Report, VelaError> {
        let providers: Vec<_> = self.providers.lock().expect("providers lock").clone();
        let checks: Vec<Check> = providers.iter().flat_map(|p| p.run(facts)).collect();
        let verdict = checks
            .iter()
            .map(|c| c.severity)
            .max()
            .unwrap_or(Severity::Pass);
        let now = self.services.now_ms();
        let report = Report {
            id: format!("pf-{}", self.services.ids.new_token()),
            verdict,
            checks,
            created_ms: now,
        };
        let json =
            serde_json::to_string(&report).map_err(|e| VelaError::internal(e.to_string()))?;
        let row = ReportRow {
            id: report.id.clone(),
            project_id: project_id.to_owned(),
            verdict: verdict.name().to_owned(),
            report: json,
            created_ms: now,
        };
        self.services
            .store
            .write(move |tx| s04_preflight::insert_report(tx.conn(), &row))?;
        Ok(report)
    }

    /// The newest reports, newest first.
    pub fn history(&self, project_id: &str, limit: u32) -> Result<Vec<Report>, VelaError> {
        let pid = project_id.to_owned();
        let rows = self
            .services
            .store
            .read(|c| s04_preflight::list_reports(c, &pid, limit))?;
        rows.into_iter()
            .map(|r| {
                serde_json::from_str(&r.report)
                    .map_err(|e| VelaError::internal(format!("a stored report is unreadable: {e}")))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
