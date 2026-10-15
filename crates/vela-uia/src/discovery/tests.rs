use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use vela_domain::errors::ErrorCode;

use super::card::{CardError, find_cards};
use super::correlate::{Expectation, Verdict, correlate};
use super::model::*;
use super::*;

pub(crate) const OPTIONS: [&str; 5] = [
    "Yes, allow this time",
    "Yes, always allow in this conversation",
    "Yes, always allow in this project",
    "Yes, always allow",
    "No (tell the agent what to do instead)",
];

/// A tree shaped like the verified Desktop 2.17.0 card.
pub(crate) fn card_tree(fragments: &[&str], dir: &str, status: &str) -> UiNode {
    let mut kids = vec![
        UiNode::new(Role::Text, "Allow checking git status?"),
        UiNode::new(Role::Text, dir),
    ];
    kids.extend(fragments.iter().map(|f| UiNode::new(Role::Text, *f)));
    kids.push(UiNode::new(Role::Text, status));
    kids.push(UiNode::new(Role::Edit, "Edit permission target"));
    for (i, o) in OPTIONS.iter().enumerate() {
        kids.push(UiNode::radio(*o, i == 0));
    }
    kids.push(UiNode::new(Role::Edit, "Write-in"));
    kids.push(UiNode::button("Skip"));
    kids.push(UiNode::button("Submit"));
    UiNode::new(Role::Window, "Antigravity").with(vec![
        UiNode::button("Approve"),
        UiNode::new(Role::Other, "card").with(kids),
    ])
}

pub(crate) fn expectation() -> Expectation {
    Expectation {
        title_contains: "vela-probe".into(),
        command: "git status --short".into(),
        working_dir: r"C:\vela-probe\wt1".into(),
    }
}

pub(crate) fn window(pid: u32) -> WindowInfo {
    WindowInfo {
        handle: 1,
        pid,
        title: "wt1 - vela-probe - Antigravity".into(),
        visible: true,
        minimized: false,
    }
}

#[derive(Default)]
struct State {
    windows: Vec<WindowInfo>,
    trees: HashMap<u64, UiNode>,
    reads: HashMap<u64, usize>,
    /// Reads that return a nearly empty tree first (lazy Chromium population).
    sparse_reads: usize,
    log: Vec<String>,
    /// When true, invoking Submit removes the card.
    clears_on_submit: bool,
    /// What was selected at the moment Submit was invoked.
    selected_at_submit: Vec<String>,
}

/// An in-memory surface that behaves like the verified card: selecting a radio deselects its siblings, Submit clears the card.
pub(crate) struct FakeSurface(Mutex<State>);

impl FakeSurface {
    pub(crate) fn new() -> Self {
        Self(Mutex::new(State {
            clears_on_submit: true,
            ..State::default()
        }))
    }

    pub(crate) fn add(&self, w: WindowInfo, tree: UiNode) {
        let mut s = self.0.lock().unwrap();
        s.trees.insert(w.handle, tree);
        s.windows.push(w);
    }

    pub(crate) fn sparse_for(&self, reads: usize) {
        self.0.lock().unwrap().sparse_reads = reads;
    }

    pub(crate) fn never_clears(&self) {
        self.0.lock().unwrap().clears_on_submit = false;
    }

    pub(crate) fn log(&self) -> Vec<String> {
        self.0.lock().unwrap().log.clone()
    }

    pub(crate) fn selected_at_submit(&self) -> Vec<String> {
        self.0.lock().unwrap().selected_at_submit.clone()
    }
}

fn node_mut<'a>(n: &'a mut UiNode, path: &[usize]) -> Option<&'a mut UiNode> {
    let mut cur = n;
    for i in path {
        cur = cur.children.get_mut(*i)?;
    }
    Some(cur)
}

impl Surface for FakeSurface {
    fn windows(&self) -> Result<Vec<WindowInfo>, VelaError> {
        Ok(self.0.lock().unwrap().windows.clone())
    }

    fn tree(&self, w: &WindowInfo) -> Result<UiNode, VelaError> {
        let mut s = self.0.lock().unwrap();
        let reads = s.reads.entry(w.handle).or_insert(0);
        *reads += 1;
        let seen = *reads;
        if seen <= s.sparse_reads {
            return Ok(UiNode::new(Role::Window, "Antigravity"));
        }
        Ok(s.trees.get(&w.handle).cloned().expect("window tree"))
    }

