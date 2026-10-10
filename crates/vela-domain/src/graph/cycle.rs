//! Cycle detection: strongly connected components, then one shortest concrete cycle per component.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use petgraph::Direction;
use petgraph::algo::tarjan_scc;
use petgraph::stable_graph::{NodeIndex, StableDiGraph};

use super::types::{Cycle, CycleReport, DependencyEdge, EdgeKind};
use crate::ids::TicketId;

/// Finds the cycles of `graph`: for every strongly connected component that contains a cycle (more than one ticket, or a
/// ticket that blocks itself), the shortest cycle through the component's smallest ticket id. Deterministic.
pub(super) fn find_cycles(graph: &StableDiGraph<TicketId, EdgeKind>) -> CycleReport {
    let mut cycles = Vec::new();
    for component in tarjan_scc(graph) {
        let cyclic = component.len() > 1
            || component
                .first()
                .is_some_and(|n| graph.find_edge(*n, *n).is_some());
        if !cyclic {
            continue;
        }
        let start = component
            .iter()
            .copied()
            .min_by(|a, b| graph[*a].cmp(&graph[*b]))
            .expect("component is non-empty");
        cycles.push(shortest_cycle_through(graph, start, &component));
    }
    cycles.sort_by(|a, b| a.tickets.cmp(&b.tickets));
    CycleReport { cycles }
}

/// Breadth-first search from `start` back to `start`, staying inside `component`. Successors are visited in ticket-id order
/// so the answer does not depend on insertion order.
fn shortest_cycle_through(
    graph: &StableDiGraph<TicketId, EdgeKind>,
    start: NodeIndex,
    component: &[NodeIndex],
) -> Cycle {
    let members: BTreeSet<NodeIndex> = component.iter().copied().collect();
    let mut parent: BTreeMap<NodeIndex, NodeIndex> = BTreeMap::new();
    let mut queue = VecDeque::from([start]);
    let mut last = start;
    'search: while let Some(node) = queue.pop_front() {
        let mut successors: Vec<NodeIndex> = graph
            .neighbors_directed(node, Direction::Outgoing)
            .filter(|n| members.contains(n))
            .collect();
        successors.sort_by(|a, b| graph[*a].cmp(&graph[*b]));
        for succ in successors {
            if succ == start {
                last = node;
                break 'search;
            }
            if let std::collections::btree_map::Entry::Vacant(slot) = parent.entry(succ) {
                slot.insert(node);
                queue.push_back(succ);
            }
        }
    }
    let mut path = vec![last];
    while let Some(prev) = parent.get(path.last().expect("path is non-empty")) {
        path.push(*prev);
    }
    path.reverse();
    let edges = path
        .iter()
        .enumerate()
        .map(|(i, from)| {
            let to = path[(i + 1) % path.len()];
            let edge = graph
                .edges_connecting(*from, to)
                .next()
                .expect("consecutive cycle tickets are joined by an edge");
            DependencyEdge {
                blocker: graph[*from].clone(),
                blocked: graph[to].clone(),
                kind: *edge.weight(),
            }
        })
        .collect();
    let mut names: Vec<TicketId> = component.iter().map(|n| graph[*n].clone()).collect();
    names.sort();
    Cycle {
        tickets: path.iter().map(|n| graph[*n].clone()).collect(),
        edges,
        component: names,
    }
}
