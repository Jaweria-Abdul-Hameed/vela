//! The isolated per-worker profile (A05, ADR-018, spikes S-HOOK-GLOBAL and S-NATIVE-POSTURE).
//!
//! Each worker gets a Vela-owned directory that the `agy` child process sees as its home (`USERPROFILE` and `HOME`). In it
//! Vela generates the native allow rules (`settings.json`), the global `PreToolUse` hook file that runs `vela-hook`, a copy of
//! the hook binary, the rule table and the spool directory. Authentication is untouched: it lives in the operating system's
//! credential store, which the child still reaches (verified), so Vela never reads, copies or references a credential.
//!
//! Layout and facts verified against agy 1.3.3 (see `docs/research/SPIKE_S-HOOK-GLOBAL.md`):
//! - rules live in `<profile>/.gemini/antigravity-cli/settings.json` as `{"permissions":{"allow":["command(git status)", ...]}}`;
//! - the global hook file is `<profile>/.gemini/config/hooks.json`;
//! - a hook command runs with **the hooks file's folder as its working directory** and is **not parsed by a shell** (quotes are
//!   passed literally), so the command uses only relative, space-free names and everything the hook needs sits next to the
//!   hooks file.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::ids::WorkerId;
use vela_domain::policy::{HookRuleTable, NativeAllowRule, PolicyEngine, native_allow_rules};

/// The hook binary's file name inside the profile (a copy of the built `vela-hook`).
pub const HOOK_EXE: &str = "vela-hook.exe";
/// The rule table's file name next to the hooks file.
pub const TABLE_FILE: &str = "vela-table.json";
/// The spool directory next to the hooks file.
pub const SPOOL_DIR: &str = "spool";
/// The hook timeout in seconds written into the hooks file.
pub const HOOK_TIMEOUT_SECONDS: u32 = 10;

/// A created profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerProfile {
    /// The directory the child sees as its home.
    pub home: PathBuf,
    /// `<home>/.gemini/antigravity-cli/settings.json`.
    pub settings_path: PathBuf,
    /// `<home>/.gemini/config/hooks.json`.
    pub hooks_path: PathBuf,
    /// The rule table the hook reads.
    pub table_path: PathBuf,
    /// Where the hook appends its records and the core reads them.
    pub spool_dir: PathBuf,
}

impl WorkerProfile {
    /// The environment overrides that make the child use this profile.
    pub fn env(&self) -> Vec<(String, String)> {
        let home = self.home.to_string_lossy().into_owned();
        vec![
            ("USERPROFILE".to_owned(), home.clone()),
            ("HOME".to_owned(), home),
        ]
    }
}

/// Renders the native allow rule text, or `None` for an entry the native syntax cannot express safely. Skipping an entry only
/// removes a native allow: the hook still decides every call, so it is never a loosening.
pub fn render_rule(rule: &NativeAllowRule) -> Option<String> {
    match rule {
        NativeAllowRule::Command {
            executable, args, ..
        } => {
            let parts: Vec<&str> = std::iter::once(executable.as_str())
                .chain(args.iter().map(String::as_str))
                .collect();
            let ambiguous = |s: &str| {
                s.is_empty()
                    || s.chars()
                        .any(|c| c.is_whitespace() || matches!(c, '(' | ')' | '"' | '\'' | '`'))
            };
            if parts.iter().any(|p| ambiguous(p)) || executable.starts_with("regex:") {
                return None;
            }
            Some(format!("command({})", parts.join(" ")))
        }
        NativeAllowRule::WriteWithin { root, .. } => {
            if root.is_empty() || root.chars().any(|c| matches!(c, '(' | ')')) {
                return None;
            }
            // Verified form: backslashes, no trailing separator.
            Some(format!(
                "write_file({})",
                root.replace('/', "\\").trim_end_matches('\\')
            ))
        }
    }
}

/// The generated `settings.json` content for `rules`.
pub fn settings_json(rules: &[NativeAllowRule]) -> Value {
    let allow: Vec<String> = rules.iter().filter_map(render_rule).collect();
    json!({ "permissions": { "allow": allow } })
}

/// The generated global hooks file. The command is relative and space-free on purpose (see the module documentation).
pub fn hooks_json() -> Value {
    json!({
        "vela": {
            "PreToolUse": [{
                "matcher": "*",
                "hooks": [{
                    "type": "command",
                    "command": format!("{HOOK_EXE} --table {TABLE_FILE} --spool {SPOOL_DIR}"),
                    "timeout": HOOK_TIMEOUT_SECONDS
                }]
            }]
        }
    })
}

fn io_err(what: &str, e: std::io::Error) -> VelaError {
    VelaError::of_class(ErrorClass::ExternalTemporary, format!("{what}: {e}"))
}

fn safe_component(s: &str) -> Result<&str, VelaError> {
    let ok = !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && !s.starts_with('.');
    if ok {
        Ok(s)
    } else {
        Err(VelaError::of_class(
            ErrorClass::ValidationFailure,
            "the run or worker id is not usable as a folder name",
        ))
    }
}

