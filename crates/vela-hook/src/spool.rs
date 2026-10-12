//! The spool: one secret-free JSON line per decision, appended to `<spool>/<conversation>.jsonl`.
//!
//! The core (A07) reads these lines to learn what the policy decided and, for an `ask`, which approval is pending. A record
//! carries the tool, the decision, the rule, the operation class and placement, the executable's lookup name, and a
//! fingerprint of those facts. It never carries a command line, an argument, a path or a file's content, because any of
//! those can hold a secret; the hook has no redactor, so it keeps them out entirely.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The longest conversation id used in a file name.
const MAX_NAME: usize = 64;

/// One decision, as recorded.
#[derive(Debug, Clone, PartialEq)]
pub struct SpoolRecord {
    /// The conversation id, when given.
    pub conversation_id: Option<String>,
    /// The step index, when given.
    pub step_idx: Option<u64>,
    /// The tool name.
    pub tool: String,
    /// `allow`, `ask` or `deny`.
    pub decision: &'static str,
    /// The rule (or hook check) that decided.
    pub rule_id: String,
    /// The operation class, when the engine classified one.
    pub class: Option<String>,
    /// Where the paths lie relative to the worktree.
    pub placement: Option<String>,
    /// The executable's lookup name, for commands.
    pub executable: Option<String>,
}

/// A file-name-safe form of a conversation id.
pub fn file_stem(conversation_id: Option<&str>) -> String {
    let cleaned: String = conversation_id
        .unwrap_or("unknown")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .take(MAX_NAME)
        .collect();
    if cleaned.is_empty() {
        "unknown".to_owned()
    } else {
        cleaned
    }
}

/// FNV-1a over the identifying facts, as 16 hex digits.
pub fn fingerprint(record: &SpoolRecord) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for part in [
        record.tool.as_str(),
        record.class.as_deref().unwrap_or(""),
        record.placement.as_deref().unwrap_or(""),
        record.executable.as_deref().unwrap_or(""),
    ] {
        for b in part.bytes().chain(std::iter::once(0xff)) {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    format!("{h:016x}")
}

fn quote(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_owned())
}

fn optional(v: Option<&str>) -> String {
    v.map_or_else(|| "null".to_owned(), quote)
}

/// The JSON line for a record (no newline).
pub fn to_line(record: &SpoolRecord, ts_ms: u128) -> String {
    format!(
        "{{\"v\":1,\"ts_ms\":{ts_ms},\"conversation_id\":{},\"step_idx\":{},\"tool\":{},\"decision\":{},\"rule_id\":{},\"class\":{},\"placement\":{},\"executable\":{},\"fingerprint\":{}}}",
        optional(record.conversation_id.as_deref()),
        record
            .step_idx
            .map_or_else(|| "null".to_owned(), |n| n.to_string()),
        quote(&record.tool),
        quote(record.decision),
        quote(&record.rule_id),
        optional(record.class.as_deref()),
        optional(record.placement.as_deref()),
        optional(record.executable.as_deref()),
        quote(&fingerprint(record)),
    )
}

/// Appends the record to its conversation's file, creating the spool directory if needed.
pub fn append(dir: &Path, record: &SpoolRecord) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!(
        "{}.jsonl",
        file_stem(record.conversation_id.as_deref())
    ));
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    // One write call per line keeps concurrent hooks from interleaving within a line.
    file.write_all(format!("{}\n", to_line(record, ts)).as_bytes())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> SpoolRecord {
        SpoolRecord {
            conversation_id: Some("c-1".to_owned()),
            step_idx: Some(3),
            tool: "run_command".to_owned(),
            decision: "ask",
            rule_id: "vela.default.unknown".to_owned(),
            class: Some("unknown".to_owned()),
            placement: Some("inside_root".to_owned()),
            executable: Some("curl".to_owned()),
        }
    }

    #[test]
    fn lines_are_valid_json_with_the_documented_fields_and_no_operation_text() {
        let line = to_line(&record(), 1234);
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        for key in [
            "v",
            "ts_ms",
            "conversation_id",
            "step_idx",
            "tool",
            "decision",
            "rule_id",
            "class",
            "placement",
            "executable",
            "fingerprint",
        ] {
            assert!(v.get(key).is_some(), "{key}");
        }
        assert_eq!(v["decision"], "ask");
        assert_eq!(v["fingerprint"].as_str().unwrap().len(), 16);
        assert!(!line.contains('\n'));
    }

    #[test]
    fn file_names_cannot_escape_the_spool_directory() {
        assert_eq!(file_stem(Some("../../etc/passwd")), "etcpasswd");
        assert_eq!(file_stem(Some("c:\\x\\y")), "cxy");
        assert_eq!(file_stem(Some("")), "unknown");
        assert_eq!(file_stem(None), "unknown");
        assert_eq!(file_stem(Some(&"a".repeat(500))).len(), MAX_NAME);
    }

    #[test]
    fn the_fingerprint_depends_only_on_the_identifying_facts() {
        let mut other = record();
        other.step_idx = Some(99);
        other.rule_id = "different".to_owned();
        assert_eq!(fingerprint(&record()), fingerprint(&other));
        other.executable = Some("git".to_owned());
        assert_ne!(fingerprint(&record()), fingerprint(&other));
    }

    #[test]
    fn append_writes_one_line_per_decision() {
        let dir = std::env::temp_dir().join(format!("vela-hook-spool-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = append(&dir, &record()).unwrap();
        append(&dir, &record()).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 2);
        assert!(path.starts_with(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
