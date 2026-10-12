//! Login auto-start (L03): available only when the user opted in, and always in step with the setting.

use std::sync::Arc;

use vela_domain::errors::{ErrorClass, VelaError};

use super::{SettingsService, Source};
use crate::orchestrator::Services;

/// The operating system's login-start registration (the Tauri autostart plugin in production).
pub trait AutostartPort: Send + Sync {
    /// Registers Vela to start at login.
    fn enable(&self) -> Result<(), VelaError>;
    /// Removes the registration.
    fn disable(&self) -> Result<(), VelaError>;
    /// Whether the registration currently exists.
    fn is_enabled(&self) -> Result<bool, VelaError>;
}

/// Setting and registration side by side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutostartStatus {
    /// The user's opt-in.
    pub opted_in: bool,
    /// The operating-system registration.
    pub registered: bool,
}

impl AutostartStatus {
    /// True when the registration matches the opt-in.
    pub fn in_step(self) -> bool {
        self.opted_in == self.registered
    }
}

/// Keeps the registration tied to the `autostart` setting.
pub struct AutostartController {
    services: Arc<Services>,
    port: Arc<dyn AutostartPort>,
}

impl AutostartController {
    /// A controller over `services` and the OS `port`.
    pub fn new(services: Arc<Services>, port: Arc<dyn AutostartPort>) -> Self {
        Self { services, port }
    }

    fn opted_in(&self) -> Result<bool, VelaError> {
        Ok(SettingsService::new(&self.services).effective(None)?["autostart"] == "true")
    }

    /// The setting and the registration.
    pub fn status(&self) -> Result<AutostartStatus, VelaError> {
        Ok(AutostartStatus {
            opted_in: self.opted_in()?,
            registered: self.port.is_enabled()?,
        })
    }

    /// The user turns auto-start on or off. The registration is changed first; the setting follows only when it worked, so
    /// the two never disagree after an error.
    pub fn set(&self, enabled: bool) -> Result<AutostartStatus, VelaError> {
        if enabled {
            self.port.enable()?;
        } else {
            self.port.disable()?;
        }
        SettingsService::new(&self.services).set_user(
            Source::User,
            "autostart",
            if enabled { "true" } else { "false" },
        )?;
        self.status()
    }

    /// Brings the registration in line with the setting (run at startup). Never registers without the opt-in; a leftover
    /// registration without one is removed.
    pub fn reconcile(&self) -> Result<AutostartStatus, VelaError> {
        let status = self.status()?;
        if status.opted_in && !status.registered {
            self.port.enable()?;
        } else if !status.opted_in && status.registered {
            self.port.disable()?;
        }
        let after = self.status()?;
        if !after.in_step() {
            return Err(VelaError::of_class(
                ErrorClass::UserActionRequired,
                "the login start registration could not be brought in line with the setting",
            )
            .with_user_action("turn auto-start off and on again in Settings"));
        }
        Ok(after)
    }
}
