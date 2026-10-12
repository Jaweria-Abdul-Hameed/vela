//! First-run onboarding (L01): explicit, conservative choices; consent recorded with the version of the text shown; revoke
//! flows for the Settings sheet.

use vela_domain::errors::{ErrorClass, VelaError};

use super::{ConsentRecord, SettingsService, Source};
use crate::orchestrator::Services;

/// One consent the application can ask for.
#[derive(Debug, Clone, Copy)]
pub struct ConsentText {
    /// The consent key.
    pub key: &'static str,
    /// The version of `text`; bump it whenever the wording changes so earlier consent no longer counts.
    pub version: &'static str,
    /// What the user is shown.
    pub text: &'static str,
}

/// Every consent text in force.
pub const CONSENTS: &[ConsentText] = &[
    ConsentText {
        key: "ui_automation",
        version: "2026-10-v1",
        text: "Let Vela click Allow on Antigravity approval prompts that match your rules, only while you are signed in and the desktop is unlocked.",
    },
    ConsentText {
        key: "background",
        version: "2026-10-v1",
        text: "Keep Vela running in the notification area after the window is closed so runs can continue.",
    },
    ConsentText {
        key: "auto_merge",
        version: "2026-10-v1",
        text: "WARNING: when a run finishes, Vela will merge the pull request into the default branch without waiting for you. Only turn this on for projects where that is safe.",
    },
    ConsentText {
        key: "autostart",
        version: "2026-10-v1",
        text: "Start Vela when you sign in to Windows.",
    },
];

fn consent_text(key: &str) -> Result<&'static ConsentText, VelaError> {
    CONSENTS.iter().find(|c| c.key == key).ok_or_else(|| {
        VelaError::of_class(
            ErrorClass::ValidationFailure,
            format!("`{key}` is not a consent Vela asks for"),
        )
    })
}

/// What the user chose on the onboarding sheet. Every field defaults to the conservative answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnboardingChoices {
    /// Keep running in the background after the window closes.
    pub background: bool,
    /// Start at login.
    pub autostart: bool,
    /// Allow guarded UI-automation approvals.
    pub ui_automation: bool,
    /// What to do about interrupted runs at startup: `ask`, `continue` or `pause`.
    pub recovery_continuation: String,
}

impl Default for OnboardingChoices {
    fn default() -> Self {
        Self {
            background: false,
            autostart: false,
            ui_automation: false,
            recovery_continuation: "ask".to_owned(),
        }
    }
}

/// Onboarding over the shared services.
pub struct Onboarding<'a> {
    services: &'a Services,
}

impl<'a> Onboarding<'a> {
    /// Onboarding over `services`.
    pub fn new(services: &'a Services) -> Self {
        Self { services }
    }

    /// True until the user has finished the first-run sheet.
    pub fn needed(&self) -> Result<bool, VelaError> {
        Ok(SettingsService::new(self.services).effective(None)?["onboarding_complete"] != "true")
    }

    /// Records the choices, each consent with the text version shown, then marks onboarding complete.
    pub fn complete(&self, choices: &OnboardingChoices) -> Result<(), VelaError> {
        let settings = SettingsService::new(self.services);
        for (key, granted) in [
            ("background", choices.background),
            ("autostart", choices.autostart),
            ("ui_automation", choices.ui_automation),
        ] {
            self.record(&settings, key, granted)?;
        }
        settings.set_user(
            Source::User,
            "recovery_continuation",
            &choices.recovery_continuation,
        )?;
        settings.set_user(Source::User, "onboarding_complete", "true")
    }

    fn record(
        &self,
        settings: &SettingsService<'_>,
        key: &str,
        granted: bool,
    ) -> Result<(), VelaError> {
        let text = consent_text(key)?;
        settings.record_consent(
            Source::User,
            &ConsentRecord {
                key: key.to_owned(),
                text_version: text.version.to_owned(),
                granted,
            },
        )?;
        // The two consents that are also settings keep the setting in step with the decision.
        if matches!(key, "background" | "autostart") {
            settings.set_user(Source::User, key, if granted { "true" } else { "false" })?;
        }
        Ok(())
    }

    /// Withdraws a consent (the Settings sheet's Automation section). Revoking is always allowed and takes effect at once.
    pub fn revoke(&self, key: &str) -> Result<(), VelaError> {
        self.record(&SettingsService::new(self.services), key, false)
    }

    /// Grants a consent again, recording the current text version.
    pub fn grant(&self, key: &str) -> Result<(), VelaError> {
        self.record(&SettingsService::new(self.services), key, true)
    }

    /// True when `key` is granted under the **current** text version. Consent given to older wording does not count.
    pub fn is_granted(&self, key: &str) -> Result<bool, VelaError> {
        let text = consent_text(key)?;
        Ok(SettingsService::new(self.services)
            .consent(key)?
            .is_some_and(|c| c.granted && c.text_version == text.version))
    }
}
