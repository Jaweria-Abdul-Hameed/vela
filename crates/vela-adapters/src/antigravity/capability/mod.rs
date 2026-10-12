//! Antigravity capability discovery (A01): installation, version, the flags the installed CLI really has, the sign-in state,
//! and the CAP-01..CAP-12 report with each missing capability's specified outcome (`ADAPTERS.md`, `CAP` table).
//!
//! The probe only runs cheap, quota-free commands (`agy --version`, `agy --help`, `agy models`). It never reads, copies or
//! persists an Antigravity credential or token: sign-in is judged from the CLI's own answer to `agy models`.
//! Persisting the snapshot belongs to the preflight service (adapter crates never depend on persistence).

use std::collections::BTreeSet;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use vela_domain::errors::{ErrorCode, VelaError};
use vela_domain::ids::WorktreePath;
use vela_domain::ports::{
    AgentCapabilities, AgentCapability, AgentProfile, CommandSpec, OutputChunk, OutputSink,
    OutputStream, PermissionPosture, PortFuture, ProcessRunner, SessionRef,
};

use super::events::{TurnClass, classify_text};
use super::session::AgyDriver;

/// One of the capabilities of `ADAPTERS.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CapId {
    /// CAP-01: discover installation, version and capability set.
    Cap01,
    /// CAP-02: fresh isolated session bound to a worktree.
    Cap02,
    /// CAP-03: deliver the task and invoke skills.
    Cap03,
    /// CAP-04: observe lifecycle.
    Cap04,
    /// CAP-05: cancel a session.
    Cap05,
    /// CAP-06: query status.
    Cap06,
    /// CAP-07: concurrency limit.
    Cap07,
    /// CAP-08: correlation identity.
    Cap08,
    /// CAP-09: capacity signals.
    Cap09,
    /// CAP-10: resume.
    Cap10,
    /// CAP-11: native permission posture.
    Cap11,
    /// CAP-12: conversation visibility in Antigravity Desktop.
    Cap12,
}

impl CapId {
    /// All twelve, in order.
    pub const ALL: [CapId; 12] = [
        Self::Cap01,
        Self::Cap02,
        Self::Cap03,
        Self::Cap04,
        Self::Cap05,
        Self::Cap06,
        Self::Cap07,
        Self::Cap08,
        Self::Cap09,
        Self::Cap10,
        Self::Cap11,
        Self::Cap12,
    ];

    /// The `CAP-NN` label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Cap01 => "CAP-01",
            Self::Cap02 => "CAP-02",
            Self::Cap03 => "CAP-03",
            Self::Cap04 => "CAP-04",
            Self::Cap05 => "CAP-05",
            Self::Cap06 => "CAP-06",
            Self::Cap07 => "CAP-07",
            Self::Cap08 => "CAP-08",
            Self::Cap09 => "CAP-09",
            Self::Cap10 => "CAP-10",
            Self::Cap11 => "CAP-11",
            Self::Cap12 => "CAP-12",
        }
    }

    /// The port-level capability this maps to.
    pub fn port_capability(self) -> AgentCapability {
        match self {
            Self::Cap01 => AgentCapability::Discovery,
            Self::Cap02 => AgentCapability::FreshSession,
            Self::Cap03 => AgentCapability::TaskDelivery,
            Self::Cap04 => AgentCapability::LifecycleObservation,
            Self::Cap05 => AgentCapability::Cancel,
            Self::Cap06 => AgentCapability::StatusQuery,
            Self::Cap07 => AgentCapability::ConcurrencyLimit,
            Self::Cap08 => AgentCapability::SessionIdentity,
            Self::Cap09 => AgentCapability::CapacityReporting,
            Self::Cap10 => AgentCapability::Resume,
            Self::Cap11 => AgentCapability::PostureInspection,
            Self::Cap12 => AgentCapability::ConversationVisibility,
        }
    }
}

/// Whether the installed environment provides the capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    /// Evidence says yes.
    Supported,
    /// Evidence says no.
    Unsupported,
    /// Nothing proves it either way yet (a spike or later ticket settles it).
    Unverified,
}

/// What a missing or unverified capability costs, exactly as `ADAPTERS.md` specifies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing to report.
    Ready,
    /// No run can start (preflight `BLOCK`).
    Block,
    /// Autonomous runs are blocked; supervised runs continue (with acknowledgement where the specification asks).
    BlockAutonomous,
    /// The feature degrades; the text says how.
    Degraded(&'static str),
    /// A release-gating product requirement that cannot be claimed yet.
    ReleaseGating,
}

