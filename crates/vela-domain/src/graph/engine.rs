//! The dependency graph: a `petgraph::StableDiGraph` of tickets with typed edges.

use std::collections::{BTreeMap, BTreeSet};

use petgraph::Direction;
use petgraph::stable_graph::{NodeIndex, StableDiGraph};
use petgraph::visit::{EdgeRef, IntoEdgeReferences};

use super::cycle::find_cycles;
use super::types::{CycleReport, DependencyEdge, EdgeKind, GraphError};
use crate::ids::TicketId;

/// A graph of tickets and the blocking relationships between them.
///
/// Building never fails on a cycle: cycles are reported by [`DependencyGraph::find_cycles`], and every computation that needs a
/// DAG ([`layers`](Self::layers), [`frontier`](Self::frontier)) returns [`GraphError::Cycle`] instead of a partial answer.
/// Every output is deterministic: ties are broken by ticket id, never by insertion order.
#[derive(Debug, Clone, Default)]
pub struct DependencyGraph {
    graph: StableDiGraph<TicketId, EdgeKind>,
    index: BTreeMap<TicketId, NodeIndex>,
}

impl DependencyGraph {
    /// An empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of tickets.
    pub fn ticket_count(&self) -> usize {
        self.index.len()
    }

    /// Number of distinct edges (one per ordered pair of tickets).
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// True when the ticket is in the graph.
    pub fn contains(&self, ticket: &TicketId) -> bool {
        self.index.contains_key(ticket)
    }

    /// Adds a ticket.
    pub fn add_ticket(&mut self, ticket: TicketId) -> Result<(), GraphError> {
        if self.index.contains_key(&ticket) {
            return Err(GraphError::DuplicateTicket(ticket));
        }
        let node = self.graph.add_node(ticket.clone());
        self.index.insert(ticket, node);
        Ok(())
    }

    /// Removes a ticket and every edge touching it. Returns false when the ticket was not in the graph.
    pub fn remove_ticket(&mut self, ticket: &TicketId) -> bool {
        match self.index.remove(ticket) {
            Some(node) => {
                self.graph.remove_node(node);
                true
            }
            None => false,
        }
    }

    /// Adds an edge: `blocker` must be done before `blocked` may start.
    ///
    /// One edge exists per ordered pair. Adding a pair again keeps the stronger kind, so an explicit edge is never weakened
    /// or replaced by an inferred hazard (FR-007). A self-edge is accepted and reported as a one-ticket cycle.
    pub fn add_edge(
        &mut self,
        blocker: &TicketId,
        blocked: &TicketId,
        kind: EdgeKind,
    ) -> Result<(), GraphError> {
        let from = self.node(blocker)?;
        let to = self.node(blocked)?;
        match self.graph.find_edge(from, to) {
            Some(existing) => {
                if kind == EdgeKind::Explicit {
                    self.graph[existing] = EdgeKind::Explicit;
                }
            }
            None => {
                self.graph.add_edge(from, to, kind);
            }
        }
        Ok(())
    }

    /// Every edge, sorted by (blocker, blocked).
    pub fn edges(&self) -> Vec<DependencyEdge> {
        let mut edges: Vec<DependencyEdge> = self
            .graph
            .edge_references()
            .map(|e| DependencyEdge {
                blocker: self.graph[e.source()].clone(),
                blocked: self.graph[e.target()].clone(),
                kind: *e.weight(),
            })
            .collect();
        edges.sort();
        edges
    }

    /// Every ticket id in ascending order.
    pub fn tickets(&self) -> Vec<TicketId> {
        self.index.keys().cloned().collect()
    }

    /// The tickets that directly block `ticket`, in ascending order.
    pub fn blockers_of(&self, ticket: &TicketId) -> Result<Vec<TicketId>, GraphError> {
        let node = self.node(ticket)?;
        Ok(self.neighbours(node, Direction::Incoming))
    }

    /// The tickets that `ticket` directly blocks, in ascending order.
    pub fn dependents_of(&self, ticket: &TicketId) -> Result<Vec<TicketId>, GraphError> {
        let node = self.node(ticket)?;
        Ok(self.neighbours(node, Direction::Outgoing))
    }

    /// Every cycle in the graph, one per strongly connected component that contains one. Empty when the graph is a DAG.
    pub fn find_cycles(&self) -> CycleReport {
        find_cycles(&self.graph)
    }

    /// Ok when the graph is a DAG.
    pub fn validate(&self) -> Result<(), GraphError> {
        let report = self.find_cycles();
        if report.is_empty() {
            Ok(())
        } else {
            Err(GraphError::Cycle(report))
        }
    }

    /// Topological layers: layer 0 holds the tickets with no blockers, and every other ticket sits one layer after its
    /// deepest blocker. Each layer is sorted by id. Edges of every kind count.
    pub fn layers(&self) -> Result<Vec<Vec<TicketId>>, GraphError> {
        let mut remaining: BTreeMap<NodeIndex, usize> = self
            .graph
            .node_indices()
            .map(|n| {
                (
                    n,
                    self.graph
                        .neighbors_directed(n, Direction::Incoming)
                        .count(),
                )
            })
            .collect();
        let mut current: Vec<NodeIndex> = remaining
            .iter()
            .filter(|(_, degree)| **degree == 0)
            .map(|(n, _)| *n)
            .collect();
        let mut layers = Vec::new();
        let mut placed = 0;
        while !current.is_empty() {
            let mut layer: Vec<TicketId> = current.iter().map(|n| self.graph[*n].clone()).collect();
            layer.sort();
            placed += layer.len();
            let mut next = Vec::new();
            for node in &current {
                for succ in self.graph.neighbors_directed(*node, Direction::Outgoing) {
                    if let Some(degree) = remaining.get_mut(&succ) {
                        *degree -= 1;
                        if *degree == 0 {
                            next.push(succ);
                        }
                    }
                }
            }
            layers.push(layer);
            current = next;
        }
        if placed < self.index.len() {
            return Err(GraphError::Cycle(self.find_cycles()));
        }
        Ok(layers)
    }

    /// The ready frontier: tickets not yet done whose every blocker (of any kind) is done, sorted by id.
    ///
    /// `done` is the set of tickets whose work is merged and validated. A ticket whose blocker failed, was cancelled or needs
    /// a human is not in `done`, so its dependents are never on the frontier. Ids in `done` that are not in the graph are
    /// ignored.
    pub fn frontier(&self, done: &BTreeSet<TicketId>) -> Result<Vec<TicketId>, GraphError> {
        self.validate()?;
        let mut ready: Vec<TicketId> = self
            .index
            .iter()
            .filter(|(ticket, _)| !done.contains(*ticket))
            .filter(|(_, node)| {
                self.graph
                    .neighbors_directed(**node, Direction::Incoming)
                    .all(|blocker| done.contains(&self.graph[blocker]))
            })
            .map(|(ticket, _)| ticket.clone())
            .collect();
        ready.sort();
        Ok(ready)
    }

    pub(super) fn node(&self, ticket: &TicketId) -> Result<NodeIndex, GraphError> {
        self.index
            .get(ticket)
            .copied()
            .ok_or_else(|| GraphError::UnknownTicket(ticket.clone()))
    }

    fn neighbours(&self, node: NodeIndex, direction: Direction) -> Vec<TicketId> {
        let mut out: Vec<TicketId> = self
            .graph
            .neighbors_directed(node, direction)
            .map(|n| self.graph[n].clone())
            .collect();
        out.sort();
        out
    }
}
