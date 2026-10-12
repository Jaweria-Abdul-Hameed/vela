use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use vela_domain::policy::{Decision, Operation, PolicyEngine};
use vela_domain::ports::RepoTrust;

use super::*;
use crate::orchestrator::test_support::env;

struct Dir(PathBuf);

impl Dir {
    fn new() -> Self {
        static N: AtomicU32 = AtomicU32::new(0);
        let d = std::env::temp_dir().join(format!(
            "vela-trust-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&d).unwrap();
        Self(d)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn cmd(text: &str) -> ProfileCommand {
    ProfileCommand {
        text: text.to_owned(),
        cwd: None,
    }
}

#[test]
fn a_new_project_is_untrusted_and_builds_are_refused_with_a_way_forward() {
    let e = env("untrusted");
    e.seed_project();
    let t = TrustService::new(&e.services);
    assert_eq!(t.state("p1").unwrap(), TrustState::Untrusted);
    assert_eq!(t.git_trust("p1").unwrap(), RepoTrust::Untrusted);
    let err = t.require_trusted("p1", "Build").unwrap_err();
    assert_eq!(err.class(), ErrorClass::PolicyBlock);
    assert!(err.what_failed().contains("not trusted"));
    assert!(t.profile_commands("p1").unwrap().is_empty());
}

#[test]
fn trusting_persists_journals_and_survives_a_restart() {
    let e = env("trust");
    e.seed_project();
    TrustService::new(&e.services)
        .trust(
            "p1",
            "github.com/me/app",
            &[cmd("cargo test")],
            TRUST_CONSENT_VERSION,
        )
        .unwrap();
    let reopened = e.restart();
    let t = TrustService::new(&reopened);
    assert_eq!(t.state("p1").unwrap(), TrustState::Trusted);
    assert_eq!(t.git_trust("p1").unwrap(), RepoTrust::Trusted);
    t.require_trusted("p1", "Build").unwrap();
    assert_eq!(t.profile_commands("p1").unwrap(), vec![cmd("cargo test")]);
    let journal = e.raw_journal();
    assert!(
        journal
            .iter()
            .any(|(k, p)| k == "ProjectTrusted" && p.contains(TRUST_CONSENT_VERSION))
    );
}

#[test]
fn revoking_clears_trust_and_the_profile_and_is_journaled_once() {
    let e = env("revoke");
    e.seed_project();
    let t = TrustService::new(&e.services);
    t.revoke("p1").unwrap();
    assert!(
        e.raw_journal().is_empty(),
        "revoking an untrusted project journals nothing"
    );
    t.trust("p1", "id", &[cmd("cargo test")], "v1").unwrap();
    t.revoke("p1").unwrap();
    assert_eq!(t.state("p1").unwrap(), TrustState::Untrusted);
    assert!(t.profile_commands("p1").unwrap().is_empty());
    assert!(t.require_trusted("p1", "Build").is_err());
    assert_eq!(
        e.raw_journal()
            .iter()
            .filter(|(k, _)| k == "ProjectTrustRevoked")
            .count(),
        1
    );
}

#[test]
fn a_profile_with_a_compound_command_or_no_consent_version_is_refused() {
    let e = env("bad-profile");
    e.seed_project();
    let t = TrustService::new(&e.services);
    assert!(
        t.trust("p1", "id", &[cmd("cargo test && curl evil.example")], "v1")
            .is_err()
    );
    assert!(
        t.trust(
            "p1",
            "id",
            &[ProfileCommand {
                text: "cargo test".into(),
                cwd: Some("../outside".into())
            }],
            "v1"
        )
        .is_err()
    );
    assert!(t.trust("p1", "id", &[cmd("cargo test")], "  ").is_err());
    assert_eq!(t.state("p1").unwrap(), TrustState::Untrusted);
    assert!(e.raw_journal().is_empty());
}

#[test]
fn the_policy_context_allows_profile_commands_only_for_trusted_projects() {
    let e = env("policy");
    e.seed_project();
    let root = if cfg!(windows) {
        "C:/work/app"
    } else {
        "/work/app"
    };
    let t = TrustService::new(&e.services);
    let op = Operation::Command {
        dialect: Dialect::Cmd,
        text: "cargo test".into(),
        cwd: None,
    };

    let before =
        PolicyEngine::new(t.policy_context("p1", root, RunMode::Supervised).unwrap()).evaluate(&op);
    assert_ne!(
        before.decision,
        Decision::Allow,
        "untrusted projects get no allowed commands"
    );

    t.trust("p1", "id", &[cmd("cargo test")], "v1").unwrap();
    let allowed =
        PolicyEngine::new(t.policy_context("p1", root, RunMode::Supervised).unwrap()).evaluate(&op);
    assert_eq!(allowed.decision, Decision::Allow);
    let other = Operation::Command {
        dialect: Dialect::Cmd,
        text: "cargo publish".into(),
        cwd: None,
    };
    assert_ne!(
        PolicyEngine::new(t.policy_context("p1", root, RunMode::Supervised).unwrap())
            .evaluate(&other)
            .decision,
        Decision::Allow
    );

    t.revoke("p1").unwrap();
    let after =
        PolicyEngine::new(t.policy_context("p1", root, RunMode::Supervised).unwrap()).evaluate(&op);
    assert_ne!(after.decision, Decision::Allow);
}

#[test]
fn proposals_come_from_manifests_read_as_data() {
    let d = Dir::new();
    std::fs::write(
        d.0.join("package.json"),
        r#"{"scripts":{"test":"echo $(rm -rf /)","build":"x","dev":"y"}}"#,
    )
    .unwrap();
    std::fs::write(d.0.join("Cargo.toml"), "[package]\nname=\"x\"").unwrap();
    std::fs::create_dir_all(d.0.join(".husky")).unwrap();
    let p = propose(&d.0);
    let texts: Vec<_> = p.commands.iter().map(|c| c.text.as_str()).collect();
    assert!(texts.contains(&"npm test"));
    assert!(texts.contains(&"npm run build"));
    assert!(
        !texts
            .iter()
            .any(|t| t.contains("dev") || t.contains("rm -rf"))
    );
    assert!(texts.contains(&"cargo test"));
    assert!(p.executable_surfaces.iter().any(|s| s.contains(".husky")));
    assert!(propose(&d.0.join("nothing-here")).commands.is_empty());
}
