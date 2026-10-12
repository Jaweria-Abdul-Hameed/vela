use std::path::PathBuf;
use std::sync::Arc;

use vela_domain::errors::ErrorCode;
use vela_domain::ids::WorktreePath;
use vela_domain::ports::{AgentAdapter, AgentCapability, AgentProfile, PermissionPosture};
use vela_testkit::fake_agy::{FakeAgy, RECORDED_AGY_VERSION};

use super::*;
use crate::antigravity::session::AgyDriver;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/transcripts")
            .join(name),
    )
    .unwrap()
}

fn probe(fake: &Arc<FakeAgy>) -> CapabilityProbe {
    CapabilityProbe::new(
        fake.clone(),
        vec![("PATH".into(), "x".into())],
        WorktreePath::new("C:/work").unwrap(),
    )
}

#[test]
fn the_recorded_help_output_yields_the_flags_subcommands_and_stream_json() {
    let installation = parse_help(&fixture("agy-help.txt"));
    for flag in [
        "--print",
        "--output-format",
        "--conversation",
        "--print-timeout",
        "--json-schema",
        "-p",
        "--sandbox",
    ] {
        assert!(
            installation.flags.contains(flag),
            "{flag} in {:?}",
            installation.flags
        );
    }
    for sub in ["models", "update", "mcp", "plugin"] {
        assert!(installation.subcommands.contains(sub), "{sub}");
    }
    assert!(installation.stream_json);
    assert!(parse_help("nothing useful").flags.is_empty());
}

#[tokio::test]
async fn against_the_recorded_cli_the_report_matches_the_observed_facts() {
    let fake = Arc::new(FakeAgy::new());
    let report = probe(&fake).probe().await;
    assert_eq!(
        report.installation.version.as_deref(),
        Some(RECORDED_AGY_VERSION)
    );
    assert_eq!(report.auth, AuthState::Authenticated);
    assert!(report.can_start_runs());
    assert!(report.blocking_error().is_none());
    for ready in [
        CapId::Cap01,
        CapId::Cap02,
        CapId::Cap03,
        CapId::Cap04,
        CapId::Cap05,
        CapId::Cap06,
        CapId::Cap08,
        CapId::Cap10,
    ] {
        assert_eq!(
            report.result(ready).unwrap().outcome,
            Outcome::Ready,
            "{ready:?}"
        );
    }
    // Not proven yet: they are reported, never claimed.
    assert_eq!(
        report.result(CapId::Cap11).unwrap().outcome,
        Outcome::BlockAutonomous
    );
    assert_eq!(
        report.result(CapId::Cap12).unwrap().outcome,
        Outcome::ReleaseGating
    );
    assert!(matches!(
        report.result(CapId::Cap07).unwrap().outcome,
        Outcome::Degraded(_)
    ));
    assert!(matches!(
        report.result(CapId::Cap09).unwrap().outcome,
        Outcome::Degraded(_)
    ));
    let findings = report.preflight_findings();
    assert!(
        findings
            .iter()
            .all(|f| f.severity == FindingSeverity::Warning)
    );
    assert!(findings.iter().any(|f| f.id == "antigravity.cap-11"));
}

#[tokio::test]
async fn a_missing_agy_blocks_with_capability_missing() {
    let fake = Arc::new(FakeAgy::new());
    fake.set_installed(false);
    let report = probe(&fake).probe().await;
    assert!(report.installation.version.is_none());
    assert!(!report.can_start_runs());
    assert_eq!(report.result(CapId::Cap01).unwrap().outcome, Outcome::Block);
    assert_eq!(
        report.blocking_error().unwrap().code(),
        Some(ErrorCode::CapabilityMissing)
    );
    assert!(
        report
            .preflight_findings()
            .iter()
            .any(|f| f.severity == FindingSeverity::Blocking)
    );
}

