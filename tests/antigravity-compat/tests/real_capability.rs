//! Real-environment A01 check: the capability probe against the installed `agy`. It runs `--version`, `--help` and `models`
//! only, so it spends no quota, but it is still gated by `VELA_REAL_ANTIGRAVITY=1` like the rest of the suite.

use std::sync::Arc;

use vela_adapters::antigravity::capability::{AuthState, CapId, CapabilityProbe, Outcome};
use vela_antigravity_compat::{GATE_ENV, gate_enabled};
use vela_domain::ids::WorktreePath;
use vela_process::redact::PatternRedactor;
use vela_process::runner::{TokioProcessRunner, baseline_env};
use vela_process::tree::TreeRegistry;

#[tokio::test]
async fn the_probe_reports_the_installed_cli() {
    if !gate_enabled(std::env::var(GATE_ENV).ok().as_deref()) {
        eprintln!("skipped: set {GATE_ENV}=1 to run the real-environment suite");
        return;
    }
    let runner = TokioProcessRunner::new(
        Arc::new(PatternRedactor::new()),
        Arc::new(TreeRegistry::new()),
    );
    let cwd = WorktreePath::new(std::env::temp_dir().to_string_lossy().replace('\\', "/")).unwrap();
    let report = CapabilityProbe::new(Arc::new(runner), baseline_env(), cwd)
        .probe()
        .await;
    eprintln!(
        "agy {:?}, auth {:?}",
        report.installation.version, report.auth
    );
    assert!(
        report.installation.version.is_some(),
        "agy must be installed on the machine running the real suite"
    );
    assert!(
        report.installation.stream_json,
        "the installed agy must offer stream-json output"
    );
    for id in [
        CapId::Cap01,
        CapId::Cap02,
        CapId::Cap03,
        CapId::Cap04,
        CapId::Cap10,
    ] {
        assert_eq!(report.result(id).unwrap().outcome, Outcome::Ready, "{id:?}");
    }
    assert!(
        !matches!(report.auth, AuthState::Unauthenticated(_)),
        "sign in to Antigravity to run the real suite"
    );
}
