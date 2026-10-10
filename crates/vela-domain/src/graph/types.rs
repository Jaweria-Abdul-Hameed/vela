//! Graph vocabulary: edge kinds, edges, cycle reports and errors.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ids::{DependencyEdge, EdgeOrigin, TicketId};

/// One concrete dependency cycle.
///
/// `tickets` is ordered along the edges: each ticket blocks the next and the last blocks the first. A ticket cannot block
/// itself (the shared [`DependencyEdge`] cannot be built that way), so a cycle has at least two tickets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cycle {
    /// The participating tickets in edge order, starting at the smallest id.
    pub tickets: Vec<TicketId>,
    /// The edges closing the cycle, in the same order (`edges[i]` goes from `tickets[i]` to the next ticket).
    pub edges: Vec<DependencyEdge>,
    /// Every ticket in the strongly connected component the cycle belongs to, ascending. These tickets can all reach each other, so
    /// breaking the reported cycle may leave another one among them.
    pub component: Vec<TicketId>,
}

impl Cycle {
    /// True when every edge of the cycle is authored (stated by the ticket or tracker). A cycle that needs an analyst edge is
    /// caused by analysis, not by the user's dependencies; [`DependencyGraph::drop_cyclic_analyst_edges`] removes those. The
    /// report shows one cycle per component, so a component can also hold an authored cycle this one does not show: drop the
    /// analyst edges first and re-run [`DependencyGraph::find_cycles`] before blaming either side.
    ///
    /// [`DependencyGraph`]: super::DependencyGraph
    pub fn is_authored_only(&self) -> bool {
        self.edges
            .iter()
            .all(|e| e.origin() == EdgeOrigin::Authored)
    }
}

impl fmt::Display for Cycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for t in &self.tickets {
            write!(f, "{t} -> ")?;
        }
        match self.tickets.first() {
            Some(first) => write!(f, "{first}"),
            None => Ok(()),
        }
    }
}

/// Every cycle found (one per strongly connected component that contains a cycle), sorted by first ticket.
/// A non-empty report means the graph is invalid (`GRAPH_INVALID`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CycleReport {
    /// The cycles.
    pub cycles: Vec<Cycle>,
}

impl CycleReport {
    /// True when no cycle was found.
    pub fn is_empty(&self) -> bool {
        self.cycles.is_empty()
    }
}

impl fmt::Display for CycleReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("dependency cycle: ")?;
        for (i, c) in self.cycles.iter().enumerate() {
            if i > 0 {
                f.write_str("; ")?;
            }
            write!(f, "{c}")?;
        }
        Ok(())
    }
}

/// Why a graph operation failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    /// An edge or query named a ticket that is not in the graph.
    UnknownTicket(TicketId),
    /// The ticket is already in the graph.
    DuplicateTicket(TicketId),
    /// The graph contains a cycle (`GRAPH_INVALID`).
    Cycle(CycleReport),
    /// An analyst edge was refused because `blocked` already (transitively) blocks `blocker`. An inferred hazard only
    /// serializes a pair and never invalidates the graph (FR-007), so the caller treats the pair as sequential-but-unordered
    /// instead of adding the edge.
    HazardClosesCycle(DependencyEdge),
    /// A stored snapshot is not in canonical form (unsorted or duplicated entries, or layers that do not match its edges).
    SnapshotMismatch,
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTicket(t) => write!(f, "unknown ticket {t}"),
            Self::DuplicateTicket(t) => write!(f, "ticket {t} is already in the graph"),
            Self::Cycle(report) => report.fmt(f),
            Self::HazardClosesCycle(e) => write!(
                f,
                "analyst edge {} -> {} would close a dependency cycle",
                e.blocker(),
                e.blocked()
            ),
            Self::SnapshotMismatch => f.write_str("graph snapshot is not canonical"),
        }
    }
}

impl std::error::Error for GraphError {}
