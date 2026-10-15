//! Approval wiring (K07): assembles the approval path in the composition root.
//!
//! The UI Automation source and delivery are constructed **only** when the user's persisted consent is on and the capability
//! probe passes (ADR-010). Consent is read live for every action, so revoking it in Settings stops delivery at once. The broker
//! (K01) is always built (native delivery needs no consent); the progress watchdog (K02) and the interactive-session guard (P03)
//! are created beside it so the run driver can poll them.

use std::sync::Arc;

use vela_core::broker::Broker;
use vela_core::broker::session::SessionGuard;
use vela_core::orchestrator::Services;
use vela_core::settings::onboarding::Onboarding;
use vela_domain::ports::{ApprovalDeliveryAdapter, ApprovalSource, SessionProbe};

/// The consent key (see `settings::onboarding::consents`).
pub const UI_AUTOMATION: &str = "ui_automation";

/// The user's persisted consent for UI automation, read on every call.
pub struct PersistedConsent {
    services: Arc<Services>,
}

impl PersistedConsent {
    /// Consent as stored in settings.
    pub fn new(services: Arc<Services>) -> Self {
        Self { services }
    }
}

#[cfg(windows)]
impl vela_uia::delivery::Consent for PersistedConsent {
    fn granted(&self) -> bool {
        Onboarding::new(&self.services)
            .is_granted(UI_AUTOMATION)
            .unwrap_or(false)
    }
}

/// What the composition root gets.
pub struct ApprovalPath {
    /// The broker, with UI-automation delivery when it is enabled and capable.
    pub broker: Arc<Broker>,
    /// The Desktop approval source, when UI automation is enabled and capable.
    pub source: Option<Arc<dyn ApprovalSource>>,
    /// Watches the interactive session for workers whose approvals depend on UI automation.
    pub session: Arc<SessionGuard>,
    /// Whether UI automation was wired.
    pub ui_automation: bool,
}

/// Whether UI automation may be wired right now: consent on and the capability probe passing.
pub fn ui_automation_available(services: &Services, capability_ok: bool) -> bool {
    capability_ok
        && Onboarding::new(services)
            .is_granted(UI_AUTOMATION)
            .unwrap_or(false)
}

/// Builds the approval path. `capability_ok` is the result of the UI Automation capability probe (K10).
pub fn build(
    services: Arc<Services>,
    probe: Arc<dyn SessionProbe>,
    capability_ok: bool,
) -> ApprovalPath {
    let wire_uia = ui_automation_available(&services, capability_ok);
    let (delivery, source) = uia_parts(&services, wire_uia);
    let name = if delivery.is_some() { "uia" } else { "native" };
    let broker = Arc::new(Broker::new(services.clone(), delivery, name));
    ApprovalPath {
        broker,
        ui_automation: source.is_some(),
        source,
        session: Arc::new(SessionGuard::new(services, probe)),
    }
}

type Parts = (
    Option<Arc<dyn ApprovalDeliveryAdapter>>,
    Option<Arc<dyn ApprovalSource>>,
);

#[cfg(windows)]
fn uia_parts(services: &Arc<Services>, wire: bool) -> Parts {
    if !wire {
        return (None, None);
    }
    let desktop = Arc::new(vela_uia::delivery::DesktopApproval::new(
        Arc::new(vela_uia::discovery::win::WinSurface),
        Arc::new(PersistedConsent::new(services.clone())),
    ));
    (Some(desktop.clone()), Some(desktop))
}

#[cfg(not(windows))]
fn uia_parts(_services: &Arc<Services>, _wire: bool) -> Parts {
    (None, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_testkit::fakes::FakeSessionProbe;

    fn services(tag: &str) -> Arc<Services> {
        let dir = std::env::temp_dir().join(format!("vela-approval-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = vela_persistence::writer::StateStore::open(
            &dir.join("state.db"),
            vela_persistence::writer::StoreOptions::default(),
        )
        .unwrap();
        Services::new(
            Arc::new(store),
            Arc::new(vela_core::orchestrator::system::SystemClock),
            Arc::new(vela_core::orchestrator::system::SystemIds::new()),
        )
    }

    #[test]
    fn ui_automation_is_not_wired_without_consent_even_when_capable() {
        let s = services("approval-no-consent");
        let path = build(
            s.clone(),
            Arc::new(FakeSessionProbe::new(
                vela_domain::ports::SessionAvailability::Available,
            )),
            true,
        );
        assert!(!path.ui_automation && path.source.is_none());
        assert!(!ui_automation_available(&s, true));
    }

    #[test]
    fn it_is_not_wired_when_the_capability_probe_fails_even_with_consent() {
        let s = services("approval-no-capability");
        Onboarding::new(&s).grant(UI_AUTOMATION).unwrap();
        assert!(!ui_automation_available(&s, false));
        let path = build(s, Arc::new(FakeSessionProbe::new()), false);
        assert!(!path.ui_automation);
    }

    #[cfg(windows)]
    #[test]
    fn with_consent_and_capability_it_is_wired_and_revocation_takes_effect_live() {
        use vela_uia::delivery::Consent;
        let s = services("approval-wired");
        Onboarding::new(&s).grant(UI_AUTOMATION).unwrap();
        let path = build(s.clone(), Arc::new(FakeSessionProbe::new()), true);
        assert!(path.ui_automation && path.source.is_some());
        let live = PersistedConsent::new(s.clone());
        assert!(live.granted());
        Onboarding::new(&s).revoke(UI_AUTOMATION).unwrap();
        assert!(
            !live.granted(),
            "revoking consent stops delivery immediately"
        );
    }
}
