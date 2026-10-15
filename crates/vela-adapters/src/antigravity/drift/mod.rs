//! Antigravity recovery and version drift (A14). The official CLI updates itself in the background, and a crash can leave `agy`
//! processes, per-worker profile directories, spool files and allow rules behind. This module decides, from facts it is handed,
//! what to do about each; it never reads or copies a credential (ADR-016) and never touches an Antigravity process or session
//! Vela did not create.
//!
//! * **Version drift**: a run snapshots the installed version, capability set and permission posture; a change at a turn
//!   boundary blocks resume until the capabilities and posture are re-detected and confirmed ([`DriftGuard`]).
//! * **Orphans**: a process is Vela's only if it matches a *recorded identity* (pid, start time and executable name together,
//!   so a reused pid is not mistaken for ours). Matching trees of workers that are not running are cleaned; anything else is
//!   left alone however much it looks like `agy` ([`classify_orphans`]).
//! * **Session identity**: a recorded conversation is resumed only when it, its profile and the version all still check out;
//!   otherwise a new session starts ([`verify_session`]).
//! * **Stale state**: profiles, spool files and expired allow rules of workers that are not running are reported or cleaned,
//!   never silently reused ([`scan_profiles`], [`expired_rules`]).

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use vela_domain::errors::VelaError;

// ------------------------------------------------------------------ version drift

/// What was true of the installation when the run started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionSnapshot {
    /// The `agy --version` text.
    pub agy_version: String,
    /// The Desktop version, when it is installed.
    pub desktop_version: Option<String>,
    /// The capabilities detected (`CAP-01`…).
    pub capabilities: BTreeSet<String>,
    /// A fingerprint of the permission posture probe's result.
    pub posture: String,
}

/// The snapshot a capability report amounts to: the version and the set of capabilities that are provided. The permission posture
/// is probed separately and is not part of this cheap snapshot.
pub fn snapshot_of(report: &super::capability::CapabilityReport) -> VersionSnapshot {
    VersionSnapshot {
        agy_version: report.installation.version.clone().unwrap_or_default(),
        desktop_version: None,
        capabilities: report
            .results
            .iter()
            .filter(|r| r.support == super::capability::Support::Supported)
            .map(|r| r.id.label().to_owned())
            .collect(),
        posture: String::new(),
    }
}

/// One way the installation changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriftReason {
    /// The CLI version changed.
    AgyVersion {
        /// Before.
        was: String,
        /// Now.
        now: String,
    },
    /// The Desktop version changed.
    DesktopVersion,
    /// Capabilities appeared or disappeared.
    Capabilities {
        /// Gone.
        lost: Vec<String>,
        /// New.
        gained: Vec<String>,
    },
    /// The permission posture is not what it was.
    Posture,
}

/// What a turn boundary may do next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gate {
    /// Nothing changed (or the change was confirmed): carry on.
    Proceed,
    /// The installation changed: re-detect capabilities and posture, then [`DriftGuard::confirm`].
    Blocked(Vec<DriftReason>),
}

/// Compares two snapshots.
pub fn diff(was: &VersionSnapshot, now: &VersionSnapshot) -> Vec<DriftReason> {
    let mut out = Vec::new();
    if was.agy_version != now.agy_version {
        out.push(DriftReason::AgyVersion {
            was: was.agy_version.clone(),
            now: now.agy_version.clone(),
        });
    }
    if was.desktop_version != now.desktop_version {
        out.push(DriftReason::DesktopVersion);
    }
    if was.capabilities != now.capabilities {
        out.push(DriftReason::Capabilities {
            lost: was
                .capabilities
                .difference(&now.capabilities)
                .cloned()
                .collect(),
            gained: now
                .capabilities
                .difference(&was.capabilities)
                .cloned()
                .collect(),
        });
    }
    if was.posture != now.posture {
        out.push(DriftReason::Posture);
    }
    out
}

/// The per-run drift guard. Resume is blocked from the moment drift is seen until it is confirmed.
#[derive(Debug, Clone)]
pub struct DriftGuard {
    baseline: VersionSnapshot,
    blocked: Vec<DriftReason>,
}

impl DriftGuard {
    /// A guard over the snapshot taken at run start.
    pub fn new(baseline: VersionSnapshot) -> Self {
        Self {
            baseline,
            blocked: Vec::new(),
        }
    }

