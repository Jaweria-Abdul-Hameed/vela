//! Antigravity preflight providers and unattended readiness (K10, `PREFLIGHT.md`, ADR-009, ADR-010, ADR-012).
//!
//! Antigravity is the default production runtime, so preflight reports its concrete state rather than a generic "adapter
//! available": the installation and capability contract (CAP-01..12), sign-in, skills, the effective native permission posture,
//! whether guarded UI automation is enabled and capable, the interactive-session conditions, and the resulting unattended
//! readiness per mode. The facts come from the adapter probes (the composition root fills the shared cell before each run); the
//! providers themselves are pure, as the engine requires.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use vela_domain::ports::SessionAvailability;

use super::{Check, CheckProvider, PreflightFacts, Severity};

/// Where the sign-in stands, as judged by the CLI's own answer (never by reading credentials).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Auth {
    /// Signed in.
    SignedIn,
    /// Not signed in: `AUTH_REQUIRED`.
    SignedOut,
    /// Could not be determined.
    Unknown,
}

/// The effective native permission posture (ADR-009).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Posture {
    /// Verified: deny blocks, unlisted is denied, allow runs, hook failure blocks.
    Meets,
    /// Verified not to hold.
    DoesNotMeet,
    /// Not determined.
    Unknown,
}

/// The run's autonomy mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Policy-allowed routine requests are delivered; sensitive ones ask.
    Supervised,
    /// A wider project-scoped allow policy.
    Autonomous,
}

/// What the adapter probes found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AntigravityFacts {
    /// `agy` is installed and runs.
    pub installed: bool,
    /// The `agy` version.
    pub agy_version: Option<String>,
    /// The Desktop version, when installed.
    pub desktop_version: Option<String>,
    /// Which capabilities are present, `CAP-01`..`CAP-12`.
    pub capabilities: BTreeMap<String, bool>,
    /// Sign-in state.
    pub auth: Auth,
    /// Required skills that are not installed.
    pub skills_missing: Vec<String>,
    /// The install of missing skills was offered and failed or was declined (otherwise it is still just an offer).
    pub skills_offer_closed: bool,
    /// The native permission posture.
    pub posture: Posture,
    /// The user enabled guarded UI automation (ADR-010).
    pub uia_consent: bool,
    /// The UI Automation capability probe passed.
    pub uia_capable: bool,
    /// The interactive-session state right now.
    pub session: SessionAvailability,
    /// Other conditions that will interrupt unattended operation (display sleep policy, updates pending, ...).
    pub unattended_interruptions: Vec<String>,
    /// The run's mode.
    pub mode: Mode,
}

/// A plain-language reason attached to a readiness verdict.
pub type Reason = String;

/// Unattended readiness per mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Readiness {
    /// For an Autonomous run.
    pub autonomous: Severity,
    /// For a Supervised run.
    pub supervised: Severity,
    /// Why, in words.
    pub reasons: Vec<Reason>,
}

impl Readiness {
    /// The verdict for `mode`.
    pub fn for_mode(&self, mode: Mode) -> Severity {
        match mode {
            Mode::Autonomous => self.autonomous,
            Mode::Supervised => self.supervised,
        }
    }
}

