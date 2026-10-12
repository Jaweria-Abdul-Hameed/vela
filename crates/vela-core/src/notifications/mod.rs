//! Notifications (S17): the user hears when human action is required, a run finishes, or something ends badly.
//!
//! The service reads the committed journal, so a notification can never describe something that did not happen, and it
//! remembers the last journal position it handled, so a restart does not repeat old notifications. When the operating
//! system refuses a notification, the same text goes to an in-app inbox the UI shows.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use vela_domain::errors::VelaError;
use vela_domain::events::{Event, EventEnvelope};
use vela_domain::ports::{Notification, Notifier};
use vela_persistence::journal;
use vela_persistence::repositories::f06_core;

use crate::diagnostics::SharedRedaction;
use crate::orchestrator::Services;
use crate::settings::SettingsService;

const LAST_SEQ_KEY: &str = "notifications.last_seq";
const INBOX_CAPACITY: usize = 100;

/// How a notification reached the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    /// A desktop notification.
    Desktop,
    /// The operating system refused it; it is in the in-app inbox.
    InApp,
}

/// One notification that was handled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivered {
    /// The journal position it came from.
    pub seq: u64,
    /// What the user saw.
    pub title: String,
    /// How it reached them.
    pub channel: Channel,
}

/// The notification a journal event calls for, if any.
pub fn notification_for(envelope: &EventEnvelope) -> Option<Notification> {
    let ticket = envelope.ticket_id.clone();
    match &envelope.event {
        Event::HumanActionRequired(p) => Some(Notification {
            title: "Vela needs you".to_owned(),
            body: p.summary.clone(),
            ticket,
        }),
        Event::RunCompleted(p) => {
            let good = {
                let o = p.outcome.to_ascii_lowercase();
                o.contains("complete") || o.contains("success") || o.contains("done")
            };
            Some(Notification {
                title: if good {
                    "Run finished".to_owned()
                } else {
                    format!("Run ended: {}", p.outcome)
                },
                body: format!("{} of {} tickets done.", p.tickets_done, p.tickets_total),
                ticket,
            })
        }
        _ => None,
    }
}

/// Delivers notifications for new journal events.
pub struct NotificationService {
    services: Arc<Services>,
    notifier: Arc<dyn Notifier>,
    redactor: Option<SharedRedaction>,
    inbox: Mutex<VecDeque<Notification>>,
}

impl NotificationService {
    /// A service that sends through `notifier`, redacting text with `redactor` when given.
    pub fn new(
        services: Arc<Services>,
        notifier: Arc<dyn Notifier>,
        redactor: Option<SharedRedaction>,
    ) -> Self {
        Self {
            services,
            notifier,
            redactor,
            inbox: Mutex::new(VecDeque::new()),
        }
    }

    /// The in-app inbox, oldest first.
    pub fn inbox(&self) -> Vec<Notification> {
        self.inbox
            .lock()
            .expect("inbox lock")
            .iter()
            .cloned()
            .collect()
    }

    /// Dismisses everything in the inbox.
    pub fn clear_inbox(&self) {
        self.inbox.lock().expect("inbox lock").clear();
    }

    fn clean(&self, text: &str) -> String {
        match &self.redactor {
            Some(r) => r.redact_text(text),
            None => text.to_owned(),
        }
    }

    /// Handles journal events newer than the last one handled. With notifications turned off the position still advances,
    /// so turning them on later does not replay the past.
    pub async fn deliver_new(&self) -> Result<Vec<Delivered>, VelaError> {
        let last: u64 = self
            .services
            .store
            .read(|c| f06_core::get_setting(c, LAST_SEQ_KEY))?
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let events = self
            .services
            .store
            .read(|c| journal::read_after(c, last, 500))?;
        let enabled =
            SettingsService::new(&self.services).effective(None)?["notifications"] == "true";
        let mut delivered = Vec::new();
        let mut newest = last;
        for envelope in &events {
            newest = envelope.seq;
            if !enabled {
                continue;
            }
            let Some(mut n) = notification_for(envelope) else {
                continue;
            };
            n.title = self.clean(&n.title);
            n.body = self.clean(&n.body);
            let channel = match self.notifier.notify(&n).await {
                Ok(()) => Channel::Desktop,
                Err(_) => {
                    let mut inbox = self.inbox.lock().expect("inbox lock");
                    if inbox.len() == INBOX_CAPACITY {
                        inbox.pop_front();
                    }
                    inbox.push_back(n.clone());
                    Channel::InApp
                }
            };
            delivered.push(Delivered {
                seq: envelope.seq,
                title: n.title,
                channel,
            });
        }
        if newest != last {
            let (value, now) = (newest.to_string(), self.services.now_ms());
            self.services
                .store
                .write(move |tx| f06_core::set_setting(tx.conn(), LAST_SEQ_KEY, &value, now))?;
        }
        Ok(delivered)
    }
}

#[cfg(test)]
mod tests;
