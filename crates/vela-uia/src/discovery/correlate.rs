//! Correlation (K05, ADR-018): before anything acts, the card must be shown to belong to the request Vela is answering. Window
//! title, the assembled command text, the working directory and the status text all have to agree; a missing or partial match
//! fails closed. Matching is exact after whitespace and path-separator normalisation: a prefix, a substring or "looks similar" is
//! not a match.

use super::card::{Card, WAITING};
use super::model::WindowInfo;

/// What the request Vela is answering says the card should show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expectation {
    /// Text the window title must contain (the workspace or conversation marker Vela correlated to the process).
    pub title_contains: String,
    /// The operation text the policy engine classified.
    pub command: String,
    /// The working directory of the request.
    pub working_dir: String,
}

/// The outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// All four agree.
    Match,
    /// Why it was rejected (non-secret).
    Reject(&'static str),
}

fn norm_text(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn norm_path(s: &str) -> String {
    let t = s.trim().replace('\\', "/");
    let t = t.trim_end_matches('/');
    t.to_ascii_lowercase()
}

/// Whether `window` and `card` are the surface for `expect`.
pub fn correlate(window: &WindowInfo, card: &Card, expect: &Expectation) -> Verdict {
    if expect.title_contains.trim().is_empty() || expect.command.trim().is_empty() {
        // An expectation with nothing to check is never a match.
        return Verdict::Reject("the request carries too little to correlate against");
    }
    if !window
        .title
        .to_ascii_lowercase()
        .contains(&expect.title_contains.to_ascii_lowercase())
    {
        return Verdict::Reject("the window title does not match");
    }
    if card.status.trim().to_ascii_lowercase() != WAITING {
        return Verdict::Reject("the card is not waiting for input");
    }
    if norm_text(&card.command) != norm_text(&expect.command) {
        return Verdict::Reject("the command text does not match");
    }
    match &card.working_dir {
        Some(dir) if norm_path(dir) == norm_path(&expect.working_dir) => Verdict::Match,
        Some(_) => Verdict::Reject("the working directory does not match"),
        None => Verdict::Reject("the card shows no working directory to confirm"),
    }
}
