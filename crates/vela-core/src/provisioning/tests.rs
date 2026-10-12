use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use vela_domain::ports::{OutputChunk, OutputStream};
use vela_testkit::fakes::{FakeProcessRunner, Scripted};

use super::*;
use crate::orchestrator::test_support::env;

struct Dirs {
    repo: PathBuf,
    wt: PathBuf,
    base: PathBuf,
}

impl Dirs {
    fn new() -> Self {
        static N: AtomicU32 = AtomicU32::new(0);
        let base = std::env::temp_dir().join(format!(
            "vela-prov-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let (repo, wt) = (base.join("repo"), base.join("wt"));
        std::fs::create_dir_all(repo.join("config")).unwrap();
        std::fs::create_dir_all(&wt).unwrap();
        std::fs::write(repo.join("config/local.json"), "{}").unwrap();
        std::fs::write(repo.join(".env"), "TOKEN=1").unwrap();
        Self { repo, wt, base }
    }
}

impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

fn cmd(text: &str) -> ProfileCommand {
    ProfileCommand {
        text: text.to_owned(),
        cwd: None,
    }
}

fn setup(
    tag: &str,
) -> (
    crate::orchestrator::test_support::TestEnv,
    Arc<FakeProcessRunner>,
    ProvisioningService,
) {
    let e = env(tag);
    e.seed_project();
    let runner = Arc::new(FakeProcessRunner::new());
    let svc = ProvisioningService::new(e.services.clone(), runner.clone());
    (e, runner, svc)
}

#[tokio::test]
async fn an_untrusted_repository_is_never_provisioned() {
    let (e, runner, svc) = setup("untrusted");
    let d = Dirs::new();
    svc.set_profile(
        "p1",
        &ProvisioningProfile {
            commands: vec![cmd("npm ci")],
            ..Default::default()
        },
    )
    .unwrap();
    let err = svc.provision("p1", &d.repo, &d.wt).await.unwrap_err();
    assert_eq!(err.class(), ErrorClass::PolicyBlock);
    assert!(runner.log().calls().is_empty());
    drop(e);
}

#[tokio::test]
async fn confirmed_commands_run_in_order_and_files_are_copied_from_the_allowlist() {
    let (e, runner, svc) = setup("run");
    let d = Dirs::new();
    TrustService::new(&e.services)
        .trust("p1", "id", &[], "v1")
        .unwrap();
    svc.set_profile(
        "p1",
        &ProvisioningProfile {
            commands: vec![cmd("npm ci"), cmd("npm run build")],
            copy: vec!["config/local.json".into()],
            resource_keys: vec!["ports:3000".into()],
            ..Default::default()
        },
    )
    .unwrap();
    let report = svc.provision("p1", &d.repo, &d.wt).await.unwrap();
    assert_eq!(report.commands_run, ["npm ci", "npm run build"]);
    assert_eq!(report.copied, ["config/local.json"]);
    assert_eq!(report.resource_keys, ["ports:3000"]);
    assert!(d.wt.join("config/local.json").is_file());
    assert!(
        !d.wt.join(".env").exists(),
        "nothing is copied automatically"
    );
    assert_eq!(runner.log().calls(), ["run:npm ci", "run:npm run build"]);
}

#[tokio::test]
async fn a_failing_command_is_a_validation_failure_with_output_and_stops_the_rest() {
    let (e, runner, svc) = setup("fail");
    let d = Dirs::new();
    TrustService::new(&e.services)
        .trust("p1", "id", &[], "v1")
        .unwrap();
    svc.set_profile(
        "p1",
        &ProvisioningProfile {
            commands: vec![cmd("npm ci"), cmd("npm run build")],
            ..Default::default()
        },
    )
    .unwrap();
    runner.push(Scripted::exit(1).with_output(OutputChunk {
        stream: OutputStream::Stderr,
        text: "ERR! network down\n".into(),
    }));
    let err = svc.provision("p1", &d.repo, &d.wt).await.unwrap_err();
    assert_eq!(err.class(), ErrorClass::ValidationFailure);
    assert!(err.what_failed().contains("exited with 1"));
    assert!(err.what_failed().contains("network down"));
    assert_eq!(runner.log().calls().len(), 1);
}

#[tokio::test]
async fn a_timeout_fails_provisioning() {
    let (e, runner, svc) = setup("timeout");
    let d = Dirs::new();
    TrustService::new(&e.services)
        .trust("p1", "id", &[], "v1")
        .unwrap();
    svc.set_profile(
        "p1",
        &ProvisioningProfile {
            commands: vec![cmd("npm ci")],
            ..Default::default()
        },
    )
    .unwrap();
    runner.push(Scripted::timeout());
    let err = svc.provision("p1", &d.repo, &d.wt).await.unwrap_err();
    assert!(err.what_failed().contains("timed out"));
}

#[test]
fn secrets_and_escapes_cannot_be_put_on_the_allowlist() {
    let (_e, _r, svc) = setup("allowlist");
    for bad in [
        ".env",
        "config/.env.local",
        "keys/server.pem",
        "../outside.txt",
        "/etc/passwd",
        "C:/x.txt",
        "sub/credentials.json",
        ".aws/config",
    ] {
        let p = ProvisioningProfile {
            copy: vec![bad.into()],
            ..Default::default()
        };
        assert!(svc.set_profile("p1", &p).is_err(), "{bad}");
    }
    assert!(
        svc.set_profile(
            "p1",
            &ProvisioningProfile {
                commands: vec![cmd("a && b")],
                ..Default::default()
            }
        )
        .is_err()
    );
    assert_eq!(svc.profile("p1").unwrap(), ProvisioningProfile::default());
}

#[tokio::test]
async fn a_missing_source_file_fails_before_any_command_runs() {
    let (e, runner, svc) = setup("missing");
    let d = Dirs::new();
    TrustService::new(&e.services)
        .trust("p1", "id", &[], "v1")
        .unwrap();
    svc.set_profile(
        "p1",
        &ProvisioningProfile {
            commands: vec![cmd("npm ci")],
            copy: vec!["config/absent.json".into()],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(svc.provision("p1", &d.repo, &d.wt).await.is_err());
    assert!(runner.log().calls().is_empty());
}
