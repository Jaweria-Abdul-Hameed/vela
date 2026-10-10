//! Graph vocabulary: edge kinds, edges, cycle reports and errors.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ids::TicketId;

/// Why one ticket blocks another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EdgeKind {
    /// Authored by the user (or the tracker): a deterministic hard blocker. Never removed or rewritten by analysis (FR-007).
    Explicit,
    /// Added from evidence of a hazard (shared files, lockfile, schema, declared resource keys). Serializes the pair.
    InferredHazard,
}

/// A directed edge: `blocker` must be done before `blocked` may start.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DependencyEdge {
    /// The ticket that must finish first.
    pub blocker: TicketId,
    /// The ticket that waits.
    pub blocked: TicketId,
    /// Why the edge exists.
    pub kind: EdgeKind,
}

/// One concrete dependency cycle.
///
/// `tickets` is ordered along the edges: each ticket blocks the next and the last blocks the first. A ticket that blocks
/// itself is a cycle of one.
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
    /// True when every edge of the cycle is user-authored. A cycle that needs an inferred hazard is a hazard problem, not a
    /// defect in the user's dependencies, and callers may report the two differently.
    pub fn is_explicit_only(&self) -> bool {
        self.edges.iter().all(|e| e.kind == EdgeKind::Explicit)
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
    /// A stored snapshot is not in canonical form (unsorted or duplicated entries, or layers that do not match its edges).
    SnapshotMismatch,
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTicket(t) => write!(f, "unknown ticket {t}"),
            Self::DuplicateTicket(t) => write!(f, "ticket {t} is already in the graph"),
            Self::Cycle(report) => report.fmt(f),
            Self::SnapshotMismatch => f.write_str("graph snapshot is not canonical"),
        }
    }
}

impl std::error::Error for GraphError {}
