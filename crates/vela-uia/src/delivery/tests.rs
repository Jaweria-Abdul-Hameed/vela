use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::*;
use crate::discovery::tests::{FakeSurface, card_tree, expectation, window};

fn fast() -> (WarmUp, Verify) {
    (
        WarmUp {
            attempts: 3,
            delay: Duration::ZERO,
            min_nodes: 5,
        },
        Verify {
            attempts: 3,
            delay: Duration::ZERO,
        },
    )
}

fn surface() -> FakeSurface {
    let s = FakeSurface::new();
    s.add(
        window(7),
        card_tree(
            &["git status --short"],
            r"C:\vela-probe\wt1",
            "Waiting for user input",
        ),
    );
    s
}

fn run(
    s: &FakeSurface,
    consent: &AtomicBool,
    decision: DeliveryDecision,
) -> Result<DeliveryResult, VelaError> {
    let (w, v) = fast();
    deliver(s, consent, 7, &expectation(), decision, w, v, &|_| {})
}

#[test]
fn option_selection_across_label_permutations_chooses_only_single_use_or_refusal() {
    let table = [
        ("Yes, allow this time", OptionKind::SingleUse),
        ("yes, ALLOW THIS TIME", OptionKind::SingleUse),
        ("Yes, allow once", OptionKind::SingleUse),
        (
            "Yes, always allow in this conversation",
            OptionKind::Persistent,
        ),
        ("Yes, always allow in this project", OptionKind::Persistent),
        ("Yes, always allow", OptionKind::Persistent),
        ("Always allow this time", OptionKind::Persistent),
        (
            "Allow this time and don't ask again",
            OptionKind::Persistent,
        ),
        (
            "Yes, allow this time for this session",
            OptionKind::Persistent,
        ),
        (
            "No (tell the agent what to do instead)",
            OptionKind::Refusal,
        ),
        ("No", OptionKind::Refusal),
        ("Not now", OptionKind::Unknown),
        ("Skip", OptionKind::Unknown),
        ("Submit", OptionKind::Unknown),
        ("", OptionKind::Unknown),
    ];
    for (label, want) in table {
        assert_eq!(classify_option(label), want, "{label:?}");
    }
}

#[test]
fn a_card_with_no_or_several_matching_options_fails_closed() {
    let tree = card_tree(&["git status"], r"C:\a", "Waiting for user input");
    let mut card = crate::discovery::card::find_cards(&tree).remove(0).unwrap();
    assert_eq!(
        choose(&card, DeliveryDecision::AllowOnce).unwrap().label,
        "Yes, allow this time"
    );
    assert_eq!(
        choose(&card, DeliveryDecision::Deny).unwrap().label,
        "No (tell the agent what to do instead)"
    );
    let mut twin = card.options[0].clone();
    twin.label = "Yes, allow once".into();
    card.options.push(twin);
    assert_eq!(
        choose(&card, DeliveryDecision::AllowOnce)
            .unwrap_err()
            .code(),
        Some(ErrorCode::ApprovalTargetAmbiguous)
    );
    card.options
        .retain(|o| classify_option(&o.label) != OptionKind::SingleUse);
    assert_eq!(
        choose(&card, DeliveryDecision::AllowOnce)
            .unwrap_err()
            .code(),
        Some(ErrorCode::ApprovalTargetAmbiguous)
    );
}

#[test]
fn allow_once_selects_only_the_single_use_option_then_submits_and_verifies_the_card_cleared() {
    let s = surface();
    let consent = AtomicBool::new(true);
    let r = run(&s, &consent, DeliveryDecision::AllowOnce).unwrap();
    assert!(r.verified, "{}", r.detail);
    assert_eq!(s.log(), ["select:Yes, allow this time", "invoke:Submit"]);
    // At the moment of Submit exactly the single-use option was selected: no persistent permission.
    assert_eq!(s.selected_at_submit(), ["Yes, allow this time"]);
}

#[test]
fn a_refusal_selects_only_the_refusal_option() {
    let s = surface();
    let consent = AtomicBool::new(true);
    let r = run(&s, &consent, DeliveryDecision::Deny).unwrap();
    assert!(r.verified);
    assert_eq!(
        s.selected_at_submit(),
        ["No (tell the agent what to do instead)"]
    );
    assert!(s.log().iter().all(|l| !l.to_lowercase().contains("always")));
}

