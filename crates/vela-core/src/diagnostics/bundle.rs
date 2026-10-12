//! Diagnostic bundle (X03): version, non-secret settings, run state, journal, capabilities, redacted logs and a Git ref
//! summary. Everything is redacted again at export, and no repository source or issue text is ever collected: the only
//! inputs are the ones named here.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};
use vela_domain::errors::{ErrorClass, VelaError};
use vela_persistence::journal;

use super::{SharedRedaction, redact_payload_text};
use crate::orchestrator::Services;
use crate::settings::SettingsService;

/// Largest journal tail included.
pub const JOURNAL_TAIL: u32 = 2000;
/// Largest size of any one log in the bundle, in bytes (the newest part is kept).
pub const LOG_CAP_BYTES: usize = 256 * 1024;

/// What the caller supplies; nothing here is read from a repository.
#[derive(Debug, Clone, Default)]
pub struct BundleInputs {
    /// Application version.
    pub version: String,
    /// The capability report, as JSON.
    pub capabilities: Value,
    /// `(name, text)` log files, already collected by the caller.
    pub logs: Vec<(String, String)>,
    /// Git ref summary lines (`refs/heads/x <sha>`), never file contents.
    pub git_refs: Vec<String>,
}

/// The bundle: file name to text.
#[derive(Debug, Clone, PartialEq)]
pub struct Bundle {
    /// Files by relative name.
    pub files: BTreeMap<String, String>,
}

/// Keeps the newest `cap` bytes of `text`, on a character boundary.
pub fn cap_log(text: &str, cap: usize) -> String {
    if text.len() <= cap {
        return text.to_owned();
    }
    let mut start = text.len() - cap;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    format!("[older output omitted]\n{}", &text[start..])
}

/// Builds the bundle.
pub fn build(
    services: &Services,
    redactor: &SharedRedaction,
    inputs: &BundleInputs,
) -> Result<Bundle, VelaError> {
    let settings = SettingsService::new(services).effective(None)?;
    let runs: Vec<Value> = services.store.read(|c| {
        let mut stmt = c
            .prepare("SELECT id, project_id, state FROM build_runs ORDER BY created_ms")
            .map_err(|e| VelaError::internal(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| {
                Ok(json!({"id": r.get::<_, String>(0)?, "project": r.get::<_, String>(1)?, "state": r.get::<_, String>(2)?}))
            })
            .map_err(|e| VelaError::internal(e.to_string()))?;
        Ok(rows.filter_map(Result::ok).collect())
    })?;
    let tip = services.store.read(journal::tip)?;
    let events = services.store.read(|c| {
        journal::read_after(c, tip.saturating_sub(u64::from(JOURNAL_TAIL)), JOURNAL_TAIL)
    })?;
    let journal_lines: Vec<String> = events
        .iter()
        .map(|e| serde_json::to_string(e).unwrap_or_default())
        .collect();

    let mut files = BTreeMap::new();
    files.insert("version.txt".to_owned(), inputs.version.clone());
    files.insert(
        "settings.json".to_owned(),
        serde_json::to_string_pretty(&settings).unwrap_or_default(),
    );
    files.insert(
        "runs.json".to_owned(),
        serde_json::to_string_pretty(&runs).unwrap_or_default(),
    );
    files.insert("journal.jsonl".to_owned(), journal_lines.join("\n"));
    files.insert(
        "capabilities.json".to_owned(),
        serde_json::to_string_pretty(&inputs.capabilities).unwrap_or_default(),
    );
    files.insert("git-refs.txt".to_owned(), inputs.git_refs.join("\n"));
    for (name, text) in &inputs.logs {
        let safe: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        files.insert(format!("logs/{safe}"), cap_log(text, LOG_CAP_BYTES));
    }
    // Re-redaction at export: every file, whatever its source.
    for text in files.values_mut() {
        *text = redact_payload_text(redactor.as_ref(), text);
    }
    Ok(Bundle { files })
}

/// Writes the bundle into a new folder `dir` (which must not exist yet) and returns the number of files written.
pub fn write_to(bundle: &Bundle, dir: &Path) -> Result<usize, VelaError> {
    if dir.exists() {
        return Err(VelaError::of_class(
            ErrorClass::UserActionRequired,
            "the export folder already exists",
        )
        .with_user_action("choose a new folder name"));
    }
    for (name, text) in &bundle.files {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| VelaError::internal(format!("cannot create {parent:?}: {e}")))?;
        }
        std::fs::write(&path, text)
            .map_err(|e| VelaError::internal(format!("cannot write {path:?}: {e}")))?;
    }
    Ok(bundle.files.len())
}

#[cfg(test)]
mod tests;