/// The readiness table (AT-015). Posture gates everything; UI-automation consent and capability decide how much of the unattended
/// operation survives prompts the native path cannot answer.
pub fn readiness(f: &AntigravityFacts) -> Readiness {
    let mut reasons = Vec::new();
    let mut autonomous = Severity::Pass;
    let mut supervised = Severity::Pass;
    let mut raise = |a: Severity, s: Severity, why: String| {
        autonomous = autonomous.max(a);
        supervised = supervised.max(s);
        reasons.push(why);
    };

    if !f.installed {
        raise(
            Severity::Block,
            Severity::Block,
            "Antigravity (agy) is not installed or does not run".into(),
        );
    }
    match f.auth {
        Auth::SignedOut => raise(
            Severity::Block,
            Severity::Block,
            "you are not signed in to Antigravity".into(),
        ),
        Auth::Unknown => raise(
            Severity::Warn,
            Severity::Warn,
            "Vela could not tell whether you are signed in".into(),
        ),
        Auth::SignedIn => {}
    }
    match f.posture {
        Posture::Meets => {}
        Posture::DoesNotMeet => raise(
            Severity::Block,
            Severity::Warn,
            "Antigravity's permission settings let some actions run without Vela's policy; Supervised mode needs your acknowledgement and Autonomous is not available".into(),
        ),
        Posture::Unknown => raise(
            Severity::Block,
            Severity::Warn,
            "Vela could not verify Antigravity's permission behavior; Supervised mode needs your acknowledgement and Autonomous is not available".into(),
        ),
    }
    if f.posture == Posture::Meets {
        if !f.uia_consent {
            raise(
                Severity::Warn,
                Severity::Pass,
                "UI automation is off, so prompts the native path cannot answer will wait for you (reduced unattended readiness)".into(),
            );
        } else if !f.uia_capable {
            raise(
                Severity::Warn,
                Severity::Pass,
                "UI automation is on but not available on this machine, so such prompts will wait for you".into(),
            );
        }
    }
    if f.uia_consent && f.uia_capable && !f.session.permits_ui_automation() {
        raise(
            Severity::Warn,
            Severity::Warn,
            "the desktop is locked or disconnected, so UI-automation approvals will pause until you return".into(),
        );
    }
    for why in &f.unattended_interruptions {
        raise(Severity::Warn, Severity::Warn, why.clone());
    }
    Readiness {
        autonomous,
        supervised,
        reasons,
    }
}

/// The adapter facts the providers read; the composition root fills it before each preflight run.
#[derive(Debug, Clone, Default)]
pub struct AntigravityFactsCell(Arc<Mutex<Option<AntigravityFacts>>>);

impl AntigravityFactsCell {
    /// An empty cell.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the latest probe results.
    pub fn set(&self, facts: AntigravityFacts) {
        *self.0.lock().expect("facts lock") = Some(facts);
    }

    fn get(&self) -> Option<AntigravityFacts> {
        self.0.lock().expect("facts lock").clone()
    }
}

/// CAP-01..07 are required (CAP-07 only limits concurrency); CAP-08..10 reduce capability; CAP-11 is the posture; CAP-12 is
/// informational.
fn cap_check(id: &str, present: bool) -> Check {
    let n: u32 = id.trim_start_matches("CAP-").parse().unwrap_or(0);
    let cid = format!("antigravity.{}", id.to_ascii_lowercase());
    let title = format!("Antigravity {id}");
    match (present, n) {
        (true, _) => Check::pass(&cid, &title, "present"),
        (false, 7) => Check::new(
            &cid,
            &title,
            Severity::Warn,
            "no concurrency limit is reported, so Vela runs one worker at a time",
            None,
        ),
        (false, 1..=6) => Check::new(
            &cid,
            &title,
            Severity::Block,
            "a required Antigravity capability is missing",
            Some("update Antigravity, then run preflight again"),
        ),
        (false, 8..=10) => Check::new(
            &cid,
            &title,
            Severity::Warn,
            "a capability is missing; Vela works with reduced capability",
            None,
        ),
        (false, _) => Check::new(
            &cid,
            &title,
            Severity::Pass,
            "not available (informational)",
            None,
        ),
    }
}

/// The Antigravity providers, registered with the preflight engine as one provider (`antigravity`).
pub struct AntigravityChecks {
    cell: AntigravityFactsCell,
}

impl AntigravityChecks {
    /// A provider over `cell`.
    pub fn new(cell: AntigravityFactsCell) -> Self {
        Self { cell }
    }
}

