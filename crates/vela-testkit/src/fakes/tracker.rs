//! An in-memory `IssueTrackerAdapter`.

use std::collections::BTreeMap;
use std::sync::Mutex;

use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::ids::TicketId;
use vela_domain::ports::{IssueTrackerAdapter, PortFuture, TrackerTicket, ready};

use super::simple::Recorder;

#[derive(Debug, Default)]
struct State {
    tickets: Vec<TrackerTicket>,
    statuses: BTreeMap<TicketId, String>,
    comments: Vec<(TicketId, String)>,
    closed: Vec<TicketId>,
    fail_writes: bool,
}

/// Holds tickets in memory; writes are recorded and can be made to fail (an outage).
#[derive(Debug, Default)]
pub struct FakeTracker {
    state: Mutex<State>,
    log: Recorder,
}

impl FakeTracker {
    /// An empty tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a ticket.
    pub fn add(&self, ticket: TrackerTicket) {
        self.state
            .lock()
            .expect("tracker lock")
            .tickets
            .push(ticket);
    }

    /// While set, every write fails with a temporary external error.
    pub fn set_outage(&self, down: bool) {
        self.state.lock().expect("tracker lock").fail_writes = down;
    }

    /// The last status written for `id`.
    pub fn status_of(&self, id: &TicketId) -> Option<String> {
        self.state
            .lock()
            .expect("tracker lock")
            .statuses
            .get(id)
            .cloned()
    }

    /// Evidence comments, in order.
    pub fn comments(&self) -> Vec<(TicketId, String)> {
        self.state.lock().expect("tracker lock").comments.clone()
    }

    /// Tickets closed, in order.
    pub fn closed(&self) -> Vec<TicketId> {
        self.state.lock().expect("tracker lock").closed.clone()
    }

    /// The recorded calls.
    pub fn log(&self) -> &Recorder {
        &self.log
    }

    fn write<T>(&self, entry: String, f: impl FnOnce(&mut State) -> T) -> PortFuture<'_, T>
    where
        T: Send + 'static,
    {
        let mut s = self.state.lock().expect("tracker lock");
        if s.fail_writes {
            self.log.record(format!("{entry}:outage"));
            return ready(Err(VelaError::of_class(
                ErrorClass::ExternalTemporary,
                "tracker is unreachable",
            )));
        }
        self.log.record(entry);
        ready(Ok(f(&mut s)))
    }
}

impl IssueTrackerAdapter for FakeTracker {
    fn list_tickets(&self) -> PortFuture<'_, Vec<TrackerTicket>> {
        self.log.record("list");
        ready(Ok(self.state.lock().expect("tracker lock").tickets.clone()))
    }

    fn read_ticket<'a>(&'a self, id: &'a TicketId) -> PortFuture<'a, TrackerTicket> {
        self.log.record(format!("read:{id}"));
        let found = self
            .state
            .lock()
            .expect("tracker lock")
            .tickets
            .iter()
            .find(|t| &t.id == id)
            .cloned();
        ready(
            found.ok_or_else(|| {
                VelaError::of_class(ErrorClass::ValidationFailure, "no such ticket")
            }),
        )
    }

    fn update_status<'a>(&'a self, id: &'a TicketId, status: &'a str) -> PortFuture<'a, ()> {
        let (id, status) = (id.clone(), status.to_owned());
        self.write(format!("status:{id}:{status}"), move |s| {
            s.statuses.insert(id, status);
        })
    }

    fn comment_evidence<'a>(&'a self, id: &'a TicketId, body: &'a str) -> PortFuture<'a, ()> {
        let (id, body) = (id.clone(), body.to_owned());
        self.write(format!("comment:{id}"), move |s| {
            s.comments.push((id, body));
        })
    }

    fn has_evidence_marker<'a>(
        &'a self,
        id: &'a TicketId,
        marker: &'a str,
    ) -> PortFuture<'a, bool> {
        let found = self
            .state
            .lock()
            .expect("tracker lock")
            .comments
            .iter()
            .any(|(t, body)| t == id && body.contains(marker));
        ready(Ok(found))
    }

    fn close_ticket<'a>(&'a self, id: &'a TicketId) -> PortFuture<'a, ()> {
        let id = id.clone();
        self.write(format!("close:{id}"), move |s| {
            s.closed.push(id);
        })
    }
}
