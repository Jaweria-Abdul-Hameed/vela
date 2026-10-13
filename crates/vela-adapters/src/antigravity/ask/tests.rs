use std::io::Write;
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;

fn dir(tag: &str) -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let d = std::env::temp_dir().join(format!(
        "vela-ask-{tag}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn line(decision: &str, exe: &str, step: Option<u64>) -> String {
    let step = step.map_or("null".to_owned(), |s| s.to_string());
    format!(
        "{{\"v\":1,\"ts_ms\":1,\"conversation_id\":\"conv-1\",\"step_idx\":{step},\"tool\":\"run_command\",\"decision\":\"{decision}\",\"rule_id\":\"r\",\"class\":\"UnknownExecutable\",\"placement\":\"inside\",\"executable\":\"{exe}\",\"fingerprint\":\"abc\"}}\n"
    )
}

fn append(path: &Path, text: &str) {
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    f.write_all(text.as_bytes()).unwrap();
}

#[test]
fn new_lines_are_read_once_and_a_partial_line_waits() {
    let d = dir("poll");
    let file = d.join("conv-1.jsonl");
    let r = SpoolReader::new(d.clone());
    assert!(r.poll().unwrap().is_empty());
    append(&file, &line("allow", "git", Some(1)));
    append(&file, &line("ask", "npm", Some(2)));
    assert_eq!(r.poll().unwrap().len(), 2);
    assert!(r.poll().unwrap().is_empty(), "nothing is read twice");
    let partial = line("ask", "curl", Some(3));
    append(&file, &partial[..40]);
    assert!(r.poll().unwrap().is_empty());
    append(&file, &partial[40..]);
    let got = r.poll().unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].executable.as_deref(), Some("curl"));
    append(&file, "this is not json\n");
    append(&file, &line("deny", "rm", Some(4)));
    assert_eq!(r.poll().unwrap().len(), 1, "a corrupt line is skipped");
    let _ = std::fs::remove_dir_all(&d);
}

#[tokio::test]
async fn only_asks_become_observations_and_they_carry_bound_evidence_when_the_step_is_known() {
    let d = dir("source");
    let file = d.join("conv-1.jsonl");
    let r = SpoolReader::new(d.clone());
    append(&file, &line("allow", "git", Some(1)));
    append(&file, &line("ask", "npm", Some(2)));
    append(&file, &line("ask", "pip", None));
    let first = r.next_observation().await.unwrap().unwrap();
    assert_eq!(
        (first.operation.as_str(), first.evidence),
        ("npm", EvidenceTier::Bound)
    );
    assert_eq!(first.session_identity.as_deref(), Some("conv-1"));
    let second = r.next_observation().await.unwrap().unwrap();
    assert_eq!(
        (second.operation.as_str(), second.evidence),
        ("pip", EvidenceTier::Observed)
    );
    assert!(r.next_observation().await.unwrap().is_none());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn the_one_time_rule_is_exact_installed_once_and_swept_when_it_expires() {
    let d = dir("rule");
    let settings = d.join("settings.json");
    let ledger = d.join("vela-once.json");
    std::fs::write(
        &settings,
        r#"{"permissions":{"allow":["command(cargo test)"]}}"#,
    )
    .unwrap();

    let rule = once_rule("npm", 1_000, 600_000).unwrap();
    assert_eq!(rule.rule, "command(npm)");
    assert_eq!(rule.expires_ms, 601_000);
    install_once_rule(&settings, &ledger, &rule).unwrap();
    install_once_rule(&settings, &ledger, &rule).unwrap();
    let s: Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(
        s["permissions"]["allow"],
        serde_json::json!(["command(cargo test)", "command(npm)"])
    );

    assert!(
        sweep_expired(&settings, &ledger, 600_999)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        sweep_expired(&settings, &ledger, 601_000).unwrap(),
        ["command(npm)"]
    );
    let s: Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
    assert_eq!(
        s["permissions"]["allow"],
        serde_json::json!(["command(cargo test)"]),
        "Vela's profile rules are never touched"
    );
    assert!(
        sweep_expired(&settings, &ledger, 999_999)
            .unwrap()
            .is_empty()
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_rule_is_created_from_a_clean_name_only_and_a_missing_settings_file_is_created() {
    for bad in ["", "rm -rf", "a)b", "x;y", "a b"] {
        assert!(once_rule(bad, 0, 1).is_none(), "{bad:?}");
    }
    let d = dir("fresh");
    let rule = once_rule("git", 0, 5).unwrap();
    install_once_rule(&d.join("settings.json"), &d.join("ledger.json"), &rule).unwrap();
    let s: Value =
        serde_json::from_str(&std::fs::read_to_string(d.join("settings.json")).unwrap()).unwrap();
    assert_eq!(
        s["permissions"]["allow"],
        serde_json::json!(["command(git)"])
    );
    assert!(resume_prompt("git").contains("approved once"));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn counts_group_by_decision() {
    let entries = vec![
        serde_json::from_str::<SpoolEntry>(line("ask", "a", Some(1)).trim()).unwrap(),
        serde_json::from_str::<SpoolEntry>(line("ask", "b", Some(2)).trim()).unwrap(),
        serde_json::from_str::<SpoolEntry>(line("deny", "c", Some(3)).trim()).unwrap(),
    ];
    let c = count_by_decision(&entries);
    assert_eq!((c["ask"], c["deny"]), (2, 1));
}