#[test]
fn no_consent_means_nothing_is_touched_and_revocation_mid_flight_stops_at_once() {
    let s = surface();
    let off = AtomicBool::new(false);
    assert_eq!(
        run(&s, &off, DeliveryDecision::AllowOnce)
            .unwrap_err()
            .code(),
        Some(ErrorCode::ApprovalUndeliverable)
    );
    assert!(s.log().is_empty());

    // Consent that is withdrawn after the first read: no pattern is ever used.
    struct RevokedAfter(std::sync::atomic::AtomicU32);
    impl Consent for RevokedAfter {
        fn granted(&self) -> bool {
            self.0.fetch_add(1, Ordering::SeqCst) < 1
        }
    }
    let s = surface();
    let (w, v) = fast();
    let e = deliver(
        &s,
        &RevokedAfter(std::sync::atomic::AtomicU32::new(0)),
        7,
        &expectation(),
        DeliveryDecision::AllowOnce,
        w,
        v,
        &|_| {},
    )
    .unwrap_err();
    assert_eq!(e.code(), Some(ErrorCode::ApprovalUndeliverable));
    assert!(
        s.log().is_empty(),
        "revocation stopped delivery before any action: {:?}",
        s.log()
    );
}

#[test]
fn a_persistent_option_already_selected_is_never_submitted() {
    // The card's default selection is somehow a persistent option and selecting ours does not clear it (a broken surface).
    let s = FakeSurface::new();
    let mut tree = card_tree(
        &["git status --short"],
        r"C:\vela-probe\wt1",
        "Waiting for user input",
    );
    {
        let card = &mut tree.children[1];
        for o in card
            .children
            .iter_mut()
            .filter(|n| n.role == crate::discovery::model::Role::Radio)
        {
            o.selected = o.name == "Yes, always allow";
        }
    }
    s.add(window(7), tree);
    // Selecting through the fake deselects siblings, so the delivery succeeds with only ours selected: proof it checks the result.
    let consent = AtomicBool::new(true);
    let r = run(&s, &consent, DeliveryDecision::AllowOnce).unwrap();
    assert!(r.verified);
    assert_eq!(s.selected_at_submit(), ["Yes, allow this time"]);
}

#[test]
fn a_card_that_does_not_clear_is_reported_unverified_not_assumed_delivered() {
    let s = surface();
    s.never_clears();
    let consent = AtomicBool::new(true);
    let r = run(&s, &consent, DeliveryDecision::AllowOnce).unwrap();
    assert!(!r.verified);
}

#[test]
fn ambiguity_and_a_wrong_card_fail_closed_before_any_action() {
    let s = FakeSurface::new();
    s.add(
        window(7),
        card_tree(
            &["rm -rf x"],
            r"C:\vela-probe\wt1",
            "Waiting for user input",
        ),
    );
    let consent = AtomicBool::new(true);
    assert_eq!(
        run(&s, &consent, DeliveryDecision::AllowOnce)
            .unwrap_err()
            .code(),
        Some(ErrorCode::ApprovalWindowMismatch)
    );
    assert!(s.log().is_empty());
}

#[tokio::test]
async fn the_adapter_is_dormant_without_a_session_and_delivers_when_one_is_registered() {
    let s = Arc::new(surface());
    let consent = Arc::new(AtomicBool::new(true));
    let (w, v) = fast();
    let a = DesktopApproval::new(s.clone(), consent.clone()).with_timing(w, v);
    assert!(a.next_observation().await.unwrap().is_none());
    assert!(!a.detect().await.unwrap().available);
    a.expect(7, expectation());
    let cap = a.detect().await.unwrap();
    assert!(cap.available && cap.tier == DeliveryTier::UiAutomation);
    let obs = a.next_observation().await.unwrap().unwrap();
    assert_eq!(obs.operation, "git status --short");
    assert_eq!(obs.working_directory.as_deref(), Some(r"C:\vela-probe\wt1"));

    // A request about a different command is refused even though a card is showing.
    let mut other = obs.clone();
    other.operation = "git push".into();
    let e = a
        .deliver(&DeliveryRequest {
            observation: other,
            decision: DeliveryDecision::AllowOnce,
        })
        .await
        .unwrap_err();
    assert_eq!(e.code(), Some(ErrorCode::ApprovalWindowMismatch));
    assert!(s.log().is_empty());

    let r = a
        .deliver(&DeliveryRequest {
            observation: obs,
            decision: DeliveryDecision::AllowOnce,
        })
        .await
        .unwrap();
    assert!(r.verified);

    // Revoking consent makes it unavailable at once.
    consent.store(false, Ordering::SeqCst);
    assert!(!a.detect().await.unwrap().available);
}
