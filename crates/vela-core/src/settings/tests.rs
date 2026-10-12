use vela_domain::ids::RunId;

use super::*;
use crate::orchestrator::test_support::env;

#[tokio::test]
async fn layers_apply_in_order_defaults_then_user_then_project() {
    let e = env("layers");
    e.seed_project();
    let s = SettingsService::new(&e.services);
    let defaults = s.effective(Some("p1")).unwrap();
    assert_eq!(defaults["max_parallel_workers"], "2");
    assert_eq!(
        defaults["review_mode"], "off",
        "ADR-024: review is off by default"
    );
    s.set_user(Source::User, "max_parallel_workers", "4")
        .unwrap();
    assert_eq!(
        s.effective(Some("p1")).unwrap()["max_parallel_workers"],
        "4"
    );
    s.set_project(Source::User, "p1", "max_parallel_workers", "3")
        .unwrap();
    assert_eq!(
        s.effective(Some("p1")).unwrap()["max_parallel_workers"],
        "3"
    );
    assert_eq!(
        s.effective(None).unwrap()["max_parallel_workers"],
        "4",
        "another project is not affected"
    );
}

#[tokio::test]
async fn invalid_keys_values_and_scopes_are_refused() {
    let e = env("invalid");
    e.seed_project();
    let s = SettingsService::new(&e.services);
    assert!(s.set_user(Source::User, "no_such_key", "x").is_err());
    assert!(s.set_user(Source::User, "graphics_mode", "neon").is_err());
    assert!(
        s.set_user(Source::User, "max_parallel_workers", "99")
            .is_err()
    );
    assert!(
        s.set_user(Source::User, "max_parallel_workers", "two")
            .is_err()
    );
    assert!(s.set_user(Source::User, "autostart", "yes").is_err());
    // A user-wide key cannot be set per project.
    assert!(
        s.set_project(Source::User, "p1", "graphics_mode", "full")
            .is_err()
    );
    assert_eq!(
        s.effective(Some("p1")).unwrap()["graphics_mode"],
        "balanced"
    );
}

#[tokio::test]
async fn repository_text_cannot_alter_any_setting() {
    let e = env("repo-text");
    e.seed_project();
    let s = SettingsService::new(&e.services);
    for k in KEYS {
        let err = s
            .set_user(Source::Repository, k.name, k.default)
            .unwrap_err();
        assert_eq!(err.class(), ErrorClass::PolicyBlock, "{}", k.name);
        assert!(
            s.set_project(Source::Repository, "p1", k.name, k.default)
                .is_err()
        );
    }
    assert!(
        s.record_consent(
            Source::Repository,
            &ConsentRecord {
                key: "ui_automation".into(),
                text_version: "v1".into(),
                granted: true
            }
        )
        .is_err()
    );
    assert!(s.consent("ui_automation").unwrap().is_none());
}

#[tokio::test]
async fn a_run_keeps_the_settings_it_started_with_except_live_keys() {
    let e = env("snapshot");
    e.seed_project();
    let run = RunId::new("run-1").unwrap();
    // The run row must exist for the snapshot's foreign key.
    crate::orchestrator::runs::Runs::new(&e.services)
        .create_run(
            "p1",
            run.clone(),
            vela_domain::ids::CommitSha::new("a".repeat(40)).unwrap(),
            vela_domain::ids::BranchName::new("vela/run-1/integration").unwrap(),
            "all",
        )
        .await
        .unwrap();
    let s = SettingsService::new(&e.services);
    s.set_user(Source::User, "max_parallel_workers", "4")
        .unwrap();
    s.set_user(Source::User, "graphics_mode", "full").unwrap();
    let captured = s.capture_run_snapshot(&run, Some("p1")).unwrap();
    assert_eq!(captured["max_parallel_workers"], "4");

    // Changed mid-run: the fixed key keeps its value, the live key follows.
    s.set_user(Source::User, "max_parallel_workers", "1")
        .unwrap();
    s.set_user(Source::User, "graphics_mode", "efficiency")
        .unwrap();
    assert_eq!(
        s.run_value(&run, Some("p1"), "max_parallel_workers")
            .unwrap(),
        "4"
    );
    assert_eq!(
        s.run_value(&run, Some("p1"), "graphics_mode").unwrap(),
        "efficiency"
    );
    // A run has one snapshot.
    assert!(s.capture_run_snapshot(&run, Some("p1")).is_err());
    // A run with no snapshot has no fixed values.
    assert!(
        s.run_value(&RunId::new("run-x").unwrap(), None, "run_mode")
            .is_err()
    );
}

#[tokio::test]
async fn consent_records_carry_the_text_version_and_are_journaled() {
    let e = env("consent");
    let s = SettingsService::new(&e.services);
    assert!(s.consent("ui_automation").unwrap().is_none());
    s.record_consent(
        Source::User,
        &ConsentRecord {
            key: "ui_automation".into(),
            text_version: "2026-10-v1".into(),
            granted: true,
        },
    )
    .unwrap();
    let c = s.consent("ui_automation").unwrap().unwrap();
    assert_eq!((c.text_version.as_str(), c.granted), ("2026-10-v1", true));
    s.record_consent(
        Source::User,
        &ConsentRecord {
            key: "ui_automation".into(),
            text_version: "2026-10-v2".into(),
            granted: false,
        },
    )
    .unwrap();
    assert!(
        !s.consent("ui_automation").unwrap().unwrap().granted,
        "the latest decision wins"
    );
    let journal = e.raw_journal();
    assert_eq!(
        journal
            .iter()
            .filter(|(k, _)| k == "ApprovalAutomationConsentChanged")
            .count(),
        2
    );
    assert!(journal.iter().any(|(_, p)| p.contains("2026-10-v1")));
    // A consent without a text version is refused: the record must say what the user saw.
    assert!(
        s.record_consent(
            Source::User,
            &ConsentRecord {
                key: "x".into(),
                text_version: " ".into(),
                granted: true
            }
        )
        .is_err()
    );
}

#[tokio::test]
async fn onboarding_choices_are_journaled_and_other_settings_are_not() {
    let e = env("onboarding");
    let s = SettingsService::new(&e.services);
    s.set_user(Source::User, "autostart", "true").unwrap();
    s.set_user(Source::User, "graphics_mode", "full").unwrap();
    let journal = e.raw_journal();
    assert_eq!(
        journal
            .iter()
            .filter(|(k, _)| k == "OnboardingChoiceRecorded")
            .count(),
        1
    );
    assert!(journal[0].1.contains("autostart"));
    assert_eq!(s.effective(None).unwrap()["autostart"], "true");
}

#[test]
fn autonomous_mode_needs_a_met_posture() {
    use vela_domain::ports::PermissionPosture as P;
    assert!(validate_run_mode("supervised", P::Unknown).is_ok());
    assert!(validate_run_mode("autonomous", P::Meets).is_ok());
    for p in [P::DoesNotMeet, P::Unknown] {
        assert_eq!(
            validate_run_mode("autonomous", p).unwrap_err().class(),
            ErrorClass::PolicyBlock
        );
    }
}