    /// Called at a turn boundary with a fresh cheap probe (version only is enough to notice; the full snapshot is better).
    pub fn check(&mut self, current: &VersionSnapshot) -> Gate {
        let reasons = diff(&self.baseline, current);
        if !reasons.is_empty() {
            self.blocked = reasons;
        }
        if self.blocked.is_empty() {
            Gate::Proceed
        } else {
            Gate::Blocked(self.blocked.clone())
        }
    }

    /// The snapshot being compared against.
    pub fn baseline(&self) -> &VersionSnapshot {
        &self.baseline
    }

    /// Whether resume is currently blocked.
    pub fn is_blocked(&self) -> bool {
        !self.blocked.is_empty()
    }

    /// Capabilities and posture were re-detected and accepted: the new snapshot becomes the baseline and resume is unblocked.
    /// `redetected` must be a snapshot from the full probe (not the cheap one), which the caller proves by passing it here.
    pub fn confirm(&mut self, redetected: VersionSnapshot) {
        self.baseline = redetected;
        self.blocked.clear();
    }
}

// ------------------------------------------------------------------ orphans

/// A process as the operating system lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessEntry {
    /// Process id.
    pub pid: u32,
    /// Parent process id.
    pub parent: u32,
    /// Start time, epoch milliseconds.
    pub started_ms: i64,
    /// Executable file name.
    pub exe: String,
}

/// What Vela recorded when it started an `agy` process for a worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedProcess {
    /// The worker it belongs to.
    pub worker: String,
    /// Process id.
    pub pid: u32,
    /// Start time, epoch milliseconds.
    pub started_ms: i64,
    /// Executable file name.
    pub exe: String,
}

/// What to do with a process tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrphanAction {
    /// Vela created it, its worker is not running: terminate the whole tree (the root and these descendants).
    Kill {
        /// The worker.
        worker: String,
        /// The root pid.
        root: u32,
        /// Descendant pids, deepest last.
        descendants: Vec<u32>,
    },
    /// Vela created it and its worker is running: leave it.
    Keep {
        /// The worker.
        worker: String,
        /// The root pid.
        root: u32,
    },
}

/// How far a recorded start time may differ from the observed one (clocks and rounding), milliseconds.
const START_TOLERANCE_MS: i64 = 2_000;

fn same_exe(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Matches recorded identities against the process table. A recorded process matches only when pid, start time and executable
/// name all agree, so a pid the system has since reused is not ours. Unmatched processes, including every `agy` that was not
/// recorded, produce no action at all.
pub fn classify_orphans(
    table: &[ProcessEntry],
    recorded: &[RecordedProcess],
    running_workers: &HashSet<String>,
) -> Vec<OrphanAction> {
    let mut out = Vec::new();
    for rec in recorded {
        let Some(root) = table.iter().find(|p| {
            p.pid == rec.pid
                && same_exe(&p.exe, &rec.exe)
                && (p.started_ms - rec.started_ms).abs() <= START_TOLERANCE_MS
        }) else {
            continue;
        };
        if running_workers.contains(&rec.worker) {
            out.push(OrphanAction::Keep {
                worker: rec.worker.clone(),
                root: root.pid,
            });
            continue;
        }
        let mut descendants = Vec::new();
        let mut frontier = vec![root.pid];
        let mut seen: HashSet<u32> = HashSet::from([root.pid]);
        while let Some(parent) = frontier.pop() {
            for child in table.iter().filter(|p| p.parent == parent) {
                // A child that started before its parent is a reused pid, not a descendant.
                if child.started_ms + START_TOLERANCE_MS >= root.started_ms
                    && seen.insert(child.pid)
                {
                    descendants.push(child.pid);
                    frontier.push(child.pid);
                }
            }
        }
        out.push(OrphanAction::Kill {
            worker: rec.worker.clone(),
            root: root.pid,
            descendants,
        });
    }
    out
}

// ------------------------------------------------------------------ session identity

/// What Vela recorded about a worker's session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedSession {
    /// The conversation id.
    pub conversation_id: String,
    /// The profile home the conversation lives in.
    pub profile_home: PathBuf,
    /// The CLI version it was created under.
    pub agy_version: String,
}

/// What can be observed now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedSession {
    /// The conversation id is still present in that profile's history.
    pub conversation_present: bool,
    /// The profile home exists and is the one recorded.
    pub profile_intact: bool,
    /// The CLI version now.
    pub agy_version: String,
    /// The capability for resuming a conversation (`CAP-10`) is present.
    pub resume_supported: bool,
}

/// Whether to continue the recorded conversation or start a new one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeDecision {
    /// Everything checks out: resume the recorded conversation.
    ResumeSame,
    /// Start a new session on the existing worktree at the last checkpoint, for this reason.
    StartNew(&'static str),
}

