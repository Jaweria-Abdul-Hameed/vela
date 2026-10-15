//! ASK handling for headless Antigravity (A07): an operation the policy cannot allow blocks, surfaces as an intervention, and after
//! a human allows it once, the conversation resumes with an exact, expiring allow rule.
//!
//! The hook writes one secret-free line per decision to a spool; this module reads those lines (never rewinding, never re-reading),
//! turns each `ask` into an approval observation, and manages the one-time rule in the worker's isolated profile. A refusal changes
//! nothing: the hook keeps denying, which is exactly what "the block stays in place" means.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Deserialize;
use serde_json::Value;
use vela_domain::errors::VelaError;
use vela_domain::ports::{ApprovalObservation, ApprovalSource, EvidenceTier, PortFuture};

/// One decision line as the hook wrote it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SpoolEntry {
    /// The conversation, when known.
    pub conversation_id: Option<String>,
    /// The step, when known.
    pub step_idx: Option<u64>,
    /// The tool.
    pub tool: String,
    /// `allow`, `ask` or `deny`.
    pub decision: String,
    /// The rule that decided.
    pub rule_id: String,
    /// The operation class.
    pub class: Option<String>,
    /// Where the paths lie.
    pub placement: Option<String>,
    /// The executable's lookup name.
    pub executable: Option<String>,
    /// The hook's fingerprint of the facts.
    pub fingerprint: String,
}

/// Reads the spool directory incrementally.
pub struct SpoolReader {
    dir: PathBuf,
    offsets: Mutex<HashMap<PathBuf, u64>>,
    pending: Mutex<VecDeque<SpoolEntry>>,
}

impl SpoolReader {
    /// A reader over `dir` (the hook's spool folder).
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            offsets: Mutex::new(HashMap::new()),
            pending: Mutex::new(VecDeque::new()),
        }
    }

    /// Reads new complete lines from every spool file; returns all new entries. A partial last line waits for the next poll, and a
    /// line that is not a valid entry is skipped.
    pub fn poll(&self) -> Result<Vec<SpoolEntry>, VelaError> {
        let mut found = Vec::new();
        let Ok(listing) = std::fs::read_dir(&self.dir) else {
            return Ok(found);
        };
        let mut files: Vec<PathBuf> = listing
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
            .collect();
        files.sort();
        let mut offsets = self.offsets.lock().expect("offsets lock");
        for path in files {
            let start = offsets.get(&path).copied().unwrap_or(0);
            let Ok(mut file) = std::fs::File::open(&path) else {
                continue;
            };
            if file.seek(SeekFrom::Start(start)).is_err() {
                continue;
            }
            let mut text = String::new();
            if file.read_to_string(&mut text).is_err() {
                continue;
            }
            let complete = text.rfind('\n').map_or(0, |i| i + 1);
            for line in text[..complete].lines().filter(|l| !l.trim().is_empty()) {
                if let Ok(entry) = serde_json::from_str::<SpoolEntry>(line) {
                    found.push(entry);
                }
            }
            offsets.insert(path, start + complete as u64);
        }
        Ok(found)
    }
}

impl ApprovalSource for SpoolReader {
    fn next_observation(&self) -> PortFuture<'_, Option<ApprovalObservation>> {
        Box::pin(async move {
            let fresh = self.poll()?;
            let mut pending = self.pending.lock().expect("pending lock");
            pending.extend(fresh.into_iter().filter(|e| e.decision == "ask"));
            Ok(pending.pop_front().map(|e| ApprovalObservation {
                source_id: "hook-spool".to_owned(),
                session_identity: e.conversation_id.clone(),
                // The spool deliberately has no command line; the executable is what can be said without risking a secret.
                operation: e.executable.clone().unwrap_or_else(|| e.tool.clone()),
                working_directory: None,
                options: vec!["allow once".to_owned(), "deny".to_owned()],
                // A line the hook wrote is bound to its conversation and step.
                evidence: if e.conversation_id.is_some() && e.step_idx.is_some() {
                    EvidenceTier::Bound
                } else {
                    EvidenceTier::Observed
                },
            }))
        })
    }
}

/// An exact allow rule that expires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnceRule {
    /// The native rule text.
    pub rule: String,
    /// When it stops applying (epoch milliseconds).
    pub expires_ms: i64,
}

/// The rule for allowing `executable` once. Executable names that could be read as syntax are refused.
pub fn once_rule(executable: &str, now_ms: i64, ttl_ms: i64) -> Option<OnceRule> {
    vela_domain::policy::bare_executable_rule(executable).map(|rule| OnceRule {
        rule,
        expires_ms: now_ms + ttl_ms,
    })
}

/// The rule for allowing exactly `command` once. Only a plain command line is accepted: anything with shell syntax, quoting or
/// grouping is refused, because an exact native rule for it could not be written without ambiguity.
pub fn once_command_rule(command: &str, now_ms: i64, ttl_ms: i64) -> Option<OnceRule> {
    let parts: Vec<&str> = command.split_whitespace().collect();
    let plain = |p: &str| {
        !p.is_empty()
            && p.chars().all(|c| {
                c.is_ascii_alphanumeric()
                    || matches!(
                        c,
                        '-' | '_' | '.' | '/' | '\\' | ':' | '=' | '+' | ',' | '@'
                    )
            })
    };
    (!parts.is_empty() && !parts[0].starts_with("regex:") && parts.iter().all(|p| plain(p))).then(
        || OnceRule {
            rule: format!("command({})", parts.join(" ")),
            expires_ms: now_ms + ttl_ms,
        },
    )
}