/// Creates and removes worker profiles under one Vela-owned root.
#[derive(Debug, Clone)]
pub struct ProfileManager {
    root: PathBuf,
    hook_exe: PathBuf,
}

impl ProfileManager {
    /// Profiles are created below `root`; `hook_exe` is the built `vela-hook` binary that gets copied into each profile.
    pub fn new(root: PathBuf, hook_exe: PathBuf) -> Self {
        Self { root, hook_exe }
    }

    fn run_dir(&self, run: &str) -> Result<PathBuf, VelaError> {
        Ok(self.root.join(safe_component(run)?))
    }

    /// The profile directory of a worker (whether or not it exists).
    pub fn home_of(&self, run: &str, worker: &WorkerId) -> Result<PathBuf, VelaError> {
        Ok(self.run_dir(run)?.join(safe_component(worker.as_str())?))
    }

    /// Where the worker's profile files are (whether or not it exists yet); nothing is written.
    pub fn locate(&self, run: &str, worker: &WorkerId) -> Result<WorkerProfile, VelaError> {
        let home = self.home_of(run, worker)?;
        let config_dir = home.join(".gemini").join("config");
        Ok(WorkerProfile {
            settings_path: home
                .join(".gemini")
                .join("antigravity-cli")
                .join("settings.json"),
            hooks_path: config_dir.join("hooks.json"),
            table_path: config_dir.join(TABLE_FILE),
            spool_dir: config_dir.join(SPOOL_DIR),
            home,
        })
    }

    /// Creates the worker's profile from `engine`: allow rules (trusted repositories only), the hook file, a copy of the hook
    /// binary, the rule table, and an empty spool. An existing profile for the worker is replaced.
    pub fn create(
        &self,
        run: &str,
        worker: &WorkerId,
        engine: &PolicyEngine,
    ) -> Result<WorkerProfile, VelaError> {
        let home = self.home_of(run, worker)?;
        if home.exists() {
            std::fs::remove_dir_all(&home)
                .map_err(|e| io_err("could not replace the old worker profile", e))?;
        }
        let settings_dir = home.join(".gemini").join("antigravity-cli");
        let config_dir = home.join(".gemini").join("config");
        for dir in [&settings_dir, &config_dir, &config_dir.join(SPOOL_DIR)] {
            std::fs::create_dir_all(dir)
                .map_err(|e| io_err("could not create the worker profile", e))?;
        }
        let write = |path: &Path, value: &Value| {
            std::fs::write(path, serde_json::to_vec_pretty(value).unwrap_or_default())
                .map_err(|e| io_err("could not write a profile file", e))
        };
        let settings_path = settings_dir.join("settings.json");
        write(&settings_path, &settings_json(&native_allow_rules(engine)))?;
        let hooks_path = config_dir.join("hooks.json");
        write(&hooks_path, &hooks_json())?;
        let table_path = config_dir.join(TABLE_FILE);
        let table =
            serde_json::to_vec_pretty(&HookRuleTable::from_engine(engine)).map_err(|e| {
                VelaError::internal(format!("the rule table could not be encoded: {e}"))
            })?;
        std::fs::write(&table_path, table)
            .map_err(|e| io_err("could not write the rule table", e))?;
        std::fs::copy(&self.hook_exe, config_dir.join(HOOK_EXE))
            .map_err(|e| io_err("could not copy the hook binary into the profile", e))?;
        Ok(WorkerProfile {
            home,
            settings_path,
            hooks_path,
            table_path,
            spool_dir: config_dir.join(SPOOL_DIR),
        })
    }

    /// Removes one worker's profile (when the worker ends). A missing profile is not an error.
    pub fn remove(&self, run: &str, worker: &WorkerId) -> Result<(), VelaError> {
        let home = self.home_of(run, worker)?;
        match std::fs::remove_dir_all(&home) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io_err("could not remove the worker profile", e)),
        }
    }

    /// Removes every profile of a run (when the run ends).
    pub fn remove_run(&self, run: &str) -> Result<(), VelaError> {
        let dir = self.run_dir(run)?;
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io_err("could not remove the run's profiles", e)),
        }
    }

    /// Removes the profiles of every run not in `active_runs` (startup and recovery: no profile outlives its run). Returns the
    /// run folders removed.
    pub fn sweep(&self, active_runs: &HashSet<String>) -> Result<Vec<String>, VelaError> {
        let mut removed = Vec::new();
        let entries = match std::fs::read_dir(&self.root) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(removed),
            Err(e) => return Err(io_err("could not list the profiles", e)),
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !active_runs.contains(&name) && entry.path().is_dir() {
                std::fs::remove_dir_all(entry.path())
                    .map_err(|e| io_err("could not remove a stale profile", e))?;
                removed.push(name);
            }
        }
        removed.sort();
        Ok(removed)
    }
}

#[cfg(test)]
mod tests;