#[test]
fn an_unauthenticated_state_yields_auth_required() {
    let state = auth_from_models(Some(1), "", &fixture("auth-error.stderr.txt"));
    assert!(matches!(state, AuthState::Unauthenticated(_)));
    let report = build_report(
        parse_help(&fixture("agy-help.txt")),
        state,
        DesktopInstall::default(),
    );
    let mut report = report;
    report.installation.version = Some("1.3.3".into());
    assert!(!report.can_start_runs());
    assert_eq!(
        report.blocking_error().unwrap().code(),
        Some(ErrorCode::AuthRequired)
    );
    assert!(
        report
            .preflight_findings()
            .iter()
            .any(|f| f.id == "antigravity.auth" && f.severity == FindingSeverity::Blocking)
    );
    assert!(matches!(
        auth_from_models(Some(0), "", ""),
        AuthState::Unknown(_)
    ));
    assert_eq!(
        auth_from_models(Some(0), &fixture("agy-models.txt"), ""),
        AuthState::Authenticated
    );
}

/// The specified BLOCK or degradation for every capability that is not provided (table test over CAP-01..CAP-12).
#[test]
fn each_missing_capability_has_its_specified_outcome() {
    use CapId::*;
    type Check = fn(Outcome) -> bool;
    let expected: [(CapId, Check); 12] = [
        (Cap01, |o| o == Outcome::Block),
        (Cap02, |o| o == Outcome::Block),
        (Cap03, |o| o == Outcome::Block),
        (Cap04, |o| o == Outcome::Block),
        (Cap05, |o| o == Outcome::BlockAutonomous),
        (Cap06, |o| o == Outcome::Block),
        (
            Cap07,
            |o| matches!(o, Outcome::Degraded(t) if t.contains("one session")),
        ),
        (
            Cap08,
            |o| matches!(o, Outcome::Degraded(t) if t.contains("interventions")),
        ),
        (
            Cap09,
            |o| matches!(o, Outcome::Degraded(t) if t.contains("pauses")),
        ),
        (
            Cap10,
            |o| matches!(o, Outcome::Degraded(t) if t.contains("last checkpoint")),
        ),
        (Cap11, |o| o == Outcome::BlockAutonomous),
        (Cap12, |o| o == Outcome::ReleaseGating),
    ];
    assert_eq!(expected.len(), CapId::ALL.len());
    for (id, check) in expected {
        assert!(
            check(outcome_for(id, Support::Unsupported)),
            "{id:?} unsupported"
        );
        assert!(
            check(outcome_for(id, Support::Unverified)),
            "{id:?} unverified"
        );
        assert_eq!(outcome_for(id, Support::Supported), Outcome::Ready);
    }
}

#[tokio::test]
async fn the_probe_never_touches_credentials_and_only_runs_quota_free_commands() {
    let fake = Arc::new(FakeAgy::new());
    probe(&fake).probe().await;
    let calls = fake.calls();
    let subcommands: Vec<&str> = calls.iter().map(|c| c.args[0].as_str()).collect();
    assert_eq!(
        subcommands,
        ["--version", "--help", "models"],
        "no prompt (no quota) is ever sent"
    );
    for call in &calls {
        assert!(call.args.iter().all(|a| a != "-p" && a != "--print"));
        assert!(
            call.env_names
                .iter()
                .all(|n| !n.to_ascii_lowercase().contains("token")
                    && !n.to_ascii_lowercase().contains("credential"))
        );
    }
    let source = include_str!("mod.rs");
    let production = source.split("#[cfg(test)]").next().unwrap();
    for banned in [
        ".gemini",
        "oauth",
        "credentials.json",
        "cookies",
        "read_to_string",
        "std::fs::read",
    ] {
        assert!(
            !production.contains(banned),
            "the probe must not read a credential store: {banned}"
        );
    }
}

#[tokio::test]
async fn the_driver_reports_the_port_capabilities_and_an_unknown_posture() {
    let fake = Arc::new(FakeAgy::new());
    let driver = AgyDriver::new(fake.clone(), vec![]);
    let caps = driver.capabilities().await.unwrap();
    assert_eq!(caps.version, RECORDED_AGY_VERSION);
    for c in [
        AgentCapability::Discovery,
        AgentCapability::LifecycleObservation,
        AgentCapability::Cancel,
        AgentCapability::Resume,
    ] {
        assert!(caps.supports(c), "{c:?}");
    }
    assert!(!caps.supports(AgentCapability::PostureInspection));
    assert!(!caps.supports(AgentCapability::ConversationVisibility));
    assert_eq!(
        driver.permission_posture().await.unwrap(),
        PermissionPosture::Unknown
    );
    fn assert_adapter<T: AgentAdapter>(_: &T) {}
    assert_adapter(&driver);
}
