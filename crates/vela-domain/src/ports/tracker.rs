//! `IssueTrackerAdapter` (`ADAPTERS.md`; component C14). Initial implementations: the local Markdown tracker and GitHub.

use crate::errors::{ErrorCode, VelaError};
use crate::ids::TicketId;

use super::PortFuture;

/// A ticket as read from a tracker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackerTicket {
    /// The ticket's identity.
    pub id: TicketId,
    /// Title.
    pub title: String,
    /// Body (untrusted text: it never changes Vela policy, ADR-013).
    pub body: String,
    /// Explicit blockers.
    pub blockers: Vec<TicketId>,
    /// Labels.
    pub labels: Vec<String>,
}

/// Reads tickets and writes status back to the tracker.
pub trait IssueTrackerAdapter: Send + Sync {
    /// Lists the tickets in scope.
    fn list_tickets(&self) -> PortFuture<'_, Vec<TrackerTicket>>;
    /// Reads one ticket.
    fn read_ticket<'a>(&'a self, id: &'a TicketId) -> PortFuture<'a, TrackerTicket>;
    /// Updates the ticket's status label.
    fn update_status<'a>(&'a self, id: &'a TicketId, status: &'a str) -> PortFuture<'a, ()>;
    /// Comments evidence on the ticket.
    fn comment_evidence<'a>(&'a self, id: &'a TicketId, body: &'a str) -> PortFuture<'a, ()>;
    /// Closes or resolves the ticket according to policy.
    fn close_ticket<'a>(&'a self, id: &'a TicketId) -> PortFuture<'a, ()>;
    /// Creates or links a pull request where the tracker supports it. The default reports the capability as missing.
    fn link_pull_request<'a>(&'a self, id: &'a TicketId, url: &'a str) -> PortFuture<'a, ()> {
        let _ = (id, url);
        Box::pin(std::future::ready(Err(VelaError::from_code(
            ErrorCode::CapabilityMissing,
            "this tracker does not support pull requests",
        ))))
    }
}
