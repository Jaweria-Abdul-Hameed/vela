//! The dependency graph: a `petgraph::StableDiGraph` of tickets with typed edges.

use std::collections::{BTreeMap, BTreeSet};

use petgraph::Direction;
use petgraph::algo::has_path_connecting;
use petgraph::stable_graph::{NodeIndex, StableDiGraph};
use petgraph::visit::{EdgeRef, IntoEdgeReferences};

use super::cycle::find_cycles;
use super::types::{CycleReport, GraphError};
use crate::ids::{DependencyEdge, EdgeOrigin, TicketId};

/// A graph of tickets and the blocking relationships between them.
///
/// Building never fails on a cycle: cycles are reported by [`DependencyGraph::find_cycles`], and every computation that needs a
/// DAG ([`layers`](Self::layers), [`frontier`](Self::frontier)) returns [`GraphError::Cycle`] instead of a partial answer.
/// Every output is deterministic: ties are broken by ticket id, never by insertion order.
#[derive(Debug, Clone, Default)]
pub struct DependencyGraph {
    graph: StableDiGraph<TicketId, EdgeOrigin>,
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

    /// Adds an edge: its blocker must be done before its blocked ticket may start. Both tickets must be in the graph.
    ///
    /// One edge exists per ordered pair. Adding a pair again keeps the stronger origin, so an authored edge is never weakened
    /// or replaced by an analyst edge (FR-007). An analyst edge that would close a cycle is refused with
    /// [`GraphError::HazardClosesCycle`] and leaves the graph unchanged, so analysis cannot make a valid graph invalid. The
    /// check sees only the edges added so far: authored edges added later can still close a cycle through an analyst edge, and
    /// [`drop_cyclic_analyst_edges`](Self::drop_cyclic_analyst_edges) removes those.
    pub fn add_edge(&mut self, edge: &DependencyEdge) -> Result<(), GraphError> {
        let from = self.node(edge.blocker())?;
        let to = self.node(edge.blocked())?;
        if edge.origin() == EdgeOrigin::Analyst
            && self.graph.find_edge(from, to).is_none()
            && has_path_connecting(&self.graph, to, from, None)
        {
            return Err(GraphError::HazardClosesCycle(edge.clone()));
        }
        self.insert_edge(from, to, edge.origin());
        Ok(())
    }

    /// Inserts without the cycle check (used to reload a stored snapshot, whose validity is checked afterwards).
    pub(super) fn add_edge_unchecked(&mut self, edge: &DependencyEdge) -> Result<(), GraphError> {
        let from = self.node(edge.blocker())?;
        let to = self.node(edge.blocked())?;
        self.insert_edge(from, to, edge.origin());
        Ok(())
    }

    fn insert_edge(&mut self, from: NodeIndex, to: NodeIndex, origin: EdgeOrigin) {
        match self.graph.find_edge(from, to) {
            Some(existing) => {
                if origin == EdgeOrigin::Authored {
                    self.graph[existing] = EdgeOrigin::Authored;
                }
            }
            None => {
                self.graph.add_edge(from, to, origin);
            }
        }
    }

    /// Removes the analyst edges that lie on cycles, until every remaining cycle is authored-only, and returns them sorted.
    /// Authored edges are never touched. The caller treats each returned pair as sequential-but-unordered. A graph whose
    /// authored edges form a DAG is valid afterwards, whatever order the edges were added in.
    pub fn drop_cyclic_analyst_edges(&mut self) -> Vec<DependencyEdge> {
        let mut removed = Vec::new();
        loop {
            let report = self.find_cycles();
            let victims: Vec<DependencyEdge> = report
                .cycles
                .iter()
                .filter_map(|c| {
                    c.edges
                        .iter()
                        .find(|e| e.origin() == EdgeOrigin::Analyst)
                        .cloned()
                })
                .collect();
            if victims.is_empty() {
                break;
            }
            for edge in victims {
                if let (Ok(from), Ok(to)) = (self.node(edge.blocker()), self.node(edge.blocked()))
                    && let Some(id) = self.graph.find_edge(from, to)
                {
                    self.graph.remove_edge(id);
                    removed.push(edge);
                }
            }
        }
        removed.sort_by(|a, b| (a.blocker(), a.blocked()).cmp(&(b.blocker(), b.blocked())));
        removed
    }

    /// Every edge, sorted by (blocker, blocked).
    pub fn edges(&self) -> Vec<DependencyEdge> {
        let mut edges: Vec<DependencyEdge> = self
            .graph
            .edge_references()
            .map(|e| {
                DependencyEdge::new(
                    self.graph[e.source()].clone(),
                    self.graph[e.target()].clone(),
                    *e.weight(),
                )
                .expect("the graph holds no self-edge")
            })
            .collect();
        edges.sort_by(|a, b| (a.blocker(), a.blocked()).cmp(&(b.blocker(), b.blocked())));
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
