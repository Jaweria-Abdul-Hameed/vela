//! The graph snapshot shape stored per run: tickets, typed edges and the computed layers, all in a canonical order.

use serde::{Deserialize, Serialize};

use super::engine::DependencyGraph;
use super::types::GraphError;
use crate::ids::{DependencyEdge, TicketId};

/// The immutable, canonical form of a validated graph. Only a DAG can be snapshotted, and loading a snapshot re-validates it,
/// so a stored snapshot never smuggles in a cycle or an edge to a missing ticket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphSnapshot {
    /// Every ticket, ascending.
    pub tickets: Vec<TicketId>,
    /// Every edge, sorted by (blocker, blocked).
    pub edges: Vec<DependencyEdge>,
    /// The topological layers (derived from `tickets` and `edges`; checked on load).
    pub layers: Vec<Vec<TicketId>>,
}

impl DependencyGraph {
    /// The canonical snapshot of a valid graph; [`GraphError::Cycle`] when the graph has a cycle.
    pub fn snapshot(&self) -> Result<GraphSnapshot, GraphError> {
        Ok(GraphSnapshot {
            tickets: self.tickets(),
            edges: self.edges(),
            layers: self.layers()?,
        })
    }

    /// Rebuilds a graph from a stored snapshot, rejecting duplicate tickets, edges to unknown tickets, cycles
    /// ([`GraphError::Cycle`], whatever the edge origins) and anything not in canonical form (unsorted or duplicated entries, or
    /// layers that do not match the tickets and edges).
    pub fn from_snapshot(snapshot: &GraphSnapshot) -> Result<Self, GraphError> {
        let mut graph = Self::new();
        for ticket in &snapshot.tickets {
            graph.add_ticket(ticket.clone())?;
        }
        for edge in &snapshot.edges {
            graph.add_edge_unchecked(edge)?;
        }
        let rebuilt = graph.snapshot()?;
        if rebuilt != *snapshot {
            return Err(GraphError::SnapshotMismatch);
        }
        Ok(graph)
    }
}
