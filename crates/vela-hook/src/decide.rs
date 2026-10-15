//! The decision: load the rule table, classify the call with the policy engine, record it, answer.
//!
//! Every path that cannot reach a confident engine decision ends in `deny` or `ask`, never `allow`.

use std::path::Path;

use serde::Serialize;
use vela_domain::policy::{Decision, HookRuleTable, Operation, PolicyEngine};

use crate::input::{HookInput, Mapped, parse, to_operation};
use crate::spool::{SpoolRecord, append};

/// The largest rule table the hook reads.
const MAX_TABLE_BYTES: u64 = 4 * 1024 * 1024;

/// What the hook tells the provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerdictKind {
    /// Proceed (the native permission rules still apply).
    Allow,
    /// Needs a human; headless mode cannot prompt, so it is not run.
    Ask,
    /// Refuse.
    Deny,
}

/// The answer in the provider's protocol: `{ "decision": ..., "reason": ... }`.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    /// The decision.
    pub kind: VerdictKind,
    /// The reason shown to the agent and the user. Never contains arguments or paths.
    pub reason: String,
    /// The rule or hook check that decided.
    pub rule_id: String,
}

#[derive(Serialize)]
struct Wire<'a> {
    decision: &'a str,
    reason: &'a str,
}

impl Verdict {
    /// A denial from a hook check.
    pub fn deny(rule_id: &str, reason: &str) -> Self {
        Self {
            kind: VerdictKind::Deny,
            reason: format!("vela-hook: {reason}"),
            rule_id: rule_id.to_owned(),
        }
    }

    fn ask(rule_id: &str, reason: &str) -> Self {
        Self {
            kind: VerdictKind::Ask,
            reason: format!("vela-hook: {reason}"),
            rule_id: rule_id.to_owned(),
        }
    }

    /// The decision word.
    pub fn word(&self) -> &'static str {
        match self.kind {
            VerdictKind::Allow => "allow",
            VerdictKind::Ask => "ask",
            VerdictKind::Deny => "deny",
        }
    }

    /// The JSON object written to standard output.
    pub fn to_json(&self) -> String {
        serde_json::to_string(&Wire {
            decision: self.word(),
            reason: &self.reason,
        })
        .unwrap_or_else(|_| {
            r#"{"decision":"deny","reason":"vela-hook: could not encode the answer"}"#.to_owned()
        })
    }
}

fn load_engine(path: &Path) -> Result<PolicyEngine, String> {
    let meta =
        std::fs::metadata(path).map_err(|e| format!("the rule table cannot be read: {e}"))?;
    if meta.len() > MAX_TABLE_BYTES {
        return Err("the rule table is too large".to_owned());
    }
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("the rule table cannot be read: {e}"))?;
    let table: HookRuleTable =
        serde_json::from_str(&text).map_err(|e| format!("the rule table is malformed: {e}"))?;
    table
        .into_engine()
        .map_err(|e| format!("the rule table is invalid: {e}"))
}

fn word_of<T: Serialize>(value: &T) -> Option<String> {
    serde_json::to_value(value)
        .ok()?
        .as_str()
        .map(str::to_owned)
}

/// The folder of skills Vela delivers to this profile, next to the rule table (`<config>/plugins/vela-skills`). The agent may
/// read them: they are Vela's own generated, read-only instructions, and the engine would otherwise refuse the path because it
/// lies under `.gemini`.
fn skills_dir(table: &Path) -> Option<String> {
    // The provider starts the hook with a relative table path and the config folder as its working directory.
    let absolute = if table.is_absolute() {
        table.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(table)
    };
    let dir = absolute.parent()?.join("plugins").join("vela-skills");
    Some(normalize(&dir.to_string_lossy()))
}

/// The file of one-time approvals Vela writes next to the rule table: `{"command(<what>)": expires_ms}`.
const ONCE_FILE: &str = "vela-once-rules.json";

/// True when the person approved this command once and the approval has not expired. A grant is either the exact command line or
/// a bare executable name; either only covers a plain command (no shell syntax), so an approval of `git` never covers `git; rm x`.
fn once_granted(table: &Path, text: &str, executable: Option<&str>) -> bool {
    let Ok(raw) = std::fs::read_to_string(table.with_file_name(ONCE_FILE)) else {
        return false;
    };
    let Ok(serde_json::Value::Object(grants)) = serde_json::from_str::<serde_json::Value>(&raw)
    else {
        return false;
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64);
    let words: Vec<&str> = text.split_whitespace().collect();
    let plain = !words.is_empty()
        && words.iter().all(|w| {
            w.chars().all(|c| {
                c.is_ascii_alphanumeric()
                    || matches!(
                        c,
                        '-' | '_' | '.' | '/' | '\\' | ':' | '=' | '+' | ',' | '@'
                    )
            })
        });
    if !plain {
        return false;
    }
    let exact = format!("command({})", words.join(" "));
    let bare = executable.and_then(vela_domain::policy::bare_executable_rule);
    grants.iter().any(|(rule, until)| {
        until.as_i64().is_some_and(|u| u > now)
            && (*rule == exact || bare.as_deref() == Some(rule.as_str()))
    })
}