/// The specified outcome of a capability that is `support`ed or not. This is the single table the report and the tests use.
pub fn outcome_for(id: CapId, support: Support) -> Outcome {
    if support == Support::Supported {
        return Outcome::Ready;
    }
    match id {
        CapId::Cap01 | CapId::Cap02 | CapId::Cap03 | CapId::Cap04 | CapId::Cap06 => Outcome::Block,
        CapId::Cap05 | CapId::Cap11 => Outcome::BlockAutonomous,
        CapId::Cap07 => Outcome::Degraded("Vela runs one session at a time and reports it"),
        CapId::Cap08 => Outcome::Degraded(
            "UI-automation delivery is unavailable; approvals become interventions",
        ),
        CapId::Cap09 => Outcome::Degraded("capacity handling pauses on any unclassified failure"),
        CapId::Cap10 => Outcome::Degraded("recovery uses a new session at the last checkpoint"),
        CapId::Cap12 => Outcome::ReleaseGating,
    }
}

/// Sign-in state, from the CLI's own answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthState {
    /// `agy models` answered with a model list.
    Authenticated,
    /// The CLI says the user must sign in.
    Unauthenticated(String),
    /// The probe could not tell (offline, unexpected output).
    Unknown(String),
}

/// One row of the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityResult {
    /// Which capability.
    pub id: CapId,
    /// Whether it is provided.
    pub support: Support,
    /// What follows from that.
    pub outcome: Outcome,
    /// The evidence, in a sentence.
    pub evidence: String,
}

/// The CLI installation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Installation {
    /// The version string (`1.3.3`), when the CLI runs.
    pub version: Option<String>,
    /// Long flags found in `--help`, without the dashes' arguments (`--output-format`).
    pub flags: BTreeSet<String>,
    /// Subcommands found in `--help`.
    pub subcommands: BTreeSet<String>,
    /// True when `--help` says `stream-json` is an output format.
    pub stream_json: bool,
}

/// The Antigravity Desktop installation, when one is found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DesktopInstall {
    /// True when an installation folder was found.
    pub found: bool,
}

/// The whole capability report (a snapshot the caller may persist).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityReport {
    /// The CLI.
    pub installation: Installation,
    /// Sign-in.
    pub auth: AuthState,
    /// Desktop.
    pub desktop: DesktopInstall,
    /// CAP-01..CAP-12.
    pub results: Vec<CapabilityResult>,
}

/// Severity of a preflight finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingSeverity {
    /// Stops the run from starting.
    Blocking,
    /// Reported, does not stop a supervised run.
    Warning,
}

/// A finding the preflight engine can show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightFinding {
    /// Stable identifier (`antigravity.cap-04`, `antigravity.auth`).
    pub id: String,
    /// How serious.
    pub severity: FindingSeverity,
    /// What to tell the user.
    pub message: String,
}

impl CapabilityReport {
    /// True when no required capability or the sign-in blocks a run from starting.
    pub fn can_start_runs(&self) -> bool {
        !self.results.iter().any(|r| r.outcome == Outcome::Block)
            && !matches!(self.auth, AuthState::Unauthenticated(_))
    }

    /// The row for `id`.
    pub fn result(&self, id: CapId) -> Option<&CapabilityResult> {
        self.results.iter().find(|r| r.id == id)
    }

    /// The error that stops a run, if any: `AUTH_REQUIRED` for sign-in, `CAPABILITY_MISSING` for a blocking capability.
    pub fn blocking_error(&self) -> Option<VelaError> {
        if let AuthState::Unauthenticated(text) = &self.auth {
            return Some(VelaError::from_code(
                ErrorCode::AuthRequired,
                format!("Antigravity needs you to sign in: {text}"),
            ));
        }
        let first = self.results.iter().find(|r| r.outcome == Outcome::Block)?;
        Some(VelaError::from_code(
            ErrorCode::CapabilityMissing,
            format!("{} is not available: {}", first.id.label(), first.evidence),
        ))
    }

    /// The findings the preflight engine shows (the check provider K10 registers).
    pub fn preflight_findings(&self) -> Vec<PreflightFinding> {
        let mut out = Vec::new();
        match &self.auth {
            AuthState::Unauthenticated(text) => out.push(PreflightFinding {
                id: "antigravity.auth".to_owned(),
                severity: FindingSeverity::Blocking,
                message: format!("Sign in to Antigravity (run `agy` once): {text}"),
            }),
            AuthState::Unknown(why) => out.push(PreflightFinding {
                id: "antigravity.auth".to_owned(),
                severity: FindingSeverity::Warning,
                message: format!("Vela could not confirm you are signed in to Antigravity: {why}"),
            }),
            AuthState::Authenticated => {}
        }
        for r in &self.results {
            let severity = match r.outcome {
                Outcome::Ready => continue,
                Outcome::Block => FindingSeverity::Blocking,
                Outcome::BlockAutonomous | Outcome::Degraded(_) | Outcome::ReleaseGating => {
                    FindingSeverity::Warning
                }
            };
            out.push(PreflightFinding {
                id: format!("antigravity.{}", r.id.label().to_ascii_lowercase()),
                severity,
                message: format!("{}: {}", r.id.label(), r.evidence),
            });
        }
        out
    }
}

