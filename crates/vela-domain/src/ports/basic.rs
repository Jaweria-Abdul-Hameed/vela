//! Small synchronous ports: time, identifiers, power, desktop-session probe, notifications.

use crate::ids::{RunId, TicketId, WorkerId};

use super::PortFuture;

/// Source of time. The domain never reads the system clock directly, so time is injectable in tests.
pub trait Clock: Send + Sync {
    /// Current UTC time in epoch milliseconds.
    fn now_ms(&self) -> i64;
}

/// Source of identifiers. The domain never generates randomness itself, so identifiers are deterministic in tests.
pub trait IdGenerator: Send + Sync {
    /// A fresh run identifier.
    fn new_run_id(&self) -> RunId;
    /// A fresh worker identifier.
    fn new_worker_id(&self) -> WorkerId;
    /// A fresh opaque token (operation, command, approval or backup identifiers).
    fn new_token(&self) -> String;
}

/// Proof that the system is being kept awake; hand it back to [`PowerManager::release`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeepAwakeToken(pub u64);

/// Keeps the machine awake while a run is active.
pub trait PowerManager: Send + Sync {
    /// Starts keeping the system awake. A failure is reported and the run continues with a warning.
    fn acquire_keep_awake(&self, reason: &str) -> Result<KeepAwakeToken, crate::errors::VelaError>;
    /// Stops keeping the system awake.
    fn release(&self, token: KeepAwakeToken);
}

/// Whether an interactive desktop is available for UI-automation approval delivery (ADR-012).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionAvailability {
    /// An unlocked interactive desktop is available.
    Available,
    /// The session is locked.
    Locked,
    /// The session is disconnected.
    Disconnected,
    /// The secure desktop is showing (for example a UAC prompt).
    SecureDesktop,
    /// Availability could not be determined; treated as unavailable.
    Unknown,
}

impl SessionAvailability {
    /// Only an explicitly available session permits UI-automation delivery.
    pub fn permits_ui_automation(self) -> bool {
        self == Self::Available
    }
}

/// Probes interactive-desktop availability.
pub trait SessionProbe: Send + Sync {
    /// The current availability.
    fn interactive_session(&self) -> SessionAvailability;
}

/// A desktop notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// Short title.
    pub title: String,
    /// Body text (never contains secrets).
    pub body: String,
    /// The ticket the notification concerns, if any.
    pub ticket: Option<TicketId>,
}

/// Desktop notifications (the only notification channel in the initial release).
pub trait Notifier: Send + Sync {
    /// Shows a notification.
    fn notify<'a>(&'a self, notification: &'a Notification) -> PortFuture<'a, ()>;
}
