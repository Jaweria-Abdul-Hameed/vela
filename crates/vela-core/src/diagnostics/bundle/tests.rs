use std::sync::Arc;

use serde_json::json;

use super::*;
use crate::diagnostics::SharedRedaction;
use crate::orchestrator::test_support::{MarkerRedactor, env};

fn inputs(secret: &str) -> BundleInputs {
    BundleInputs {
        version: "0.1.0".into(),
        capabilities: json!({"headless": true, "note": format!("seen {secret}")}),
        logs: vec![
            ("core log.txt".into(), format!("line with {secret}\nsecond")),
            ("big.log".into(), "x".repeat(LOG_CAP_BYTES * 2)),
        ],
        git_refs: vec![format!("refs/heads/main {}", "a".repeat(40))],
    }
}

#[tokio::test]
async fn the_bundle_has_every_part_and_no_secret() {
    let e = env("bundle");
    let run = e.create_run().await;
    let secret = ["sk-live", "-9f8e7d"].concat();
    e.append(vela_domain::events::Event::HumanActionRequired(
        vela_domain::events::HumanActionRequiredPayload {
            intervention_kind: "X".into(),
            summary: format!("leaked {secret}"),
        },
    ));
    let red: SharedRedaction = Arc::new(MarkerRedactor(secret.clone()));
    let b = build(&e.services, &red, &inputs(&secret)).unwrap();
    for name in [
        "version.txt",
        "settings.json",
        "runs.json",
        "journal.jsonl",
        "capabilities.json",
        "git-refs.txt",
        "logs/core_log.txt",
        "logs/big.log",
    ] {
        assert!(b.files.contains_key(name), "{name}");
    }
    for (name, text) in &b.files {
        assert!(!text.contains(&secret), "{name} leaks");
    }
    assert!(b.files["runs.json"].contains(run.as_str()));
    assert!(b.files["journal.jsonl"].contains("[REDACTED]"));
    assert!(b.files["settings.json"].contains("max_parallel_workers"));
    assert!(b.files["logs/big.log"].len() < LOG_CAP_BYTES + 100);
}

#[test]
fn logs_are_capped_keeping_the_newest_part() {
    let text = format!("{}END", "a".repeat(1000));
    let capped = cap_log(&text, 100);
    assert!(capped.ends_with("END"));
    assert!(capped.len() < 200);
    assert_eq!(cap_log("short", 100), "short");
    let multi = "é".repeat(100);
    assert!(cap_log(&multi, 51).ends_with('é'));
}

#[tokio::test]
async fn the_bundle_is_written_to_a_new_folder_only() {
    let e = env("write");
    let red: SharedRedaction = Arc::new(MarkerRedactor("zzz".into()));
    let b = build(&e.services, &red, &BundleInputs::default()).unwrap();
    let dir = std::env::temp_dir().join(format!("vela-bundle-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(write_to(&b, &dir).unwrap() >= 6);
    assert!(dir.join("settings.json").is_file());
    assert!(
        write_to(&b, &dir).is_err(),
        "an existing folder is never overwritten"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