/// Parses `agy --help` (a Go flag listing): long and short flags, and the subcommand names under "Available subcommands".
pub fn parse_help(help: &str) -> Installation {
    let mut flags = BTreeSet::new();
    let mut subcommands = BTreeSet::new();
    let mut in_subcommands = false;
    for line in help.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("Available subcommands") {
            in_subcommands = true;
            continue;
        }
        if in_subcommands {
            if let Some(name) = trimmed.split_whitespace().next() {
                subcommands.insert(name.to_owned());
            }
        } else if trimmed.starts_with('-')
            && let Some(flag) = trimmed.split_whitespace().next()
        {
            flags.insert(flag.to_owned());
        }
    }
    Installation {
        version: None,
        flags,
        subcommands,
        stream_json: help.contains("stream-json"),
    }
}

/// Builds the report from what the probe observed. Pure, so it is testable against recorded outputs.
pub fn build_report(
    installation: Installation,
    auth: AuthState,
    desktop: DesktopInstall,
) -> CapabilityReport {
    let installed = installation.version.is_some();
    let has = |f: &str| installation.flags.contains(f);
    let rows: [(CapId, bool, bool, String); 12] = [
        (
            CapId::Cap01,
            installed,
            false,
            match &installation.version {
                Some(v) => format!("agy {v} is installed"),
                None => "the agy command-line tool is not installed or does not start".to_owned(),
            },
        ),
        (
            CapId::Cap02,
            installed && has("--print"),
            false,
            "a headless turn runs in the working directory with `agy --print`".to_owned(),
        ),
        (
            CapId::Cap03,
            installed && has("--print"),
            false,
            "the task is delivered with `agy --print`; skills expand as slash commands in print mode".to_owned(),
        ),
        (
            CapId::Cap04,
            installed && installation.stream_json,
            false,
            "`--output-format stream-json` reports init, step and result events".to_owned(),
        ),
        (
            CapId::Cap05,
            installed,
            false,
            "Vela ends a session by terminating its process tree (verified)".to_owned(),
        ),
        (
            CapId::Cap06,
            installed && installation.stream_json,
            false,
            "status is derived from the event stream".to_owned(),
        ),
        (
            CapId::Cap07,
            false,
            true,
            "the provider documents no concurrency limit; two parallel sessions ran in a probe".to_owned(),
        ),
        (
            CapId::Cap08,
            installed && installation.stream_json,
            false,
            "the init event carries the conversation id and working directory".to_owned(),
        ),
        (
            CapId::Cap09,
            false,
            true,
            "capacity arrives only as human-readable error text".to_owned(),
        ),
        (
            CapId::Cap10,
            installed && has("--conversation"),
            false,
            "a conversation resumes with `--conversation <id>` (verified)".to_owned(),
        ),
        (
            CapId::Cap11,
            false,
            true,
            "the native permission posture is checked by the posture probe (not yet run)".to_owned(),
        ),
        (
            CapId::Cap12,
            false,
            true,
            "whether headless conversations open in Antigravity Desktop is settled by spike S-DESKTOP-VISIBILITY".to_owned(),
        ),
    ];
    let results = rows
        .into_iter()
        .map(|(id, supported, unverified, evidence)| {
            let support = if supported {
                Support::Supported
            } else if unverified {
                Support::Unverified
            } else {
                Support::Unsupported
            };
            CapabilityResult {
                id,
                support,
                outcome: outcome_for(id, support),
                evidence,
            }
        })
        .collect();
    CapabilityReport {
        installation,
        auth,
        desktop,
        results,
    }
}

/// Judges the sign-in state from the output of `agy models`.
pub fn auth_from_models(exit_code: Option<i32>, stdout: &str, stderr: &str) -> AuthState {
    if let Some(TurnClass::AuthRequired { text }) = classify_text(&format!("{stderr}\n{stdout}")) {
        return AuthState::Unauthenticated(text);
    }
    let listed = stdout
        .lines()
        .filter(|l| l.contains('\t') || l.to_ascii_lowercase().contains("gemini"))
        .count();
    match (exit_code, listed) {
        (Some(0), n) if n > 0 => AuthState::Authenticated,
        (Some(0), _) => AuthState::Unknown("`agy models` printed no models".to_owned()),
        _ => AuthState::Unknown(
            stderr
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("`agy models` failed")
                .trim()
                .chars()
                .take(200)
                .collect(),
        ),
    }
}

