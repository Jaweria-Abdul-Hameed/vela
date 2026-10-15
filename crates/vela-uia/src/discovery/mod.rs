//! Discovery and observation of Desktop approval cards (K05). Everything here works on an abstract accessibility tree
//! ([`model::Surface`]), so the decisions are tested against in-memory trees and against a real Win32 fake window alike; the
//! real UI Automation binding is [`win`].
//!
//! Vela only ever *reads* here. It finds windows of the Vela-correlated process, waits for the lazily populated Chromium tree,
//! assembles the card from its fragments, and correlates it with the request before anything may act on it. Anything
//! ambiguous fails closed with a typed error: a minimized window, two cards, a card that does not match, an unrelated
//! "Approve" button.

pub mod card;
pub mod correlate;
pub mod model;
pub mod win;

use std::time::Duration;

use model::{NodePath, Surface, UiNode, WindowInfo};
use vela_domain::errors::{ErrorCode, VelaError};

/// How to wait for a tree that fills in after a UI Automation client first asks.
#[derive(Debug, Clone, Copy)]
pub struct WarmUp {
    /// The most reads before giving up.
    pub attempts: u32,
    /// Pause between reads.
    pub delay: Duration,
    /// The tree is considered populated once it has at least this many nodes (and a card, when one is expected).
    pub min_nodes: usize,
}

impl Default for WarmUp {
    fn default() -> Self {
        Self {
            attempts: 20,
            delay: Duration::from_millis(500),
            min_nodes: 20,
        }
    }
}

/// Reads the window's tree, retrying while it is nearly empty or has no card yet. `sleep` is injected so tests do not wait.
/// Returns the last tree read, which may still contain no card: the caller decides what that means.
pub fn warm_up(
    surface: &dyn Surface,
    window: &WindowInfo,
    policy: WarmUp,
    sleep: &dyn Fn(Duration),
) -> Result<UiNode, VelaError> {
    let mut last = surface.tree(window)?;
    for _ in 1..policy.attempts.max(1) {
        if last.count() >= policy.min_nodes && card::find_cards(&last).iter().any(|c| c.is_ok()) {
            break;
        }
        sleep(policy.delay);
        last = surface.tree(window)?;
    }
    Ok(last)
}

/// A card found in a window, with where it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observed {
    /// The window.
    pub window: WindowInfo,
    /// The parsed card.
    pub card: card::Card,
}

/// Finds the one correlated card among the windows of `pid`. Fails closed:
/// * a window that is minimized or not visible is `ApprovalSurfaceUnavailable` (not bypassed);
/// * no card at all is `ApprovalSurfaceNotFound`;
/// * two or more matching cards are `ApprovalTargetAmbiguous`;
/// * a card that does not correlate with `expect` is never returned (`ApprovalWindowMismatch`).
pub fn observe(
    surface: &dyn Surface,
    pid: u32,
    expect: &correlate::Expectation,
    policy: WarmUp,
    sleep: &dyn Fn(Duration),
) -> Result<Observed, VelaError> {
    let windows: Vec<WindowInfo> = surface
        .windows()?
        .into_iter()
        .filter(|w| w.pid == pid)
        .collect();
    if windows.is_empty() {
        return Err(VelaError::from_code(
            ErrorCode::ApprovalSurfaceNotFound,
            "no window of the correlated Antigravity process was found",
        ));
    }
    let mut matches: Vec<Observed> = Vec::new();
    let mut mismatched = false;
    let mut unavailable = false;
    for w in windows {
        if w.minimized || !w.visible {
            unavailable = true;
            continue;
        }
        let tree = warm_up(surface, &w, policy, sleep)?;
        for found in card::find_cards(&tree) {
            let Ok(card) = found else { continue };
            match correlate::correlate(&w, &card, expect) {
                correlate::Verdict::Match => matches.push(Observed {
                    window: w.clone(),
                    card,
                }),
                correlate::Verdict::Reject(_) => mismatched = true,
            }
        }
    }
    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 if unavailable => Err(VelaError::from_code(
            ErrorCode::ApprovalSurfaceUnavailable,
            "the Antigravity window is minimized or not visible; Vela does not restore or move windows",
        )),
        0 if mismatched => Err(VelaError::from_code(
            ErrorCode::ApprovalWindowMismatch,
            "an approval card is showing but it does not match the request Vela is answering",
        )),
        0 => Err(VelaError::from_code(
            ErrorCode::ApprovalSurfaceNotFound,
            "no approval card was found in the correlated window",
        )),
        _ => Err(VelaError::from_code(
            ErrorCode::ApprovalTargetAmbiguous,
            "more than one approval card matches; Vela will not guess which to answer",
        )),
    }
}

/// Re-resolves a recorded path against a fresh tree, confirming the node is still the same one (role and name), so a tree that
/// changed between reading and acting is never acted on blindly.
pub fn resolve<'t>(
    tree: &'t UiNode,
    path: &NodePath,
    role: model::Role,
    name: &str,
) -> Result<&'t UiNode, VelaError> {
    let node = tree.at(path).ok_or_else(|| {
        VelaError::from_code(
            ErrorCode::ApprovalSurfaceNotFound,
            "the control is no longer where it was read",
        )
    })?;
    if node.role != role || node.name != name {
        return Err(VelaError::from_code(
            ErrorCode::ApprovalTargetAmbiguous,
            "the control at that position changed since it was read",
        ));
    }
    Ok(node)
}

#[cfg(test)]
pub(crate) mod tests;