    fn select(&self, w: &WindowInfo, path: &NodePath) -> Result<(), VelaError> {
        let mut s = self.0.lock().unwrap();
        let tree = s.trees.get_mut(&w.handle).expect("tree");
        let parent_path = &path[..path.len() - 1];
        let parent = node_mut(tree, parent_path).expect("parent");
        for (i, c) in parent.children.iter_mut().enumerate() {
            if c.role == Role::Radio {
                c.selected = i == path[path.len() - 1];
            }
        }
        let name = node_mut(tree, path).expect("node").name.clone();
        s.log.push(format!("select:{name}"));
        Ok(())
    }

    fn invoke(&self, w: &WindowInfo, path: &NodePath) -> Result<(), VelaError> {
        let mut s = self.0.lock().unwrap();
        let tree = s.trees.get_mut(&w.handle).expect("tree");
        let name = node_mut(tree, path).expect("node").name.clone();
        s.log.push(format!("invoke:{name}"));
        if name == "Submit" {
            let chosen: Vec<String> = s.trees[&w.handle]
                .walk()
                .into_iter()
                .filter(|(_, n)| n.role == Role::Radio && n.selected)
                .map(|(_, n)| n.name.clone())
                .collect();
            s.selected_at_submit = chosen;
            if s.clears_on_submit {
                let t = s.trees.get_mut(&w.handle).expect("tree");
                t.children.retain(|c| c.name != "card");
            }
        }
        Ok(())
    }
}

fn no_sleep() -> impl Fn(Duration) {
    |_| {}
}

fn fast() -> WarmUp {
    WarmUp {
        attempts: 5,
        delay: Duration::ZERO,
        min_nodes: 5,
    }
}

#[test]
fn the_card_is_assembled_from_its_fragments() {
    let tree = card_tree(
        &["git", " status ", "--short"],
        r"C:\vela-probe\wt1",
        "Waiting for user input",
    );
    let cards = find_cards(&tree);
    assert_eq!(cards.len(), 1);
    let c = cards[0].as_ref().unwrap();
    assert_eq!(c.title, "Allow checking git status?");
    assert_eq!(c.command, "git status --short");
    assert_eq!(c.working_dir.as_deref(), Some(r"C:\vela-probe\wt1"));
    assert_eq!(c.status, "Waiting for user input");
    assert_eq!(c.options.len(), 5);
    assert!(c.options[0].selected && !c.options[1].selected);
    // The unrelated Approve button is never part of a card.
    assert!(tree.walk().iter().any(|(_, n)| n.name == "Approve"));
    assert!(c.options.iter().all(|o| o.label != "Approve"));
}

#[test]
fn a_card_that_is_not_waiting_or_has_no_options_is_not_a_live_card() {
    let answered = card_tree(&["git status"], r"C:\a", "Done");
    assert_eq!(find_cards(&answered)[0], Err(CardError::NotWaiting));
    let lonely = UiNode::new(Role::Window, "w")
        .with(vec![UiNode::button("Submit"), UiNode::button("Approve")]);
    assert_eq!(find_cards(&lonely)[0], Err(CardError::NoOptions));
    assert!(
        find_cards(&UiNode::new(Role::Window, "w").with(vec![UiNode::button("Approve")]))
            .is_empty()
    );
}

#[test]
fn two_cards_in_one_window_are_two_cards() {
    let one = card_tree(&["git status"], r"C:\a", "Waiting for user input");
    let two = card_tree(&["git log"], r"C:\b", "Waiting for user input");
    let both =
        UiNode::new(Role::Window, "w").with(vec![one.children[1].clone(), two.children[1].clone()]);
    let cards = find_cards(&both);
    assert_eq!(cards.len(), 2);
    assert_eq!(cards[0].as_ref().unwrap().command, "git status");
    assert_eq!(cards[1].as_ref().unwrap().command, "git log");
}

fn card(command: &str, dir: &str, status: &str) -> super::card::Card {
    find_cards(&card_tree(&[command], dir, status))
        .remove(0)
        .unwrap()
}

