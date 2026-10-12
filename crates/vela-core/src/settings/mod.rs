//! Settings (S02): defaults, user-global settings, project settings, and an immutable per-run snapshot, plus the onboarding
//! choices and consent records.
//!
//! Layer order, lowest to highest: built-in defaults, user-global, project, then the run snapshot (which fixes the values a run
//! uses). A key marked `live` may change during a run; every other key keeps the value the snapshot captured. Project settings
//! live in Vela's store and never in the repository, and repository text can never change a setting: no code path accepts a
//! value from a repository ([`Source::Repository`] is always refused).

use std::collections::BTreeMap;

use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::events::{
    ApprovalAutomationConsentChangedPayload, Event, OnboardingChoiceRecordedPayload,
};
use vela_domain::ids::RunId;
use vela_persistence::journal::NewEvent;
use vela_persistence::repositories::{f06_core, s02_settings};

use crate::orchestrator::Services;

/// The kind of value a key holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// One of a fixed set of words.
    Enum(&'static [&'static str]),
    /// An integer in `min..=max`.
    Int(i64, i64),
    /// `true` or `false`.
    Bool,
}

/// Where a setting may be set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// User-global only.
    User,
    /// User-global default, overridable per project.
    UserOrProject,
}

/// One known setting.
#[derive(Debug, Clone, Copy)]
pub struct Key {
    /// The key.
    pub name: &'static str,
    /// The built-in default.
    pub default: &'static str,
    /// What it holds.
    pub kind: Kind,
    /// Where it may be set.
    pub scope: Scope,
    /// True when a change takes effect in a run already started.
    pub live: bool,
    /// True for onboarding choices (journaled when set).
    pub onboarding: bool,
}

/// Every setting Vela knows. Adding a setting is adding a row.
pub const KEYS: &[Key] = &[
    Key {
        name: "graphics_mode",
        default: "balanced",
        kind: Kind::Enum(&["full", "balanced", "efficiency"]),
        scope: Scope::User,
        live: true,
        onboarding: false,
    },
    Key {
        name: "run_mode",
        default: "supervised",
        kind: Kind::Enum(&["supervised", "autonomous"]),
        scope: Scope::UserOrProject,
        live: false,
        onboarding: false,
    },
    Key {
        name: "max_parallel_workers",
        default: "2",
        kind: Kind::Int(1, 8),
        scope: Scope::UserOrProject,
        live: false,
        onboarding: false,
    },
    Key {
        name: "review_mode",
        default: "off",
        kind: Kind::Enum(&["off", "single"]),
        scope: Scope::UserOrProject,
        live: false,
        onboarding: false,
    },
    Key {
        name: "integration_fix_attempts",
        default: "3",
        kind: Kind::Int(0, 10),
        scope: Scope::UserOrProject,
        live: false,
        onboarding: false,
    },
    Key {
        name: "turn_timeout_minutes",
        default: "30",
        kind: Kind::Int(1, 240),
        scope: Scope::UserOrProject,
        live: false,
        onboarding: false,
    },
    Key {
        name: "background",
        default: "true",
        kind: Kind::Bool,
        scope: Scope::User,
        live: true,
        onboarding: true,
    },
    Key {
        name: "autostart",
        default: "false",
        kind: Kind::Bool,
        scope: Scope::User,
        live: true,
        onboarding: true,
    },
    Key {
        name: "recovery_continuation",
        default: "ask",
        kind: Kind::Enum(&["ask", "continue", "pause"]),
        scope: Scope::User,
        live: true,
        onboarding: true,
    },
];

/// Who is asking for a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The user, through the application.
    User,
    /// Text from a repository, an issue or `AGENTS.md`: never allowed to change anything.
    Repository,
}

fn key(name: &str) -> Result<&'static Key, VelaError> {
    KEYS.iter().find(|k| k.name == name).ok_or_else(|| {
        VelaError::of_class(
            ErrorClass::ValidationFailure,
            format!("`{name}` is not a setting Vela knows"),
        )
    })
}

fn check_value(k: &Key, value: &str) -> Result<(), VelaError> {
    let ok = match k.kind {
        Kind::Enum(options) => options.contains(&value),
        Kind::Int(min, max) => value.parse::<i64>().is_ok_and(|n| (min..=max).contains(&n)),
        Kind::Bool => matches!(value, "true" | "false"),
    };
    if ok {
        Ok(())
    } else {
        Err(VelaError::of_class(
            ErrorClass::ValidationFailure,
            format!("`{value}` is not a valid value for `{}`", k.name),
        ))
    }
}

/// The text of a consent and its version, so the record says what the user actually saw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsentRecord {
    /// What was consented to.
    pub key: String,
    /// The version of the text shown.
    pub text_version: String,
    /// Granted or revoked.
    pub granted: bool,
}

/// Settings over the shared services.
pub struct SettingsService<'a> {
    services: &'a Services,
}

impl<'a> SettingsService<'a> {
    /// Settings over `services`.
    pub fn new(services: &'a Services) -> Self {
        Self { services }
    }

    fn refuse_repository(source: Source) -> Result<(), VelaError> {
        if source == Source::Repository {
            return Err(VelaError::of_class(
                ErrorClass::PolicyBlock,
                "repository, issue and AGENTS.md text cannot change Vela's settings",
            ));
        }
        Ok(())
    }

