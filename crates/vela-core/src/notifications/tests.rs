use std::sync::Arc;

use vela_domain::errors::VelaError;
use vela_domain::events::{Event, HumanActionRequiredPayload, RunCompletedPayload};
use vela_domain::ports::{Notification, Notifier, PortFuture, ready};
use vela_testkit::fakes::FakeNotifier;

use super::*;
use crate::orchestrator::test_support::{MarkerRedactor, env};
use crate::settings::{SettingsService, Source};

fn human(summary: &str) -> Event {
    Event::HumanActionRequired(HumanActionRequiredPayload {
        intervention_kind: "REVIEW_STALLED".into(),
        summary: summary.into(),
    })
}

fn completed(outcome: &str) -> Event {
    Event::RunCompleted(RunCompletedPayload {
        outcome: outcome.into(),
        tickets_done: 3,
        tickets_total: 4,
    })
}

struct Denied;

impl Notifier for Denied {
    fn notify<'a>(&'a self, _n: &'a Notification) -> PortFuture<'a, ()> {
        ready(Err(VelaError::internal("notifications are blocked")))
    }
}

#[tokio::test]
async fn human_action_and_run_end_notify_and_other_events_do_not() {
    let e = env("notify");
    let fake = Arc::new(FakeNotifier::new());
    let svc = NotificationService::new(e.services.clone(), fake.clone(), None);
    e.append(Event::RunPaused(vela_domain::events::RunPausedPayload {
        reason: "x".into(),
    }));
    e.append(human("approve the merge"));
    e.append(completed("COMPLETED"));
    e.append(completed("FAILED"));
    let delivered = svc.deliver_new().await.unwrap();
    assert_eq!(delivered.len(), 3);
    let sent = fake.sent();
    assert_eq!(sent[0].title, "Vela needs you");
    assert_eq!(sent[1].title, "Run finished");
    assert_eq!(sent[1].body, "3 of 4 tickets done.");
    assert_eq!(sent[2].title, "Run ended: FAILED");
}

#[tokio::test]
async fn a_restart_does_not_repeat_old_notifications() {
    let e = env("once");
    let fake = Arc::new(FakeNotifier::new());
    e.append(human("one"));
    let first = NotificationService::new(e.services.clone(), fake.clone(), None);
    assert_eq!(first.deliver_new().await.unwrap().len(), 1);
    let again = NotificationService::new(e.restart(), fake.clone(), None);
    assert!(again.deliver_new().await.unwrap().is_empty());
    e.append(human("two"));
    assert_eq!(again.deliver_new().await.unwrap().len(), 1);
    assert_eq!(fake.sent().len(), 2);
}

#[tokio::test]
async fn a_refused_notification_goes_to_the_in_app_inbox() {
    let e = env("denied");
    let svc = NotificationService::new(e.services.clone(), Arc::new(Denied), None);
    e.append(human("look at this"));
    let delivered = svc.deliver_new().await.unwrap();
    assert_eq!(delivered[0].channel, Channel::InApp);
    assert_eq!(svc.inbox().len(), 1);
    assert_eq!(svc.inbox()[0].body, "look at this");
    svc.clear_inbox();
    assert!(svc.inbox().is_empty());
}

#[tokio::test]
async fn turning_notifications_off_skips_them_without_replaying_later() {
    let e = env("off");
    let fake = Arc::new(FakeNotifier::new());
    let svc = NotificationService::new(e.services.clone(), fake.clone(), None);
    SettingsService::new(&e.services)
        .set_user(Source::User, "notifications", "false")
        .unwrap();
    e.append(human("quiet"));
    assert!(svc.deliver_new().await.unwrap().is_empty());
    SettingsService::new(&e.services)
        .set_user(Source::User, "notifications", "true")
        .unwrap();
    assert!(svc.deliver_new().await.unwrap().is_empty());
    assert!(fake.sent().is_empty());
}

#[tokio::test]
async fn secrets_are_redacted_from_notification_text() {
    let e = env("redact");
    let fake = Arc::new(FakeNotifier::new());
    let secret = ["tok", "en-abc123"].concat();
    let red: SharedRedaction = Arc::new(MarkerRedactor(secret.clone()));
    let svc = NotificationService::new(e.services.clone(), fake.clone(), Some(red));
    e.append(human(&format!("fix the login with {secret}")));
    svc.deliver_new().await.unwrap();
    assert!(!fake.sent()[0].body.contains(&secret));
    assert!(fake.sent()[0].body.contains("[REDACTED]"));
}
