//! Delivery of one decision to a Desktop approval card (K06, ADR-018 section 5). After the policy engine has decided, exactly one
//! single-use allow or one refusal is delivered, through `SelectionItemPattern` and `InvokePattern` only, and the outcome is
//! verified. A persistent option ("always allow ...") is never selectable: it is rejected by label before any pattern is touched,
//! and the card is read back after selecting to prove the right option, and no persistent one, is selected at the moment of
//! Submit. Consent is required at the start **and again immediately before each action**, so revoking it stops delivery at once.

use std::sync::Arc;
use std::time::Duration;

use vela_domain::errors::{ErrorCode, VelaError};
use vela_domain::ports::{
    ApprovalDeliveryAdapter, ApprovalObservation, ApprovalSource, DeliveryCapability,
    DeliveryDecision, DeliveryRequest, DeliveryResult, DeliveryTier, EvidenceTier, PortFuture,
};

use crate::discovery::card::{Card, CardOption};
use crate::discovery::correlate::Expectation;
use crate::discovery::model::{Role, Surface};
use crate::discovery::{WarmUp, observe, resolve};

/// What an option does, judged by its label alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionKind {
    /// Allows this one request.
    SingleUse,
    /// Refuses it (and tells the agent to do something else).
    Refusal,
    /// Grants a standing permission. Never selected.
    Persistent,
    /// Not recognised. Never selected.
    Unknown,
}

/// Classifies a label. Persistence wins over everything: a label that mentions "always" or a scope of standing is persistent even
/// if it also says "allow".
pub fn classify_option(label: &str) -> OptionKind {
    let l = label.trim().to_ascii_lowercase();
    const STANDING: [&str; 7] = [
        "always",
        "don't ask",
        "do not ask",
        "never ask",
        "in this conversation",
        "in this project",
        "for this session",
    ];
    if STANDING.iter().any(|s| l.contains(s)) {
        return OptionKind::Persistent;
    }
    if l.starts_with("yes") && (l.contains("this time") || l.contains("once")) {
        return OptionKind::SingleUse;
    }
    if l == "no" || l.starts_with("no ") || l.starts_with("no(") || l.starts_with("no,") {
        return OptionKind::Refusal;
    }
    OptionKind::Unknown
}

/// Picks the one option for `decision`. Exactly one candidate of the right kind must exist; none or several is ambiguity and
/// fails closed.
pub fn choose<'a>(card: &'a Card, decision: DeliveryDecision) -> Result<&'a CardOption, VelaError> {
    let wanted = match decision {
        DeliveryDecision::AllowOnce => OptionKind::SingleUse,
        DeliveryDecision::Deny => OptionKind::Refusal,
    };
    let mut hits = card
        .options
        .iter()
        .filter(|o| classify_option(&o.label) == wanted);
    match (hits.next(), hits.next()) {
        (Some(one), None) => Ok(one),
        (None, _) => Err(VelaError::from_code(
            ErrorCode::ApprovalTargetAmbiguous,
            "the card offers no recognisable option for this decision",
        )),
        _ => Err(VelaError::from_code(
            ErrorCode::ApprovalTargetAmbiguous,
            "the card offers more than one option that could be this decision",
        )),
    }
}

/// The consent the user gave for UI automation (ADR-010), checked live.
pub trait Consent: Send + Sync {
    /// True while the user's consent stands.
    fn granted(&self) -> bool;
}

impl Consent for std::sync::atomic::AtomicBool {
    fn granted(&self) -> bool {
        self.load(std::sync::atomic::Ordering::SeqCst)
    }
}

fn require(consent: &dyn Consent) -> Result<(), VelaError> {
    if consent.granted() {
        Ok(())
    } else {
        Err(VelaError::from_code(
            ErrorCode::ApprovalUndeliverable,
            "consent for UI automation is not given or was revoked",
        ))
    }
}

/// How long to wait for the card to clear after Submit.
#[derive(Debug, Clone, Copy)]
pub struct Verify {
    /// Reads of the window.
    pub attempts: u32,
    /// Pause between reads.
    pub delay: Duration,
}

impl Default for Verify {
    fn default() -> Self {
        Self {
            attempts: 20,
            delay: Duration::from_millis(300),
        }
    }
}