#[derive(Default)]
struct Capture {
    out: Mutex<String>,
    err: Mutex<String>,
}

impl OutputSink for Capture {
    fn on_output(&self, chunk: OutputChunk) {
        let target = match chunk.stream {
            OutputStream::Stdout => &self.out,
            OutputStream::Stderr => &self.err,
        };
        target.lock().expect("capture lock").push_str(&chunk.text);
    }
}

/// Runs the discovery commands.
pub struct CapabilityProbe {
    runner: Arc<dyn ProcessRunner>,
    program: String,
    env: Vec<(String, String)>,
    cwd: WorktreePath,
    next: AtomicU64,
    desktop_dirs: Vec<std::path::PathBuf>,
}

impl CapabilityProbe {
    /// A probe that runs `agy` through `runner` in `cwd` with the given environment allowlist.
    pub fn new(
        runner: Arc<dyn ProcessRunner>,
        env: Vec<(String, String)>,
        cwd: WorktreePath,
    ) -> Self {
        Self {
            runner,
            program: "agy".to_owned(),
            env,
            cwd,
            next: AtomicU64::new(0),
            desktop_dirs: Vec::new(),
        }
    }

    /// Folders to look in for an Antigravity Desktop installation (the composition root supplies the real ones).
    pub fn with_desktop_dirs(mut self, dirs: Vec<std::path::PathBuf>) -> Self {
        self.desktop_dirs = dirs;
        self
    }

    async fn run(&self, args: &[&str]) -> Result<(Option<i32>, String, String), VelaError> {
        let spec = CommandSpec {
            id: format!("agy-probe-{}", self.next.fetch_add(1, Ordering::SeqCst) + 1),
            program: self.program.clone(),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            cwd: self.cwd.clone(),
            env: self.env.clone(),
            timeout_ms: NonZeroU64::new(60_000).expect("non-zero"),
        };
        let sink = Capture::default();
        let outcome = self.runner.run(&spec, &sink).await?;
        Ok((
            outcome.exit_code,
            sink.out.into_inner().expect("capture lock"),
            sink.err.into_inner().expect("capture lock"),
        ))
    }

    /// Discovers everything. A missing `agy` is not an error: it is the report's answer.
    pub async fn probe(&self) -> CapabilityReport {
        let desktop = DesktopInstall {
            found: self.desktop_dirs.iter().any(|d| d.is_dir()),
        };
        let version = match self.run(&["--version"]).await {
            Ok((Some(0), out, _)) => out
                .lines()
                .next()
                .map(|l| l.trim().to_owned())
                .filter(|v| !v.is_empty()),
            _ => None,
        };
        let Some(version) = version else {
            let installation = Installation::default();
            return build_report(
                installation,
                AuthState::Unknown("the agy command-line tool is not installed".to_owned()),
                desktop,
            );
        };
        let mut installation = match self.run(&["--help"]).await {
            Ok((_, out, err)) => parse_help(&format!("{out}\n{err}")),
            Err(_) => Installation::default(),
        };
        installation.version = Some(version);
        let auth = match self.run(&["models"]).await {
            Ok((code, out, err)) => auth_from_models(code, &out, &err),
            Err(e) => AuthState::Unknown(e.what_failed().to_owned()),
        };
        build_report(installation, auth, desktop)
    }
}

impl AgentProfile for AgyDriver {
    fn capabilities(&self) -> PortFuture<'_, AgentCapabilities> {
        Box::pin(async move {
            let report = self.capability_probe().probe().await;
            let supported = report
                .results
                .iter()
                .filter(|r| r.support == Support::Supported)
                .map(|r| r.id.port_capability())
                .collect();
            Ok(AgentCapabilities {
                version: report.installation.version.unwrap_or_default(),
                supported,
                max_concurrent_sessions: None,
            })
        })
    }

    fn permission_posture(&self) -> PortFuture<'_, PermissionPosture> {
        // The posture probe (A06) settles this; until it has run the posture is unknown and autonomous runs stay blocked.
        Box::pin(async { Ok(PermissionPosture::Unknown) })
    }

    fn provision_profile<'a>(&'a self, _session: &'a SessionRef) -> PortFuture<'a, ()> {
        // The per-worker profile manager (A05) replaces this.
        Box::pin(async { Ok(()) })
    }
}

#[cfg(test)]
mod tests;