/// The first word of a command line as the policy names the executable: no folder, no extension, lower case.
fn lookup_name(command: &str) -> String {
    let first = command.split_whitespace().next().unwrap_or("");
    let file = first
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(first)
        .to_ascii_lowercase();
    for ext in [".exe", ".cmd", ".bat", ".com"] {
        if let Some(stem) = file.strip_suffix(ext) {
            return stem.to_owned();
        }
    }
    file
}

/// What the person is asked about, from the hook's own records and the turn: for each executable the hook asked about, the command
/// line the agent tried (taken from the tool step the CLI reported, never from the agent's words), or the bare executable when no
/// step names it. Entries that were not asks are ignored.
pub fn asked_commands(
    entries: &[SpoolEntry],
    steps: &[crate::antigravity::events::ToolStepReport],
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen_exe: Vec<String> = Vec::new();
    for e in entries.iter().filter(|e| e.decision == "ask") {
        let Some(exe) = e.executable.as_deref().map(str::to_ascii_lowercase) else {
            continue;
        };
        if seen_exe.contains(&exe) {
            continue;
        }
        seen_exe.push(exe.clone());
        let tried: Vec<&str> = steps
            .iter()
            .filter(|s| s.tool == "run_command")
            .filter_map(|s| s.command.as_deref())
            .filter(|c| lookup_name(c) == exe)
            .collect();
        if tried.is_empty() {
            out.push(exe);
        } else {
            for c in tried {
                let c = c.trim().to_owned();
                if !out.contains(&c) {
                    out.push(c);
                }
            }
        }
    }
    out
}

/// The one-time rule for what the person approved: an exact command line when it is one, an executable name otherwise.
pub fn grant_rule(what: &str, now_ms: i64, ttl_ms: i64) -> Option<OnceRule> {
    if what.split_whitespace().count() > 1 {
        once_command_rule(what, now_ms, ttl_ms)
    } else {
        once_rule(what, now_ms, ttl_ms)
    }
}

fn read_json(path: &Path) -> Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

fn write_json(path: &Path, v: &Value) -> Result<(), VelaError> {
    std::fs::write(path, serde_json::to_string_pretty(v).unwrap_or_default())
        .map_err(|e| VelaError::internal(format!("cannot write {}: {e}", path.display())))
}

/// Adds the rule to the profile's `settings.json` and remembers its expiry in `ledger` (a Vela-owned sidecar file).
pub fn install_once_rule(settings: &Path, ledger: &Path, rule: &OnceRule) -> Result<(), VelaError> {
    let mut s = read_json(settings);
    let allow = s
        .pointer_mut("/permissions/allow")
        .and_then(Value::as_array_mut);
    match allow {
        Some(list) => {
            if !list.iter().any(|v| v.as_str() == Some(&rule.rule)) {
                list.push(Value::String(rule.rule.clone()));
            }
        }
        None => {
            s["permissions"]["allow"] = serde_json::json!([rule.rule]);
        }
    }
    write_json(settings, &s)?;
    let mut l = read_json(ledger);
    l[&rule.rule] = Value::from(rule.expires_ms);
    write_json(ledger, &l)
}

/// Removes every expired one-time rule from `settings`. Rules Vela did not add are never touched. Returns what was removed.
pub fn sweep_expired(
    settings: &Path,
    ledger: &Path,
    now_ms: i64,
) -> Result<Vec<String>, VelaError> {
    let mut l = read_json(ledger);
    let expired: Vec<String> = l
        .as_object()
        .map(|m| {
            m.iter()
                .filter(|(_, v)| v.as_i64().is_some_and(|e| e <= now_ms))
                .map(|(k, _)| k.clone())
                .collect()
        })
        .unwrap_or_default();
    if expired.is_empty() {
        return Ok(expired);
    }
    let mut s = read_json(settings);
    if let Some(list) = s
        .pointer_mut("/permissions/allow")
        .and_then(Value::as_array_mut)
    {
        list.retain(|v| !v.as_str().is_some_and(|t| expired.iter().any(|e| e == t)));
    }
    write_json(settings, &s)?;
    for e in &expired {
        if let Some(m) = l.as_object_mut() {
            m.remove(e);
        }
    }
    write_json(ledger, &l)?;
    Ok(expired)
}

/// What the resumed conversation is told after an allow-once.
pub fn resume_prompt(executable: &str) -> String {
    format!(
        "The operation `{executable}` was approved once by the user. Retry it now and continue with the ticket."
    )
}

/// The per-class counts of a batch of entries (for the UI's posture panel).
pub fn count_by_decision(entries: &[SpoolEntry]) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for e in entries {
        *out.entry(e.decision.clone()).or_insert(0) += 1;
    }
    out
}

#[cfg(test)]
mod tests;