/// Delivers `decision` to the card correlated with `expect` in process `pid`.
pub fn deliver(
    surface: &dyn Surface,
    consent: &dyn Consent,
    pid: u32,
    expect: &Expectation,
    decision: DeliveryDecision,
    warm: WarmUp,
    verify: Verify,
    sleep: &dyn Fn(Duration),
) -> Result<DeliveryResult, VelaError> {
    require(consent)?;
    let observed = observe(surface, pid, expect, warm, sleep)?;
    let option = choose(&observed.card, decision)?.clone();

    // Re-read, then confirm the controls are still the ones we chose, before touching anything.
    require(consent)?;
    let tree = surface.tree(&observed.window)?;
    resolve(&tree, &option.path, Role::Radio, &option.label)?;
    resolve(&tree, &observed.card.submit, Role::Button, "Submit").or_else(|_| {
        // The submit label is matched case-insensitively when finding the card; use what was read.
        let n = tree
            .at(&observed.card.submit)
            .filter(|n| n.role == Role::Button && n.name.trim().eq_ignore_ascii_case("submit"));
        n.ok_or_else(|| {
            VelaError::from_code(
                ErrorCode::ApprovalTargetAmbiguous,
                "the submit button changed since it was read",
            )
        })
    })?;

    require(consent)?;
    surface.select(&observed.window, &option.path)?;

    // Prove what is selected now: ours, and no persistent option.
    let after = surface.tree(&observed.window)?;
    for o in &observed.card.options {
        let node = after.at(&o.path);
        let selected = node.is_some_and(|n| n.selected);
        match (o.path == option.path, selected) {
            (true, false) => {
                return Err(VelaError::from_code(
                    ErrorCode::ApprovalDeliveryFailed,
                    "the chosen option did not become selected",
                ));
            }
            (false, true) if classify_option(&o.label) == OptionKind::Persistent => {
                return Err(VelaError::from_code(
                    ErrorCode::ApprovalDeliveryFailed,
                    "a persistent option is selected; nothing was submitted",
                ));
            }
            (false, true) => {
                return Err(VelaError::from_code(
                    ErrorCode::ApprovalDeliveryFailed,
                    "more than one option is selected; nothing was submitted",
                ));
            }
            _ => {}
        }
    }

    require(consent)?;
    surface.invoke(&observed.window, &observed.card.submit)?;

    for attempt in 0..verify.attempts.max(1) {
        let tree = surface.tree(&observed.window)?;
        let still_there = crate::discovery::card::find_cards(&tree)
            .into_iter()
            .filter_map(Result::ok)
            .any(|c| c.command == observed.card.command && c.title == observed.card.title);
        if !still_there {
            return Ok(DeliveryResult {
                verified: true,
                detail: format!(
                    "delivered {decision:?} by option {:?}; the card cleared",
                    option.label
                ),
            });
        }
        if attempt + 1 < verify.attempts {
            sleep(verify.delay);
        }
    }
    Ok(DeliveryResult {
        verified: false,
        detail: "submitted, but the card was still showing afterwards".to_owned(),
    })
}

/// The Desktop approval source and delivery adapter over a [`Surface`]. It is dormant unless a Vela-correlated Desktop session is
/// registered through [`DesktopApproval::expect`].
pub struct DesktopApproval {
    surface: Arc<dyn Surface>,
    consent: Arc<dyn Consent>,
    target: std::sync::Mutex<Option<(u32, Expectation)>>,
    warm: WarmUp,
    verify: Verify,
}

impl DesktopApproval {
    /// An adapter over `surface` that acts only while `consent` stands.
    pub fn new(surface: Arc<dyn Surface>, consent: Arc<dyn Consent>) -> Self {
        Self {
            surface,
            consent,
            target: std::sync::Mutex::new(None),
            warm: WarmUp::default(),
            verify: Verify::default(),
        }
    }

    /// Faster timings for tests.
    pub fn with_timing(mut self, warm: WarmUp, verify: Verify) -> Self {
        self.warm = warm;
        self.verify = verify;
        self
    }

    /// Declares the process and request Vela is waiting on. Without one nothing is observed or delivered.
    pub fn expect(&self, pid: u32, expectation: Expectation) {
        *self.target.lock().expect("target lock") = Some((pid, expectation));
    }

    /// Forgets the request.
    pub fn clear(&self) {
        *self.target.lock().expect("target lock") = None;
    }

    fn target(&self) -> Option<(u32, Expectation)> {
        self.target.lock().expect("target lock").clone()
    }
}

impl ApprovalSource for DesktopApproval {
    fn next_observation(&self) -> PortFuture<'_, Option<ApprovalObservation>> {
        Box::pin(async move {
            let Some((pid, expect)) = self.target() else {
                return Ok(None);
            };
            match observe(&*self.surface, pid, &expect, self.warm, &std::thread::sleep) {
                Ok(o) => Ok(Some(ApprovalObservation {
                    source_id: format!("uia:{}:{}", o.window.handle, o.card.title),
                    session_identity: Some(format!("pid:{pid}")),
                    operation: o.card.command.clone(),
                    working_directory: o.card.working_dir.clone(),
                    options: o.card.options.iter().map(|x| x.label.clone()).collect(),
                    evidence: EvidenceTier::Observed,
                })),
                Err(e) if e.code() == Some(ErrorCode::ApprovalSurfaceNotFound) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }
}

impl ApprovalDeliveryAdapter for DesktopApproval {
    fn detect(&self) -> PortFuture<'_, DeliveryCapability> {
        Box::pin(async move {
            let available =
                self.consent.granted() && self.target().is_some() && self.surface.windows().is_ok();
            Ok(DeliveryCapability {
                tier: DeliveryTier::UiAutomation,
                available,
            })
        })
    }

    fn deliver<'a>(&'a self, request: &'a DeliveryRequest) -> PortFuture<'a, DeliveryResult> {
        Box::pin(async move {
            let (pid, expect) = self.target().ok_or_else(|| {
                VelaError::from_code(
                    ErrorCode::ApprovalUndeliverable,
                    "no Desktop session is registered to answer",
                )
            })?;
            // The request must be about what we are watching: re-correlate, never trust the caller's word.
            if request
                .observation
                .operation
                .split_whitespace()
                .collect::<Vec<_>>()
                != expect.command.split_whitespace().collect::<Vec<_>>()
            {
                return Err(VelaError::from_code(
                    ErrorCode::ApprovalWindowMismatch,
                    "the request is not the one the Desktop card is showing",
                ));
            }
            deliver(
                &*self.surface,
                &*self.consent,
                pid,
                &expect,
                request.decision,
                self.warm,
                self.verify,
                &std::thread::sleep,
            )
        })
    }

    fn cancel<'a>(&'a self, _observation: &'a ApprovalObservation) -> PortFuture<'a, ()> {
        Box::pin(async move {
            self.clear();
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests;