impl CheckProvider for AntigravityChecks {
    fn id(&self) -> &'static str {
        "antigravity"
    }

    fn run(&self, _facts: &PreflightFacts) -> Vec<Check> {
        let Some(f) = self.cell.get() else {
            return vec![Check::new(
                "antigravity.probe",
                "Antigravity",
                Severity::Block,
                "Antigravity has not been checked yet",
                Some("run preflight again"),
            )];
        };
        let mut out = Vec::new();
        out.push(if f.installed {
            Check::pass(
                "antigravity.install",
                "Antigravity installed",
                format!(
                    "agy {}{}",
                    f.agy_version.as_deref().unwrap_or("(version unknown)"),
                    f.desktop_version
                        .as_deref()
                        .map(|d| format!(", Desktop {d}"))
                        .unwrap_or_default()
                ),
            )
        } else {
            Check::new(
                "antigravity.install",
                "Antigravity installed",
                Severity::Block,
                "agy was not found or did not run",
                Some("install Antigravity and sign in once"),
            )
        });
        for (id, present) in &f.capabilities {
            out.push(cap_check(id, *present));
        }
        out.push(match f.auth {
            Auth::SignedIn => Check::pass("antigravity.auth", "Antigravity sign-in", "signed in"),
            Auth::SignedOut => Check::new(
                "antigravity.auth",
                "Antigravity sign-in",
                Severity::Block,
                "AUTH_REQUIRED: you are not signed in",
                Some("open Antigravity and sign in with your Google account; Vela never handles your credentials"),
            ),
            Auth::Unknown => Check::new(
                "antigravity.auth",
                "Antigravity sign-in",
                Severity::Warn,
                "the sign-in state could not be determined",
                Some("run `agy` once interactively"),
            ),
        });
        out.push(if f.skills_missing.is_empty() {
            Check::pass(
                "antigravity.skills",
                "Antigravity skills",
                "all required skills are installed",
            )
        } else {
            Check::new(
                "antigravity.skills",
                "Antigravity skills",
                if f.skills_offer_closed {
                    Severity::Block
                } else {
                    Severity::Warn
                },
                format!("missing: {}", f.skills_missing.join(", ")),
                Some("let Vela install the skills, or install them yourself"),
            )
        });
        let mode_name = match f.mode {
            Mode::Autonomous => "Autonomous",
            Mode::Supervised => "Supervised",
        };
        out.push(match (f.posture, f.mode) {
            (Posture::Meets, _) => Check::pass(
                "antigravity.posture",
                "Native permission posture",
                "Antigravity enforces Vela's policy natively (verified)",
            ),
            (p, Mode::Autonomous) => Check::new(
                "antigravity.posture",
                "Native permission posture",
                Severity::Block,
                format!(
                    "{} for {mode_name} mode",
                    if p == Posture::Unknown {
                        "could not be verified"
                    } else {
                        "does not hold"
                    }
                ),
                Some("Supervised mode can proceed with your acknowledgement"),
            ),
            (p, Mode::Supervised) => Check::new(
                "antigravity.posture",
                "Native permission posture",
                Severity::Warn,
                format!(
                    "{}; Vela cannot enforce policy over actions Antigravity runs on its own",
                    if p == Posture::Unknown {
                        "could not be verified"
                    } else {
                        "does not hold"
                    }
                ),
                Some("acknowledge to continue in Supervised mode"),
            ),
        });
        out.push(match (f.uia_consent, f.uia_capable) {
            (true, true) => Check::pass("antigravity.uia", "UI automation", "enabled and available"),
            (true, false) => Check::new(
                "antigravity.uia",
                "UI automation",
                Severity::Warn,
                "enabled, but UI Automation is not available on this machine",
                None,
            ),
            (false, _) => Check::new(
                "antigravity.uia",
                "UI automation",
                Severity::Warn,
                "off: reduced unattended readiness (prompts the native path cannot answer will wait for you)",
                Some("you can enable it in Settings"),
            ),
        });
        out.push(if f.uia_consent && f.uia_capable && !f.session.permits_ui_automation() {
            Check::new(
                "antigravity.session",
                "Interactive session",
                Severity::Warn,
                "the desktop is locked or disconnected; UI-automation approvals pause until you return",
                None,
            )
        } else {
            Check::pass("antigravity.session", "Interactive session", "ok")
        });
        let r = readiness(&f);
        let verdict = r.for_mode(f.mode);
        out.push(Check::new(
            "antigravity.readiness",
            "Unattended readiness",
            verdict,
            if r.reasons.is_empty() {
                format!("ready to run unattended in {mode_name} mode")
            } else {
                r.reasons.join("; ")
            },
            None,
        ));
        out
    }
}

#[cfg(test)]
mod tests;
