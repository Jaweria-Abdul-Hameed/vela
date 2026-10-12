use std::sync::{Arc, Mutex};

use vela_domain::errors::VelaError;

use super::autostart::{AutostartController, AutostartPort};
use super::onboarding::{CONSENTS, Onboarding, OnboardingChoices};
use super::*;
use crate::orchestrator::test_support::env;

#[derive(Default)]
struct FakeOs {
    on: Mutex<bool>,
    fail: Mutex<bool>,
}

impl AutostartPort for FakeOs {
    fn enable(&self) -> Result<(), VelaError> {
        if *self.fail.lock().unwrap() {
            return Err(VelaError::internal("registry is read-only"));
        }
        *self.on.lock().unwrap() = true;
        Ok(())
    }
    fn disable(&self) -> Result<(), VelaError> {
        *self.on.lock().unwrap() = false;
        Ok(())
    }
    fn is_enabled(&self) -> Result<bool, VelaError> {
        Ok(*self.on.lock().unwrap())
    }
}

#[test]
fn the_first_run_is_needed_until_completed_and_defaults_are_conservative() {
    let e = env("onboard");
    let o = Onboarding::new(&e.services);
    assert!(o.needed().unwrap());
    assert_eq!(
        OnboardingChoices::default(),
        OnboardingChoices {
            background: false,
            autostart: false,
            ui_automation: false,
            recovery_continuation: "ask".into()
        }
    );
    o.complete(&OnboardingChoices::default()).unwrap();
    assert!(!o.needed().unwrap());
    for c in CONSENTS {
        assert!(!o.is_granted(c.key).unwrap(), "{} must default off", c.key);
    }
    let s = SettingsService::new(&e.services).effective(None).unwrap();
    assert_eq!(s["background"], "false");
    assert_eq!(s["autostart"], "false");
}

#[test]
fn consent_records_the_text_version_and_revoking_takes_effect() {
    let e = env("consent-flow");
    let o = Onboarding::new(&e.services);
    o.complete(&OnboardingChoices {
        background: true,
        autostart: false,
        ui_automation: true,
        recovery_continuation: "auto_safe".into(),
    })
    .unwrap();
    assert!(o.is_granted("ui_automation").unwrap());
    assert!(o.is_granted("background").unwrap());
    let recorded = SettingsService::new(&e.services)
        .consent("ui_automation")
        .unwrap()
        .unwrap();
    assert_eq!(
        recorded.text_version,
        CONSENTS
            .iter()
            .find(|c| c.key == "ui_automation")
            .unwrap()
            .version
    );
    o.revoke("ui_automation").unwrap();
    assert!(!o.is_granted("ui_automation").unwrap());
    o.revoke("background").unwrap();
    assert_eq!(
        SettingsService::new(&e.services).effective(None).unwrap()["background"],
        "false"
    );
    assert_eq!(
        SettingsService::new(&e.services).effective(None).unwrap()["recovery_continuation"],
        "auto_safe"
    );
    assert!(o.revoke("nonsense").is_err());
}

#[test]
fn consent_given_to_older_wording_does_not_count() {
    let e = env("old-consent");
    SettingsService::new(&e.services)
        .record_consent(
            Source::User,
            &ConsentRecord {
                key: "ui_automation".into(),
                text_version: "2024-old".into(),
                granted: true,
            },
        )
        .unwrap();
    assert!(
        !Onboarding::new(&e.services)
            .is_granted("ui_automation")
            .unwrap()
    );
}

#[test]
fn autostart_follows_the_opt_in_and_never_registers_without_it() {
    let e = env("autostart");
    let os = Arc::new(FakeOs::default());
    let c = AutostartController::new(e.services.clone(), os.clone());
    // A leftover registration without an opt-in is removed.
    os.enable().unwrap();
    let s = c.reconcile().unwrap();
    assert!(!s.registered && !s.opted_in);
    // Opting in registers; reconcile keeps it.
    assert!(c.set(true).unwrap().registered);
    assert!(c.reconcile().unwrap().in_step());
    // Registration lost (for example by a cleanup tool): reconcile restores only because of the opt-in.
    os.disable().unwrap();
    assert!(c.reconcile().unwrap().registered);
    assert!(!c.set(false).unwrap().registered);
    assert_eq!(
        SettingsService::new(&e.services).effective(None).unwrap()["autostart"],
        "false"
    );
}

#[test]
fn a_failed_registration_leaves_the_setting_unchanged() {
    let e = env("autostart-fail");
    let os = Arc::new(FakeOs::default());
    *os.fail.lock().unwrap() = true;
    let c = AutostartController::new(e.services.clone(), os);
    assert!(c.set(true).is_err());
    assert!(!c.status().unwrap().opted_in);
}
