//! Assembling the approval card from the tree (K05). The verified Desktop card is a DOM card inside the window: a title ("Allow
//! checking git status?"), the working directory, the command split into text fragments, the status text "Waiting for user
//! input", five radio options and `Skip` and `Submit` buttons. A card is anchored on its `Submit` button; a button called
//! "Approve" that is not part of such a card is never a card.

use super::model::{NodePath, Role, UiNode};

/// The status text a live card shows.
pub const WAITING: &str = "waiting for user input";

/// The submit button's label.
pub const SUBMIT: &str = "submit";

/// One radio option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardOption {
    /// The label.
    pub label: String,
    /// Where it is.
    pub path: NodePath,
    /// Whether it was selected when read.
    pub selected: bool,
}

/// A parsed card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    /// The title.
    pub title: String,
    /// The working directory shown, when the card shows one.
    pub working_dir: Option<String>,
    /// The command assembled from its fragments.
    pub command: String,
    /// The status text.
    pub status: String,
    /// The radio options, in order.
    pub options: Vec<CardOption>,
    /// The `Submit` button.
    pub submit: NodePath,
    /// Where the card's own subtree starts.
    pub root: NodePath,
}

/// Why a candidate is not a usable card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardError {
    /// There is no title.
    NoTitle,
    /// It does not say it is waiting for input (stale or already answered).
    NotWaiting,
    /// It offers no options.
    NoOptions,
}

fn looks_like_path(s: &str) -> bool {
    let b = s.as_bytes();
    (b.len() > 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/'))
        || s.starts_with("\\\\")
        || s.starts_with('/')
}

fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Every card in the tree, each parsed or explained. The root of each is the smallest subtree that holds one `Submit` button and
/// radio options and no other `Submit` button, so two cards in one window are two results, never merged.
pub fn find_cards(tree: &UiNode) -> Vec<Result<Card, CardError>> {
    let all = tree.walk();
    let submits: Vec<&NodePath> = all
        .iter()
        .filter(|(_, n)| n.role == Role::Button && n.name.trim().eq_ignore_ascii_case(SUBMIT))
        .map(|(p, _)| p)
        .collect();
    let submit_count = |n: &UiNode| {
        n.walk()
            .iter()
            .filter(|(_, c)| c.role == Role::Button && c.name.trim().eq_ignore_ascii_case(SUBMIT))
            .count()
    };
    let radios = |n: &UiNode| {
        n.walk()
            .iter()
            .filter(|(_, c)| c.role == Role::Radio)
            .count()
    };
    let mut out = Vec::new();
    for submit in submits {
        // Climb until the subtree has radio options; stop before it would take in another card's Submit.
        let mut root = submit.clone();
        let mut chosen: Option<NodePath> = None;
        loop {
            let node = tree.at(&root).expect("path from walk");
            if submit_count(node) > 1 {
                break;
            }
            if radios(node) > 0 {
                chosen = Some(root.clone());
            }
            if root.is_empty() {
                break;
            }
            root.pop();
        }
        let Some(root) = chosen else {
            out.push(Err(CardError::NoOptions));
            continue;
        };
        out.push(parse(tree, &root, submit));
    }
    out
}

fn parse(tree: &UiNode, root: &NodePath, submit: &NodePath) -> Result<Card, CardError> {
    let node = tree.at(root).expect("root path");
    let mut texts: Vec<String> = Vec::new();
    let mut options = Vec::new();
    for (rel, n) in node.walk() {
        let mut path = root.clone();
        path.extend(rel);
        match n.role {
            Role::Text if !n.name.trim().is_empty() => texts.push(n.name.clone()),
            Role::Radio => options.push(CardOption {
                label: n.name.trim().to_owned(),
                path,
                selected: n.selected,
            }),
            _ => {}
        }
    }
    if options.is_empty() {
        return Err(CardError::NoOptions);
    }
    let status = texts
        .iter()
        .find(|t| t.trim().eq_ignore_ascii_case(WAITING))
        .cloned()
        .ok_or(CardError::NotWaiting)?;
    let title_at = texts
        .iter()
        .position(|t| t.trim_end().ends_with('?'))
        .ok_or(CardError::NoTitle)?;
    let title = texts[title_at].trim().to_owned();
    let working_dir = texts
        .iter()
        .find(|t| looks_like_path(t.trim()))
        .map(|t| t.trim().to_owned());
    let labels: Vec<&str> = options.iter().map(|o| o.label.as_str()).collect();
    let fragments: String = texts
        .iter()
        .skip(title_at + 1)
        .filter(|t| {
            let tt = t.trim();
            !tt.eq_ignore_ascii_case(WAITING)
                && !looks_like_path(tt)
                && !labels.contains(&tt)
                && !tt.eq_ignore_ascii_case("skip")
                && !tt.eq_ignore_ascii_case(SUBMIT)
        })
        .map(String::as_str)
        .collect::<Vec<_>>()
        .concat();
    Ok(Card {
        title,
        working_dir,
        command: collapse(&fragments),
        status: status.trim().to_owned(),
        options,
        submit: submit.clone(),
        root: root.clone(),
    })
}
