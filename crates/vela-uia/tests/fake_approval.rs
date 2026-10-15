//! The real UI Automation binding against the fake approval window (K04, K05, K06): discovery, correlation, fail-closed ambiguity,
//! delivery of a single-use allow or a refusal, and proof that no persistent option was ever selected.
#![cfg(windows)]

mod fake_window;

use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use fake_window::{CardSpec, FakeWindow, OPTIONS, Scenario};
use vela_domain::errors::ErrorCode;
use vela_domain::ports::DeliveryDecision;
use vela_uia::delivery::{Verify, deliver};
use vela_uia::discovery::card::find_cards;
use vela_uia::discovery::correlate::Expectation;
use vela_uia::discovery::model::{Role, Surface};
use vela_uia::discovery::win::WinSurface;
use vela_uia::discovery::{WarmUp, observe};

/// One fake window at a time: they share a process and a window class.
static SERIAL: Mutex<()> = Mutex::new(());

fn expectation() -> Expectation {
    Expectation {
        title_contains: "vela-probe".into(),
        command: "git status --short".into(),
        working_dir: r"C:\vela-probe\wt1".into(),
    }
}

fn warm() -> WarmUp {
    WarmUp {
        attempts: 30,
        delay: Duration::from_millis(200),
        min_nodes: 10,
    }
}

fn sleep(d: Duration) {
    std::thread::sleep(d);
}

#[test]
fn the_harness_exposes_every_documented_role_and_label_to_ui_automation() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let w = FakeWindow::start(Scenario::one_card());
    let surface = WinSurface;
    let win = surface
        .windows()
        .unwrap()
        .into_iter()
        .find(|x| x.pid == w.pid && x.title.contains("vela-probe"))
        .expect("the fake window is listed");
    assert!(win.visible && !win.minimized);
    let tree = vela_uia::discovery::warm_up(&surface, &win, warm(), &sleep).unwrap();
    let names: Vec<(Role, String)> = tree
        .walk()
        .into_iter()
        .map(|(_, n)| (n.role, n.name.clone()))
        .collect();
    for o in OPTIONS {
        assert!(
            names.contains(&(Role::Radio, o.to_owned())),
            "missing option {o}: {names:?}"
        );
    }
    assert!(names.contains(&(Role::Button, "Submit".into())));
    assert!(names.contains(&(Role::Button, "Skip".into())));
    assert!(names.contains(&(Role::Button, "Approve".into())));
    assert!(names.contains(&(Role::Text, "Waiting for user input".into())));
    let cards = find_cards(&tree);
    assert_eq!(cards.len(), 1);
    let card = cards[0].as_ref().unwrap();
    assert_eq!(card.command, "git status --short");
    assert_eq!(card.working_dir.as_deref(), Some(r"C:\vela-probe\wt1"));
    assert!(card.options[0].selected, "option 1 is selected by default");
    let radios: Vec<_> = tree
        .walk()
        .into_iter()
        .filter(|(_, n)| n.role == Role::Radio)
        .collect();
    assert!(radios.iter().all(|(_, n)| n.can_select));
}

#[test]
fn a_single_use_allow_is_delivered_and_no_persistent_option_is_ever_selected() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let w = FakeWindow::start(Scenario::one_card());
    let consent = AtomicBool::new(true);
    let r = deliver(
        &WinSurface,
        &consent,
        w.pid,
        &expectation(),
        DeliveryDecision::AllowOnce,
        warm(),
        Verify {
            attempts: 20,
            delay: Duration::from_millis(250),
        },
        &sleep,
    )
    .expect("delivered");
    assert!(r.verified, "{}", r.detail);
    let submits = w.submits();
    assert_eq!(submits, vec![vec!["Yes, allow this time".to_owned()]]);
    assert!(
        submits
            .iter()
            .flatten()
            .all(|l| !l.to_lowercase().contains("always")),
        "a persistent option was selected at Submit"
    );
    assert_eq!(w.cards_left(), 0);
}

#[test]
fn a_refusal_is_delivered_through_the_refusal_option_only() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let w = FakeWindow::start(Scenario::one_card());
    let consent = AtomicBool::new(true);
    let r = deliver(
        &WinSurface,
        &consent,
        w.pid,
        &expectation(),
        DeliveryDecision::Deny,
        warm(),
        Verify {
            attempts: 20,
            delay: Duration::from_millis(250),
        },
        &sleep,
    )
    .expect("delivered");
    assert!(r.verified, "{}", r.detail);
    assert_eq!(
        w.submits(),
        vec![vec!["No (tell the agent what to do instead)".to_owned()]]
    );
}

#[test]
fn a_tree_that_fills_in_late_is_waited_for() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = Scenario::one_card();
    s.delay_ms = 1500;
    let w = FakeWindow::start(s);
    let o =
        observe(&WinSurface, w.pid, &expectation(), warm(), &sleep).expect("found after warm-up");
    assert_eq!(o.card.command, "git status --short");
}

#[test]
fn a_minimized_window_is_reported_unavailable_and_not_restored() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = Scenario::one_card();
    s.minimized = true;
    let w = FakeWindow::start(s);
    let e = observe(&WinSurface, w.pid, &expectation(), warm(), &sleep).unwrap_err();
    assert_eq!(e.code(), Some(ErrorCode::ApprovalSurfaceUnavailable));
    assert_eq!(w.cards_left(), 1);
    assert!(w.submits().is_empty());
}

#[test]
fn duplicate_cards_fail_closed_and_nothing_is_submitted() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = Scenario::one_card();
    s.cards = vec![CardSpec::git_status(), CardSpec::git_status()];
    let w = FakeWindow::start(s);
    let consent = AtomicBool::new(true);
    let e = deliver(
        &WinSurface,
        &consent,
        w.pid,
        &expectation(),
        DeliveryDecision::AllowOnce,
        warm(),
        Verify::default(),
        &sleep,
    )
    .unwrap_err();
    assert_eq!(e.code(), Some(ErrorCode::ApprovalTargetAmbiguous));
    assert!(w.submits().is_empty());
}

#[test]
fn a_non_correlated_card_and_an_unrelated_approve_button_are_never_acted_on() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = Scenario::one_card();
    s.cards = vec![CardSpec {
        fragments: vec!["rm -rf build".into()],
        dir: r"C:\vela-probe\wt1".into(),
    }];
    let w = FakeWindow::start(s);
    let e = observe(&WinSurface, w.pid, &expectation(), warm(), &sleep).unwrap_err();
    assert_eq!(e.code(), Some(ErrorCode::ApprovalWindowMismatch));

    let mut only_approve = Scenario::one_card();
    only_approve.cards = vec![];
    drop(w);
    let w2 = FakeWindow::start(only_approve);
    let e = observe(&WinSurface, w2.pid, &expectation(), warm(), &sleep).unwrap_err();
    assert_eq!(e.code(), Some(ErrorCode::ApprovalSurfaceNotFound));
    assert!(w2.submits().is_empty());
}