/// The decision table. Anything short of a full match starts a new session; a conversation is never assumed.
pub fn verify_session(
    recorded: Option<&RecordedSession>,
    seen: &ObservedSession,
) -> ResumeDecision {
    let Some(rec) = recorded else {
        return ResumeDecision::StartNew("no conversation was recorded");
    };
    if rec.conversation_id.is_empty() {
        return ResumeDecision::StartNew("the recorded conversation id is empty");
    }
    if !seen.resume_supported {
        return ResumeDecision::StartNew("this installation cannot resume a conversation");
    }
    if !seen.profile_intact {
        return ResumeDecision::StartNew("the worker profile is gone or is not the recorded one");
    }
    if !seen.conversation_present {
        return ResumeDecision::StartNew("the conversation is no longer in the profile");
    }
    if rec.agy_version != seen.agy_version {
        return ResumeDecision::StartNew("Antigravity was updated since the conversation began");
    }
    ResumeDecision::ResumeSame
}

// ------------------------------------------------------------------ stale state

/// What kind of leftover.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaleKind {
    /// A worker's profile directory.
    Profile,
    /// Spool files (hook records) inside a profile.
    Spool,
}

/// A leftover of a worker that is not running.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaleArtifact {
    /// What it is.
    pub kind: StaleKind,
    /// The run folder name.
    pub run: String,
    /// The worker folder name.
    pub worker: String,
    /// Where it is.
    pub path: PathBuf,
}

/// Lists the profile directories (`<root>/<run>/<worker>`) of workers that are not running, and the spool directories inside
/// profiles of workers that are not running. A running worker's state is never listed. Sorted for stable reporting.
pub fn scan_profiles(
    root: &Path,
    running: &HashSet<(String, String)>,
    spool_dir_name: &str,
) -> Result<Vec<StaleArtifact>, VelaError> {
    let mut out = Vec::new();
    let runs = match std::fs::read_dir(root) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => {
            return Err(VelaError::internal(format!(
                "could not list the profiles: {e}"
            )));
        }
    };
    for run in runs.flatten().filter(|e| e.path().is_dir()) {
        let run_name = run.file_name().to_string_lossy().into_owned();
        let Ok(workers) = std::fs::read_dir(run.path()) else {
            continue;
        };
        for w in workers.flatten().filter(|e| e.path().is_dir()) {
            let worker = w.file_name().to_string_lossy().into_owned();
            if running.contains(&(run_name.clone(), worker.clone())) {
                continue;
            }
            out.push(StaleArtifact {
                kind: StaleKind::Profile,
                run: run_name.clone(),
                worker: worker.clone(),
                path: w.path(),
            });
            let spool = find_dir(&w.path(), spool_dir_name);
            if let Some(path) = spool {
                out.push(StaleArtifact {
                    kind: StaleKind::Spool,
                    run: run_name.clone(),
                    worker,
                    path,
                });
            }
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn find_dir(base: &Path, name: &str) -> Option<PathBuf> {
    let mut stack = vec![base.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).ok()?.flatten() {
            let p = e.path();
            if p.is_dir() {
                if e.file_name() == name {
                    return Some(p);
                }
                stack.push(p);
            }
        }
    }
    None
}

/// Removes the listed artifacts. Profiles are removed whole (their spool goes with them); a spool listed alone is emptied.
/// Returns what was removed.
pub fn clean(stale: &[StaleArtifact]) -> Result<Vec<PathBuf>, VelaError> {
    let mut removed = Vec::new();
    for a in stale {
        match std::fs::remove_dir_all(&a.path) {
            Ok(()) => removed.push(a.path.clone()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(VelaError::internal(format!(
                    "could not remove {}: {e}",
                    a.path.display()
                )));
            }
        }
    }
    Ok(removed)
}

/// An allow rule with a lifetime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedRule {
    /// The rule's identifier.
    pub id: String,
    /// The worker it was granted to.
    pub worker: String,
    /// When it stops being valid, epoch milliseconds.
    pub expires_ms: i64,
}

/// The rules to drop: expired ones, and every rule of a worker that is not running (a rule never outlives its worker).
pub fn expired_rules(
    rules: &[TimedRule],
    running_workers: &HashSet<String>,
    now_ms: i64,
) -> Vec<String> {
    rules
        .iter()
        .filter(|r| r.expires_ms <= now_ms || !running_workers.contains(&r.worker))
        .map(|r| r.id.clone())
        .collect()
}

#[cfg(test)]
mod tests;
