use super::*;
use crate::preflight::collect_local;

fn facts() -> AntigravityFacts {
    AntigravityFacts {
        installed: true,
        agy_version: Some("1.3.0".into()),
        desktop_version: Some("2.17.0".into()),
        capabilities: (1..=12).map(|n| (format!("CAP-{n:02}"), true)).collect(),
        auth: Auth::SignedIn,
        skills_missing: vec![],
        skills_offer_closed: false,
        posture: Posture::Meets,
        uia_consent: true,
        uia_capable: true,
        session: SessionAvailability::Available,
        unattended_interruptions: vec![],
        mode: Mode::Autonomous,
    }
}

fn run(f: AntigravityFacts) -> Vec<Check> {
    let cell = AntigravityFactsCell::new();
    cell.set(f);
    AntigravityChecks::new(cell).run(&collect_local(std::path::Path::new(".")))
}

fn get<'a>(checks: &'a [Check], id: &str) -> &'a Check {
    checks
        .iter()
        .find(|c| c.id == id)
        .unwrap_or_else(|| panic!("no check {id}"))
}

#[test]
fn every_posture_and_consent_combination_has_the_specified_readiness() {
    use Severity::*;
    // (posture, consent, capable) -> (autonomous, supervised)
    let table = [
        (Posture::Meets, true, true, Pass, Pass),
        (Posture::Meets, true, false, Warn, Pass),
        (Posture::Meets, false, true, Warn, Pass),
        (Posture::Meets, false, false, Warn, Pass),
        (Posture::DoesNotMeet, true, true, Block, Warn),
        (Posture::DoesNotMeet, false, false, Block, Warn),
        (Posture::Unknown, true, true, Block, Warn),
        (Posture::Unknown, false, false, Block, Warn),
    ];
    for (posture, consent, capable, auto, sup) in table {
        let r = readiness(&AntigravityFacts {
            posture,
            uia_consent: consent,
            uia_capable: capable,
            ..facts()
        });
        assert_eq!(
            (r.autonomous, r.supervised),
            (auto, sup),
            "{posture:?} consent={consent} capable={capable}"
        );
    }
}

#[test]
fn a_missing_install_or_a_missing_sign_in_blocks_both_modes() {
    for f in [
        AntigravityFacts {
            installed: false,
            ..facts()
        },
        AntigravityFacts {
            auth: Auth::SignedOut,
            ..facts()
        },
    ] {
        let r = readiness(&f);
        assert_eq!(
            (r.autonomous, r.supervised),
            (Severity::Block, Severity::Block)
        );
    }
    let r = readiness(&AntigravityFacts {
        auth: Auth::Unknown,
        ..facts()
    });
    assert_eq!(
        (r.autonomous, r.supervised),
        (Severity::Warn, Severity::Warn)
    );
}

#[test]
fn a_locked_desktop_only_matters_when_ui_automation_is_in_use() {
    let locked = AntigravityFacts {
        session: SessionAvailability::Locked,
        ..facts()
    };
    assert_eq!(readiness(&locked).autonomous, Severity::Warn);
    let off = AntigravityFacts {
        uia_consent: false,
        ..locked
    };
    let r = readiness(&off);
    assert!(r.reasons.iter().all(|x| !x.contains("locked")));
}

#[test]
fn capability_rows_follow_the_severity_table() {
    let mut f = facts();
    for id in ["CAP-03", "CAP-07", "CAP-09", "CAP-12"] {
        f.capabilities.insert(id.into(), false);
    }
    let c = run(f);
    assert_eq!(get(&c, "antigravity.cap-03").severity, Severity::Block);
    assert_eq!(get(&c, "antigravity.cap-07").severity, Severity::Warn);
    assert!(get(&c, "antigravity.cap-07").detail.contains("one worker"));
    assert_eq!(get(&c, "antigravity.cap-09").severity, Severity::Warn);
    assert_eq!(get(&c, "antigravity.cap-12").severity, Severity::Pass);
    assert_eq!(get(&c, "antigravity.cap-01").severity, Severity::Pass);
}

#[test]
fn auth_and_skills_rows_follow_the_severity_table() {
    let c = run(AntigravityFacts {
        auth: Auth::SignedOut,
        ..facts()
    });
    assert_eq!(get(&c, "antigravity.auth").severity, Severity::Block);
    assert!(get(&c, "antigravity.auth").detail.contains("AUTH_REQUIRED"));
    let offer = run(AntigravityFacts {
        skills_missing: vec!["grill-me".into()],
        ..facts()
    });
    assert_eq!(get(&offer, "antigravity.skills").severity, Severity::Warn);
    let closed = run(AntigravityFacts {
        skills_missing: vec!["grill-me".into()],
        skills_offer_closed: true,
        ..facts()
    });
    assert_eq!(get(&closed, "antigravity.skills").severity, Severity::Block);
}

#[test]
fn posture_blocks_autonomous_and_only_warns_supervised() {
    let a = run(AntigravityFacts {
        posture: Posture::DoesNotMeet,
        mode: Mode::Autonomous,
        ..facts()
    });
    assert_eq!(get(&a, "antigravity.posture").severity, Severity::Block);
    assert_eq!(get(&a, "antigravity.readiness").severity, Severity::Block);
    let s = run(AntigravityFacts {
        posture: Posture::Unknown,
        mode: Mode::Supervised,
        ..facts()
    });
    assert_eq!(get(&s, "antigravity.posture").severity, Severity::Warn);
    assert!(
        get(&s, "antigravity.posture")
            .remediation
            .as_deref()
            .unwrap()
            .contains("acknowledge")
    );
    assert_eq!(get(&s, "antigravity.readiness").severity, Severity::Warn);
}

#[test]
fn the_readiness_summary_is_plain_language_and_says_when_everything_is_fine() {
    let ok = run(facts());
    assert_eq!(get(&ok, "antigravity.readiness").severity, Severity::Pass);
    assert!(
        get(&ok, "antigravity.readiness")
            .detail
            .contains("ready to run unattended")
    );
    let reduced = run(AntigravityFacts {
        uia_consent: false,
        ..facts()
    });
    let r = get(&reduced, "antigravity.readiness");
    assert_eq!(r.severity, Severity::Warn);
    assert!(
        r.detail.contains("UI automation is off")
            && r.detail.contains("reduced unattended readiness")
    );
    assert_eq!(get(&reduced, "antigravity.uia").severity, Severity::Warn);
}

#[test]
fn before_the_probes_have_run_the_provider_blocks_rather_than_guessing() {
    let c = AntigravityChecks::new(AntigravityFactsCell::new())
        .run(&collect_local(std::path::Path::new(".")));
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].severity, Severity::Block);
}

#[test]
fn it_registers_with_the_engine_and_its_rows_appear_in_the_report() {
    let e = crate::orchestrator::test_support::env("pf-agy");
    e.seed_project();
    let engine = crate::preflight::PreflightEngine::new(e.services.clone());
    let cell = AntigravityFactsCell::new();
    cell.set(AntigravityFacts {
        uia_consent: false,
        ..facts()
    });
    engine.register(Arc::new(AntigravityChecks::new(cell)));
    let report = engine
        .run("p1", &collect_local(std::path::Path::new(".")))
        .unwrap();
    assert!(
        report
            .checks
            .iter()
            .any(|c| c.id == "antigravity.readiness" && c.severity == Severity::Warn)
    );
}