/// Lower-case, forward-slash form for comparing paths on Windows.
fn normalize(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

/// True when `path` is inside `dir` (both normalized) with no `..` segment.
fn is_inside(dir: &str, path: &str) -> bool {
    let path = normalize(path);
    !path.split('/').any(|part| part == "..")
        && path
            .strip_prefix(dir)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Decides one call from its raw input.
pub fn run(text: &str, table: &Path, spool: &Path) -> Verdict {
    let input = match parse(text) {
        Ok(i) => i,
        Err(why) => return Verdict::deny("vela.hook.input", &why),
    };
    let engine = match load_engine(table) {
        Ok(e) => e,
        Err(why) => {
            return record_and_return(&input, None, Verdict::deny("vela.hook.table", &why), spool);
        }
    };
    let (verdict, class, placement, executable) = match to_operation(&input) {
        Mapped::Internal => (
            Verdict {
                kind: VerdictKind::Allow,
                reason: "vela-hook: an internal agent tool".to_owned(),
                rule_id: "vela.hook.internal-tool".to_owned(),
            },
            None,
            None,
            None,
        ),
        Mapped::Unknown(why) => (
            Verdict::ask("vela.hook.unknown-tool", &why),
            None,
            None,
            None,
        ),
        Mapped::Operation(Operation::FileRead { path })
            if skills_dir(table).is_some_and(|dir| is_inside(&dir, &path)) =>
        {
            (
                Verdict {
                    kind: VerdictKind::Allow,
                    reason: "vela-hook: a skill Vela provided to this worker".to_owned(),
                    rule_id: "vela.hook.skills-read".to_owned(),
                },
                None,
                None,
                None,
            )
        }
        Mapped::Operation(operation) => {
            let decision = engine.evaluate(&operation);
            let granted = decision.decision == Decision::Ask
                && matches!(&operation, Operation::Command { text, .. } if once_granted(table, text, decision.scope.executable.as_deref()));
            let kind = match decision.decision {
                Decision::Allow => VerdictKind::Allow,
                Decision::Ask if granted => VerdictKind::Allow,
                Decision::Ask => VerdictKind::Ask,
                Decision::Deny => VerdictKind::Deny,
            };
            (
                Verdict {
                    kind,
                    reason: if granted {
                        "vela-hook: the person approved this command once".to_owned()
                    } else {
                        format!("vela-policy: {}", decision.reason)
                    },
                    rule_id: if granted {
                        "vela.hook.once-grant".to_owned()
                    } else {
                        decision.rule_id.as_str().to_owned()
                    },
                },
                word_of(&decision.scope.class),
                word_of(&decision.scope.placement),
                decision.scope.executable.clone(),
            )
        }
    };
    record_and_return(&input, Some((class, placement, executable)), verdict, spool)
}

type Facts = (Option<String>, Option<String>, Option<String>);

/// Writes the spool line. An `allow` that could not be recorded is refused: Vela must be able to see what ran.
fn record_and_return(
    input: &HookInput,
    facts: Option<Facts>,
    verdict: Verdict,
    spool: &Path,
) -> Verdict {
    let (class, placement, executable) = facts.unwrap_or((None, None, None));
    let record = SpoolRecord {
        conversation_id: input.conversation_id.clone(),
        step_idx: input.step_idx,
        tool: input.tool.clone(),
        decision: verdict.word(),
        rule_id: verdict.rule_id.clone(),
        class,
        placement,
        executable,
    };
    match append(spool, &record) {
        Ok(_) => verdict,
        Err(e) if verdict.kind == VerdictKind::Allow => Verdict::deny(
            "vela.hook.spool",
            &format!("the decision could not be recorded: {e}"),
        ),
        Err(_) => verdict,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_use_the_provider_protocol() {
        let v = Verdict::deny("r", "because");
        let j: serde_json::Value = serde_json::from_str(&v.to_json()).unwrap();
        assert_eq!(j["decision"], "deny");
        assert_eq!(j["reason"], "vela-hook: because");
        assert_eq!(Verdict::ask("r", "x").word(), "ask");
    }

    #[test]
    fn only_paths_inside_the_skills_folder_without_dot_dot_count() {
        let dir = skills_dir(Path::new("C:/p/.gemini/config/vela-table.json")).unwrap();
        assert_eq!(dir, "c:/p/.gemini/config/plugins/vela-skills");
        assert!(is_inside(
            &dir,
            "C:\\p\\.gemini\\config\\plugins\\vela-skills\\skills\\implement\\SKILL.md"
        ));
        assert!(!is_inside(&dir, "C:/p/.gemini/config/plugins/vela-skills"));
        assert!(!is_inside(
            &dir,
            "C:/p/.gemini/config/plugins/vela-skills/../hooks.json"
        ));
        assert!(!is_inside(
            &dir,
            "C:/p/.gemini/config/plugins/vela-skills-evil/x"
        ));
        assert!(!is_inside(
            &dir,
            "C:/p/.gemini/antigravity-cli/settings.json"
        ));
    }

    #[test]
    fn a_missing_table_denies_even_for_a_valid_input() {
        let input = r#"{"conversationId":"c","toolCall":{"name":"finish","args":{}}}"#;
        let spool = std::env::temp_dir().join(format!("vela-hook-d-{}", std::process::id()));
        let v = run(input, Path::new("does-not-exist.json"), &spool);
        assert_eq!(v.kind, VerdictKind::Deny);
        assert_eq!(v.rule_id, "vela.hook.table");
        let _ = std::fs::remove_dir_all(&spool);
    }
}
