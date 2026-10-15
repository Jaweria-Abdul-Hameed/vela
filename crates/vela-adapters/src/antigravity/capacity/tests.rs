use super::policy_block::{self, Scope, Source};
use super::*;

#[test]
fn reset_times_are_parsed_in_the_shapes_the_provider_uses_and_never_guessed() {
    let h = 3_600_000;
    let m = 60_000;
    assert_eq!(
        parse_reset_after_ms("Your quota will reset after 3h12m5s."),
        Some(3 * h + 12 * m + 5_000)
    );
    assert_eq!(
        parse_reset_after_ms("quota resets in 45 minutes"),
        Some(45 * m)
    );
    assert_eq!(
        parse_reset_after_ms("Try again in 90 seconds"),
        Some(90_000)
    );
    assert_eq!(
        parse_reset_after_ms("quota resets in 2 hours 5 minutes."),
        Some(2 * h + 5 * m)
    );
    assert_eq!(
        parse_reset_after_ms("You have exhausted your capacity."),
        None
    );
    assert_eq!(parse_reset_after_ms("reset after soon"), None);
    assert_eq!(
        parse_reset_after_ms("retry after 99999999999999999999h"),
        None
    );
}

#[test]
fn evidence_is_redacted_and_bounded() {
    let r = redact(
        "mail me@example.com or visit https://x.invalid/a?token=SECRET and key ABCDEFGHIJKLMNOPQRSTUVWXYZ012345",
    );
    assert!(!r.contains("me@example.com") && !r.contains("SECRET") && !r.contains("ABCDEFGHIJ"));
    assert!(
        r.contains("https://x.invalid/a?[query]")
            && r.contains("[address]")
            && r.contains("[token]")
    );
    assert!(excerpt(&"x ".repeat(500)).chars().count() <= 300);
    assert_eq!(excerpt("\n\n  first real line \nsecond"), "first real line");
}

#[test]
fn capacity_text_is_capacity_with_its_reset_time_whatever_the_exit_code_was() {
    let c = classify_access(
        "jetski: RESOURCE_EXHAUSTED (code 429): exhausted your capacity. Your quota will reset after 1h30m.",
    );
    assert!(matches!(
        c,
        Some(AccessClass::Capacity {
            reset_after_ms: Some(5_400_000),
            ..
        })
    ));
}

#[test]
fn a_policy_block_outranks_every_other_reading() {
    // Wording that also contains a quota phrase must still be a block, never softened into a wait.
    let c = classify_access(
        "Your account has been suspended for a violation of the terms. quota exceeded",
    );
    assert!(matches!(c, Some(AccessClass::PolicyBlock(_))));
}

#[test]
fn the_documented_style_and_the_synthetic_signals_each_fire_and_say_where_they_came_from() {
    let doc = policy_block::detect(
        "Sign-in failed: this account is blocked because of a Terms of Service violation. You can appeal at https://example.invalid/appeal?token=abcdefghijklmnopqrstuvwxyz0123456789",
    )
    .unwrap();
    assert_eq!(doc.signal, "documented-terms-block-with-appeal");
    assert_eq!(
        (doc.scope, doc.source),
        (Scope::Account, Source::Documented)
    );
    assert!(!doc.evidence.contains("abcdefghijklmnop"));
    let syn = policy_block::detect("Your account has been suspended.").unwrap();
    assert_eq!(syn.source, Source::Synthetic);
    let svc = policy_block::detect("Access to this service has been blocked.").unwrap();
    assert_eq!(svc.scope, Scope::Service);
}

#[test]
fn a_vague_word_alone_is_never_a_block_and_unknown_access_failures_are_unclassified() {
    assert!(policy_block::detect("the build was blocked by a lock").is_none());
    assert!(policy_block::detect("terms").is_none());
    let c =
        classify_access("jetski: permission denied (code 403) for this account on model gemini-x");
    assert!(matches!(c, Some(AccessClass::Unclassified { .. })));
    assert!(classify_access("jetski: no output produced").is_none());
    assert!(classify_access("").is_none());
}

#[test]
fn every_signal_is_lower_case_and_labelled() {
    for s in policy_block::SIGNALS {
        assert!(
            s.all_of
                .iter()
                .all(|p| *p == p.to_ascii_lowercase() && !p.is_empty()),
            "{}",
            s.id
        );
        assert_eq!(
            s.id.starts_with("synthetic-"),
            s.source == Source::Synthetic,
            "{}",
            s.id
        );
    }
}

/// The adapter's source never retries, switches profile, reads credentials or selects an API key after a classification.
#[test]
fn the_adapter_has_no_retry_credential_or_api_key_path() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/antigravity");
    let mut offenders = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs")
                && !p.file_name().is_some_and(|n| n == "tests.rs")
            {
                let text = std::fs::read_to_string(&p).unwrap().to_ascii_lowercase();
                let code: String = text
                    .lines()
                    .filter(|l| !l.trim_start().starts_with("//"))
                    .collect::<Vec<_>>()
                    .join("\n");
                for bad in [
                    "gemini_api_key",
                    "google_api_key",
                    "oauth_creds",
                    "credentials.json",
                    "retry_on_policy",
                ] {
                    if code.contains(bad) {
                        offenders.push(format!("{}: {bad}", p.display()));
                    }
                }
            }
        }
    }
    assert!(offenders.is_empty(), "{offenders:?}");
}