#[test]
fn correlation_requires_title_command_directory_and_status_all_to_agree() {
    let w = window(7);
    let e = expectation();
    let good = card(
        "git status --short",
        r"C:\vela-probe\wt1",
        "Waiting for user input",
    );
    assert_eq!(correlate(&w, &good, &e), Verdict::Match);
    // Path spelling differences that mean the same place are fine.
    let slash = card(
        "git status   --short",
        "c:/vela-probe/wt1/",
        "Waiting for user input",
    );
    assert_eq!(correlate(&w, &slash, &e), Verdict::Match);

    let other_title = WindowInfo {
        title: "other - Antigravity".into(),
        ..w.clone()
    };
    assert!(matches!(
        correlate(&other_title, &good, &e),
        Verdict::Reject(_)
    ));
    let other_cmd = card("git status", r"C:\vela-probe\wt1", "Waiting for user input");
    assert!(matches!(correlate(&w, &other_cmd, &e), Verdict::Reject(_)));
    let prefix = card(
        "git status --short --branch",
        r"C:\vela-probe\wt1",
        "Waiting for user input",
    );
    assert!(
        matches!(correlate(&w, &prefix, &e), Verdict::Reject(_)),
        "a prefix is not a match"
    );
    let other_dir = card(
        "git status --short",
        r"C:\vela-probe\wt2",
        "Waiting for user input",
    );
    assert!(matches!(correlate(&w, &other_dir, &e), Verdict::Reject(_)));
    let mut no_dir = good.clone();
    no_dir.working_dir = None;
    assert!(matches!(correlate(&w, &no_dir, &e), Verdict::Reject(_)));
    let mut stale = good.clone();
    stale.status = "Done".into();
    assert!(matches!(correlate(&w, &stale, &e), Verdict::Reject(_)));
    let empty = Expectation {
        title_contains: String::new(),
        ..e
    };
    assert!(matches!(correlate(&w, &good, &empty), Verdict::Reject(_)));
}

fn code(r: Result<Observed, VelaError>) -> Option<ErrorCode> {
    r.err().and_then(|e| e.code())
}

#[test]
fn the_card_is_found_after_warming_up_a_sparse_tree() {
    let s = FakeSurface::new();
    s.add(
        window(7),
        card_tree(
            &["git status --short"],
            r"C:\vela-probe\wt1",
            "Waiting for user input",
        ),
    );
    s.sparse_for(3);
    let o = observe(&s, 7, &expectation(), fast(), &no_sleep()).unwrap();
    assert_eq!(o.card.command, "git status --short");
}

#[test]
fn observation_fails_closed_on_every_ambiguity() {
    let waiting = "Waiting for user input";
    // No window of that process.
    let s = FakeSurface::new();
    s.add(
        window(7),
        card_tree(&["git status --short"], r"C:\vela-probe\wt1", waiting),
    );
    assert_eq!(
        code(observe(&s, 99, &expectation(), fast(), &no_sleep())),
        Some(ErrorCode::ApprovalSurfaceNotFound)
    );

    // Minimized: not restored, reported unavailable.
    let s = FakeSurface::new();
    s.add(
        WindowInfo {
            minimized: true,
            visible: false,
            ..window(7)
        },
        card_tree(&["git status --short"], r"C:\vela-probe\wt1", waiting),
    );
    assert_eq!(
        code(observe(&s, 7, &expectation(), fast(), &no_sleep())),
        Some(ErrorCode::ApprovalSurfaceUnavailable)
    );

    // A card for something else.
    let s = FakeSurface::new();
    s.add(
        window(7),
        card_tree(&["rm -rf x"], r"C:\vela-probe\wt1", waiting),
    );
    assert_eq!(
        code(observe(&s, 7, &expectation(), fast(), &no_sleep())),
        Some(ErrorCode::ApprovalWindowMismatch)
    );

    // Only an unrelated Approve button.
    let s = FakeSurface::new();
    s.add(
        window(7),
        UiNode::new(Role::Window, "w").with(vec![UiNode::button("Approve")]),
    );
    assert_eq!(
        code(observe(&s, 7, &expectation(), fast(), &no_sleep())),
        Some(ErrorCode::ApprovalSurfaceNotFound)
    );

    // Two identical correlated cards.
    let s = FakeSurface::new();
    let one = card_tree(&["git status --short"], r"C:\vela-probe\wt1", waiting);
    let both =
        UiNode::new(Role::Window, "w").with(vec![one.children[1].clone(), one.children[1].clone()]);
    s.add(window(7), both);
    assert_eq!(
        code(observe(&s, 7, &expectation(), fast(), &no_sleep())),
        Some(ErrorCode::ApprovalTargetAmbiguous)
    );
}

#[test]
fn a_tree_that_changed_is_never_acted_on() {
    let tree = card_tree(&["git status"], r"C:\a", "Waiting for user input");
    let c = find_cards(&tree).remove(0).unwrap();
    let path = c.options[0].path.clone();
    assert!(resolve(&tree, &path, Role::Radio, "Yes, allow this time").is_ok());
    assert!(resolve(&tree, &path, Role::Radio, "Yes, always allow").is_err());
    assert!(resolve(&tree, &path, Role::Button, "Yes, allow this time").is_err());
    assert!(resolve(&tree, &vec![9, 9], Role::Radio, "x").is_err());
}