    /// Sets a user-global setting. Onboarding choices are journaled.
    pub fn set_user(&self, source: Source, name: &str, value: &str) -> Result<(), VelaError> {
        Self::refuse_repository(source)?;
        let k = key(name)?;
        check_value(k, value)?;
        let (name, value, now) = (name.to_owned(), value.to_owned(), self.services.now_ms());
        let onboarding = k.onboarding;
        let event = NewEvent {
            ts: now,
            run_id: None,
            ticket_id: None,
            worker_id: None,
            event: Event::OnboardingChoiceRecorded(OnboardingChoiceRecordedPayload {
                key: name.clone(),
                value: value.clone(),
            }),
        };
        self.services.store.write(move |tx| {
            f06_core::set_setting(tx.conn(), &name, &value, now)?;
            if onboarding {
                tx.append(&event)?;
            }
            Ok(())
        })
    }

    /// Sets a project setting (stored in Vela, never in the repository).
    pub fn set_project(
        &self,
        source: Source,
        project_id: &str,
        name: &str,
        value: &str,
    ) -> Result<(), VelaError> {
        Self::refuse_repository(source)?;
        let k = key(name)?;
        if k.scope != Scope::UserOrProject {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                format!("`{name}` is a user-wide setting and cannot be set per project"),
            ));
        }
        check_value(k, value)?;
        let (project, name, value, now) = (
            project_id.to_owned(),
            name.to_owned(),
            value.to_owned(),
            self.services.now_ms(),
        );
        self.services.store.write(move |tx| {
            s02_settings::set_project_setting(tx.conn(), &project, &name, &value, now)
        })
    }

    /// The effective settings for a project (or none): defaults, then user-global, then project.
    pub fn effective(
        &self,
        project_id: Option<&str>,
    ) -> Result<BTreeMap<String, String>, VelaError> {
        let (global, project) = self.services.store.read(|c| {
            let global = s02_settings::list_global_settings(c)?;
            let project = match project_id {
                Some(p) => s02_settings::list_project_settings(c, p)?,
                None => Vec::new(),
            };
            Ok((global, project))
        })?;
        let mut out: BTreeMap<String, String> = KEYS
            .iter()
            .map(|k| (k.name.to_owned(), k.default.to_owned()))
            .collect();
        for (name, value) in global.into_iter().chain(project) {
            // Unknown or invalid stored values (for example from an older build) are ignored rather than trusted.
            if key(&name).and_then(|k| check_value(k, &value)).is_ok() {
                out.insert(name, value);
            }
        }
        Ok(out)
    }

    /// Captures the settings a run will use. A run has exactly one snapshot; it never changes afterwards.
    pub fn capture_run_snapshot(
        &self,
        run: &RunId,
        project_id: Option<&str>,
    ) -> Result<BTreeMap<String, String>, VelaError> {
        let effective = self.effective(project_id)?;
        let json =
            serde_json::to_string(&effective).map_err(|e| VelaError::internal(e.to_string()))?;
        let (run_text, now) = (run.as_str().to_owned(), self.services.now_ms());
        self.services
            .store
            .write(move |tx| s02_settings::insert_run_snapshot(tx.conn(), &run_text, &json, now))?;
        Ok(effective)
    }

    /// The value a run uses for `name`: the snapshot's value, except for a `live` key, which follows the current setting.
    pub fn run_value(
        &self,
        run: &RunId,
        project_id: Option<&str>,
        name: &str,
    ) -> Result<String, VelaError> {
        let k = key(name)?;
        if k.live {
            return self
                .effective(project_id)?
                .remove(name)
                .ok_or_else(|| VelaError::internal("a known key has no value"));
        }
        let snapshot = self
            .services
            .store
            .read(|c| s02_settings::get_run_snapshot(c, run.as_str()))?
            .ok_or_else(|| {
                VelaError::of_class(
                    ErrorClass::ValidationFailure,
                    "the run has no settings snapshot",
                )
            })?;
        let map: BTreeMap<String, String> =
            serde_json::from_str(&snapshot).map_err(|e| VelaError::internal(e.to_string()))?;
        Ok(map
            .get(name)
            .cloned()
            .unwrap_or_else(|| k.default.to_owned()))
    }

    /// Records a consent decision with the version of the text the user saw, and journals it. Consent to guarded UI automation
    /// uses the dedicated journal event.
    pub fn record_consent(&self, source: Source, consent: &ConsentRecord) -> Result<(), VelaError> {
        Self::refuse_repository(source)?;
        if consent.key.trim().is_empty() || consent.text_version.trim().is_empty() {
            return Err(VelaError::of_class(
                ErrorClass::ValidationFailure,
                "a consent needs a key and the version of the text the user saw",
            ));
        }
        let now = self.services.now_ms();
        let event = if consent.key == "ui_automation" {
            Event::ApprovalAutomationConsentChanged(ApprovalAutomationConsentChangedPayload {
                enabled: consent.granted,
                consent_text_version: consent.text_version.clone(),
            })
        } else {
            Event::OnboardingChoiceRecorded(OnboardingChoiceRecordedPayload {
                key: format!("consent.{}", consent.key),
                value: format!("{}@{}", consent.granted, consent.text_version),
            })
        };
        let new_event = NewEvent {
            ts: now,
            run_id: None,
            ticket_id: None,
            worker_id: None,
            event,
        };
        let c = consent.clone();
        self.services.store.write(move |tx| {
            s02_settings::insert_consent(
                tx.conn(),
                &c.key,
                &c.text_version,
                c.granted,
                "user",
                now,
            )?;
            tx.append(&new_event).map(|_| ())
        })
    }

    /// The current consent for `key`, if one was ever recorded.
    pub fn consent(&self, key: &str) -> Result<Option<ConsentRecord>, VelaError> {
        let found = self
            .services
            .store
            .read(|c| s02_settings::latest_consent(c, key, "user"))?;
        Ok(found.map(|c| ConsentRecord {
            key: c.key,
            text_version: c.text_version,
            granted: c.granted,
        }))
    }
}

#[cfg(test)]
mod tests;
