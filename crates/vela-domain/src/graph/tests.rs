use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use proptest::prelude::*;

use super::*;
use crate::ids::{DependencyEdge, EdgeOrigin as EdgeKind, TicketId};

fn t(s: &str) -> TicketId {
    TicketId::new(s).expect("valid id")
}

fn n(i: usize) -> TicketId {
    t(&format!("T{i:04}"))
}

fn edge(a: usize, b: usize, kind: EdgeKind) -> DependencyEdge {
    DependencyEdge::new(n(a), n(b), kind).expect("no self-edge")
}

fn build(count: usize, edges: &[(usize, usize, EdgeKind)]) -> DependencyGraph {
    let mut g = DependencyGraph::new();
    for i in 0..count {
        g.add_ticket(n(i)).unwrap();
    }
    for (a, b, kind) in edges.iter().filter(|(a, b, _)| a != b) {
        g.add_edge(&edge(*a, *b, *kind)).unwrap();
    }
    g
}

/// Adds edges in the given order, skipping hazards the graph refuses (as the graph builder does).
fn build_tolerant(count: usize, edges: &[(usize, usize, EdgeKind)]) -> DependencyGraph {
    let mut g = DependencyGraph::new();
    for i in 0..count {
        g.add_ticket(n(i)).unwrap();
    }
    for (a, b, kind) in edges.iter().filter(|(a, b, _)| a != b) {
        match g.add_edge(&edge(*a, *b, *kind)) {
            Ok(()) | Err(GraphError::HazardClosesCycle(_)) => {}
            Err(other) => panic!("{other}"),
        }
    }
    g
}

fn done(ids: &[usize]) -> BTreeSet<TicketId> {
    ids.iter().map(|i| n(*i)).collect()
}

#[test]
fn diamond_has_three_layers_and_a_frontier_that_follows_completion() {
    let g = build(
        4,
        &[
            (0, 1, EdgeKind::Authored),
            (0, 2, EdgeKind::Authored),
            (1, 3, EdgeKind::Authored),
            (2, 3, EdgeKind::Analyst),
        ],
    );
    assert_eq!(
        g.layers().unwrap(),
        vec![vec![n(0)], vec![n(1), n(2)], vec![n(3)]]
    );
    assert_eq!(g.frontier(&done(&[])).unwrap(), vec![n(0)]);
    assert_eq!(g.frontier(&done(&[0])).unwrap(), vec![n(1), n(2)]);
    // The hazard edge gates ticket 3 exactly like an explicit one.
    assert_eq!(g.frontier(&done(&[0, 1])).unwrap(), vec![n(2)]);
    assert_eq!(g.frontier(&done(&[0, 1, 2])).unwrap(), vec![n(3)]);
    assert!(g.frontier(&done(&[0, 1, 2, 3])).unwrap().is_empty());
}

#[test]
fn a_failed_blocker_keeps_its_dependents_off_the_frontier() {
    let g = build(3, &[(0, 1, EdgeKind::Authored), (1, 2, EdgeKind::Authored)]);
    // Ticket 0 never reaches `done` (it failed): nothing behind it is ready, and it is still reported ready itself.
    assert_eq!(g.frontier(&done(&[])).unwrap(), vec![n(0)]);
    assert!(!g.frontier(&done(&[])).unwrap().contains(&n(1)));
}

#[test]
fn unknown_done_ids_are_ignored() {
    let g = build(2, &[(0, 1, EdgeKind::Authored)]);
    assert_eq!(g.frontier(&done(&[0, 99])).unwrap(), vec![n(1)]);
}

#[test]
fn a_cycle_is_reported_with_its_participants_in_edge_order() {
    let g = build(
        5,
        &[
            (0, 1, EdgeKind::Authored),
            (3, 1, EdgeKind::Analyst),
            (1, 2, EdgeKind::Authored),
            (2, 3, EdgeKind::Authored),
            (3, 4, EdgeKind::Authored),
        ],
    );
    let report = g.find_cycles();
    assert_eq!(report.cycles.len(), 1);
    let cycle = &report.cycles[0];
    assert_eq!(cycle.tickets, vec![n(1), n(2), n(3)]);
    assert_eq!(cycle.component, vec![n(1), n(2), n(3)]);
    assert!(!cycle.is_authored_only());
    assert_eq!(cycle.to_string(), "T0001 -> T0002 -> T0003 -> T0001");
    assert!(matches!(g.validate(), Err(GraphError::Cycle(_))));
    assert!(matches!(g.layers(), Err(GraphError::Cycle(_))));
    assert!(matches!(g.frontier(&done(&[])), Err(GraphError::Cycle(_))));
    assert!(matches!(g.snapshot(), Err(GraphError::Cycle(_))));
}

#[test]
fn the_component_names_tickets_outside_the_shortest_cycle() {
    // 0 -> 1 -> 0 is the shortest cycle, but 2 also lies on a cycle through 1 (1 -> 2 -> 3 -> 1).
    let g = build(
        4,
        &[
            (0, 1, EdgeKind::Authored),
            (1, 0, EdgeKind::Authored),
            (1, 2, EdgeKind::Authored),
            (2, 3, EdgeKind::Authored),
            (3, 1, EdgeKind::Authored),
        ],
    );
    let report = g.find_cycles();
    assert_eq!(report.cycles.len(), 1);
    assert_eq!(report.cycles[0].tickets, vec![n(0), n(1)]);
    assert_eq!(report.cycles[0].component, vec![n(0), n(1), n(2), n(3)]);
}

#[test]
fn separate_cycles_are_each_reported() {
    let g = build(
        6,
        &[
            (4, 5, EdgeKind::Authored),
            (5, 4, EdgeKind::Authored),
            (0, 1, EdgeKind::Authored),
            (1, 0, EdgeKind::Authored),
            (1, 2, EdgeKind::Authored),
        ],
    );
    let firsts: Vec<_> = g
        .find_cycles()
        .cycles
        .iter()
        .map(|c| c.tickets.clone())
        .collect();
    assert_eq!(firsts, vec![vec![n(0), n(1)], vec![n(4), n(5)]]);
}

#[test]
fn an_analyst_edge_that_would_close_a_cycle_is_refused_and_changes_nothing() {
    let mut g = build(3, &[(0, 1, EdgeKind::Authored), (1, 2, EdgeKind::Authored)]);
    let before = g.snapshot().unwrap();
    for (a, b) in [(2, 0), (1, 0)] {
        let err = g.add_edge(&edge(a, b, EdgeKind::Analyst)).unwrap_err();
        assert_eq!(
            err,
            GraphError::HazardClosesCycle(edge(a, b, EdgeKind::Analyst))
        );
    }
    assert_eq!(g.snapshot().unwrap(), before);
    // A forward analyst edge is fine.
    g.add_edge(&edge(0, 2, EdgeKind::Analyst)).unwrap();
    assert!(g.validate().is_ok());
    // An authored back edge is the user's, so it is accepted and reported rather than refused.
    g.add_edge(&edge(2, 0, EdgeKind::Authored)).unwrap();
    assert!(g.validate().is_err());
}

#[test]
fn analyst_edges_added_before_the_authored_edges_that_close_them_can_be_dropped() {
    // Analyst 0 -> 1 first; then the authored 1 -> 2 and 2 -> 0 close a cycle through it.
    let mut g = build(
        4,
        &[
            (0, 1, EdgeKind::Analyst),
            (1, 2, EdgeKind::Authored),
            (2, 0, EdgeKind::Authored),
            (2, 3, EdgeKind::Analyst),
        ],
    );
    assert!(g.validate().is_err());
    assert_eq!(
        g.drop_cyclic_analyst_edges(),
        vec![edge(0, 1, EdgeKind::Analyst)]
    );
    // The authored edges form a DAG, so the graph is valid again; the analyst edge off the cycle stays.
    assert!(g.validate().is_ok());
    assert_eq!(g.edge_count(), 3);
}

#[test]
fn an_analyst_edge_on_a_second_cycle_of_the_same_component_is_dropped_too() {
    // The authored 0 <-> 1 cycle is the shortest and is the one reported; the analyst 0 -> 2 closes another through 2 -> 0.
    let mut g = build(
        3,
        &[
            (0, 1, EdgeKind::Authored),
            (1, 0, EdgeKind::Authored),
            (0, 2, EdgeKind::Analyst),
            (2, 0, EdgeKind::Authored),
        ],
    );
    assert_eq!(
        g.drop_cyclic_analyst_edges(),
        vec![edge(0, 2, EdgeKind::Analyst)]
    );
    let report = g.find_cycles();
    assert_eq!(report.cycles.len(), 1);
    assert_eq!(report.cycles[0].component, vec![n(0), n(1)]);
    assert!(report.cycles[0].is_authored_only());
}

#[test]
fn conflicting_analyst_edges_resolve_the_same_way_in_any_order() {
    let forward = [(0, 1, EdgeKind::Analyst), (1, 0, EdgeKind::Analyst)];
    let mut a = DependencyGraph::new();
    let mut b = DependencyGraph::new();
    for i in 0..2 {
        a.add_ticket(n(i)).unwrap();
        b.add_ticket(n(i)).unwrap();
    }
    for (x, y, k) in forward {
        a.add_edge_unchecked(&edge(x, y, k)).unwrap();
    }
    for (x, y, k) in forward.iter().rev() {
        b.add_edge_unchecked(&edge(*x, *y, *k)).unwrap();
    }
    assert_eq!(a.drop_cyclic_analyst_edges(), b.drop_cyclic_analyst_edges());
    assert_eq!(a.edges(), b.edges());
    assert!(a.validate().is_ok());
}

#[test]
fn dropping_cyclic_analyst_edges_never_removes_an_authored_edge() {
    let mut g = build(
        3,
        &[
            (0, 1, EdgeKind::Authored),
            (1, 2, EdgeKind::Authored),
            (2, 0, EdgeKind::Authored),
        ],
    );
    assert!(g.drop_cyclic_analyst_edges().is_empty());
    assert_eq!(g.edge_count(), 3);
    assert!(g.find_cycles().cycles[0].is_authored_only());
}

#[test]
fn an_explicit_edge_is_never_weakened_by_a_hazard() {
    let mut g = build(2, &[(0, 1, EdgeKind::Analyst)]);
    g.add_edge(&edge(0, 1, EdgeKind::Authored)).unwrap();
    g.add_edge(&edge(0, 1, EdgeKind::Analyst)).unwrap();
    assert_eq!(g.edge_count(), 1);
    assert_eq!(g.edges()[0].origin(), EdgeKind::Authored);
}

#[test]
fn unknown_and_duplicate_tickets_are_rejected() {
    let mut g = build(1, &[]);
    assert_eq!(
        g.add_edge(&edge(0, 7, EdgeKind::Authored)),
        Err(GraphError::UnknownTicket(n(7)))
    );
    assert_eq!(g.add_ticket(n(0)), Err(GraphError::DuplicateTicket(n(0))));
    assert!(g.blockers_of(&n(7)).is_err());
}

#[test]
fn removing_a_ticket_removes_its_edges() {
    let mut g = build(3, &[(0, 1, EdgeKind::Authored), (1, 2, EdgeKind::Authored)]);
    assert!(g.remove_ticket(&n(1)));
    assert!(!g.remove_ticket(&n(1)));
    assert_eq!(g.edge_count(), 0);
    assert_eq!(g.frontier(&done(&[])).unwrap(), vec![n(0), n(2)]);
}

#[test]
fn results_do_not_depend_on_insertion_order() {
    let edges = [
        (0, 2, EdgeKind::Authored),
        (1, 2, EdgeKind::Authored),
        (2, 3, EdgeKind::Analyst),
    ];
    let forward = build(4, &edges);
    let mut reversed = DependencyGraph::new();
    for i in (0..4).rev() {
        reversed.add_ticket(n(i)).unwrap();
    }
    for (a, b, k) in edges.iter().rev() {
        reversed.add_edge(&edge(*a, *b, *k)).unwrap();
    }
    assert_eq!(forward.snapshot().unwrap(), reversed.snapshot().unwrap());
}

#[test]
fn snapshot_round_trips_and_rejects_tampering() {
    let g = build(4, &[(0, 1, EdgeKind::Authored), (1, 3, EdgeKind::Analyst)]);
    let snap = g.snapshot().unwrap();
    let json = serde_json::to_string(&snap).unwrap();
    let back: GraphSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(back, snap);
    assert_eq!(
        DependencyGraph::from_snapshot(&back)
            .unwrap()
            .snapshot()
            .unwrap(),
        snap
    );

    let mut wrong_layers = snap.clone();
    wrong_layers.layers.reverse();
    assert_eq!(
        DependencyGraph::from_snapshot(&wrong_layers).unwrap_err(),
        GraphError::SnapshotMismatch
    );

    let mut cyclic = snap.clone();
    cyclic.edges.push(edge(3, 0, EdgeKind::Authored));
    assert!(matches!(
        DependencyGraph::from_snapshot(&cyclic),
        Err(GraphError::Cycle(_))
    ));

    let mut dangling = snap;
    dangling.edges.push(edge(0, 9, EdgeKind::Authored));
    assert_eq!(
        DependencyGraph::from_snapshot(&dangling).unwrap_err(),
        GraphError::UnknownTicket(n(9))
    );
}

#[test]
fn five_hundred_node_graph_is_processed_within_the_recorded_time() {
    // A layered graph: 50 layers of 10 tickets, each ticket blocked by two tickets of the previous layer, plus one
    // inferred hazard between neighbours of the same layer pair.
    let (width, depth) = (10, 50);
    let mut edges = Vec::new();
    for layer in 1..depth {
        for i in 0..width {
            let me = layer * width + i;
            let prev = (layer - 1) * width;
            edges.push((prev + i, me, EdgeKind::Authored));
            edges.push((prev + (i + 1) % width, me, EdgeKind::Analyst));
        }
    }
    let started = Instant::now();
    let g = build(width * depth, &edges);
    let layers = g.layers().unwrap();
    assert_eq!(layers.len(), depth);
    assert!(g.find_cycles().is_empty());
    let mut completed = BTreeSet::new();
    let mut waves = 0;
    loop {
        let frontier = g.frontier(&completed).unwrap();
        if frontier.is_empty() {
            break;
        }
        waves += 1;
        completed.extend(frontier);
    }
    let snapshot = g.snapshot().unwrap();
    let elapsed = started.elapsed();
    assert_eq!(waves, depth);
    assert_eq!(completed.len(), width * depth);
    assert_eq!(
        DependencyGraph::from_snapshot(&snapshot)
            .unwrap()
            .snapshot()
            .unwrap(),
        snapshot
    );
    eprintln!(
        "D02 500-node graph: build, layers, {waves} frontier waves, snapshot and reload in {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "500-node graph took {elapsed:?}"
    );
}

/// Brute-force reachability, the oracle for cycle detection.
fn reaches(count: usize, edges: &[(usize, usize)]) -> Vec<Vec<bool>> {
    let mut r = vec![vec![false; count]; count];
    for (a, b) in edges {
        r[*a][*b] = true;
    }
    for k in 0..count {
        let via = r[k].clone();
        for row in r.iter_mut() {
            if row[k] {
                for (cell, reachable) in row.iter_mut().zip(&via) {
                    *cell |= *reachable;
                }
            }
        }
    }
    r
}

fn arbitrary_graph() -> impl Strategy<Value = (usize, Vec<(usize, usize, bool)>)> {
    (1usize..12).prop_flat_map(|count| {
        (
            Just(count),
            proptest::collection::vec((0..count, 0..count, any::<bool>()), 0..30),
        )
    })
}

fn index_of(ticket: &TicketId) -> usize {
    ticket.as_str()[1..].parse().expect("generated id")
}

fn kind(explicit: bool) -> EdgeKind {
    if explicit {
        EdgeKind::Authored
    } else {
        EdgeKind::Analyst
    }
}

proptest! {
    #[test]
    fn cycle_detection_agrees_with_reachability_and_reports_real_cycles(
        (count, raw) in arbitrary_graph()
    ) {
        let typed: Vec<_> = raw.iter().map(|(a, b, e)| (*a, *b, kind(*e))).collect();
        let g = build_tolerant(count, &typed);
        let pairs: Vec<_> = g
            .edges()
            .iter()
            .map(|e| (index_of(e.blocker()), index_of(e.blocked())))
            .collect();
        let r = reaches(count, &pairs);
        let has_cycle = (0..count).any(|i| r[i][i]);
        let report = g.find_cycles();
        prop_assert_eq!(!report.is_empty(), has_cycle);
        prop_assert_eq!(g.validate().is_err(), has_cycle);
        let edge_set: BTreeSet<_> = g.edges().into_iter().map(|e| (e.blocker().clone(), e.blocked().clone())).collect();
        for cycle in &report.cycles {
            prop_assert!(!cycle.tickets.is_empty());
            prop_assert_eq!(cycle.tickets.len(), cycle.edges.len());
            let distinct: BTreeSet<_> = cycle.tickets.iter().collect();
            prop_assert_eq!(distinct.len(), cycle.tickets.len());
            for (i, ticket) in cycle.tickets.iter().enumerate() {
                let next = &cycle.tickets[(i + 1) % cycle.tickets.len()];
                prop_assert!(edge_set.contains(&(ticket.clone(), next.clone())));
                prop_assert_eq!(cycle.edges[i].blocker(), ticket);
                prop_assert_eq!(cycle.edges[i].blocked(), next);
            }
        }
        // Exactly one cycle per cyclic strongly connected component, and `component` is that component.
        let mut expected: BTreeSet<Vec<TicketId>> = BTreeSet::new();
        for (i, row) in r.iter().enumerate() {
            if row[i] {
                expected.insert((0..count).filter(|j| row[*j] && r[*j][i]).map(n).collect());
            }
        }
        let reported: Vec<_> = report.cycles.iter().map(|c| c.component.clone()).collect();
        prop_assert_eq!(reported.len(), expected.len());
        prop_assert_eq!(reported.into_iter().collect::<BTreeSet<_>>(), expected);
        for cycle in &report.cycles {
            prop_assert!(cycle.tickets.iter().all(|t| cycle.component.contains(t)));
            prop_assert_eq!(&cycle.tickets[0], cycle.component.iter().min().unwrap());
        }
    }

    #[test]
    fn dropping_analyst_edges_leaves_only_authored_cycles_and_all_authored_edges(
        (count, raw) in arbitrary_graph()
    ) {
        let typed: Vec<_> = raw.iter().map(|(a, b, e)| (*a, *b, kind(*e))).collect();
        let mut g = build_tolerant(count, &typed);
        let authored_before: Vec<_> = g.edges().into_iter().filter(|e| e.origin() == EdgeKind::Authored).collect();
        let removed = g.drop_cyclic_analyst_edges();
        prop_assert!(removed.iter().all(|e| e.origin() == EdgeKind::Analyst));
        let authored_after: Vec<_> = g.edges().into_iter().filter(|e| e.origin() == EdgeKind::Authored).collect();
        prop_assert_eq!(authored_before, authored_after);
        prop_assert!(g.find_cycles().cycles.iter().all(|c| c.is_authored_only()));
        // No analyst edge lies on any cycle: its blocked ticket cannot reach its blocker.
        let pairs: Vec<_> = g
            .edges()
            .iter()
            .map(|e| (index_of(e.blocker()), index_of(e.blocked())))
            .collect();
        let r = reaches(count, &pairs);
        for e in g.edges().iter().filter(|e| e.origin() == EdgeKind::Analyst) {
            prop_assert!(!r[index_of(e.blocked())][index_of(e.blocker())]);
        }
        // If the authored edges alone form a DAG, nothing the analyst added can leave the graph invalid.
        let authored_only = build_tolerant(
            count,
            &typed.iter().filter(|(_, _, k)| *k == EdgeKind::Authored).cloned().collect::<Vec<_>>(),
        );
        if authored_only.validate().is_ok() {
            prop_assert!(g.validate().is_ok());
        }
    }

    #[test]
    fn layers_and_frontier_respect_every_edge_on_dags(
        (count, raw) in arbitrary_graph(),
        order_seed in any::<u64>(),
        done_mask in proptest::collection::vec(any::<bool>(), 12),
    ) {
        // Orient every edge from the lower to the higher rank of a random permutation, which makes the graph acyclic.
        let mut rank: Vec<usize> = (0..count).collect();
        let mut s = order_seed;
        for i in (1..count).rev() {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            rank.swap(i, (s >> 33) as usize % (i + 1));
        }
        let typed: Vec<_> = raw
            .iter()
            .filter(|(a, b, _)| a != b)
            .map(|(a, b, e)| {
                let (lo, hi) = if rank[*a] < rank[*b] { (*a, *b) } else { (*b, *a) };
                (lo, hi, kind(*e))
            })
            .collect();
        let g = build(count, &typed);
        prop_assert!(g.validate().is_ok());

        let layers = g.layers().unwrap();
        let mut layer_of = BTreeMap::new();
        for (depth, layer) in layers.iter().enumerate() {
            prop_assert!(!layer.is_empty());
            prop_assert!(layer.windows(2).all(|w| w[0] < w[1]));
            for ticket in layer {
                prop_assert!(layer_of.insert(ticket.clone(), depth).is_none());
            }
        }
        prop_assert_eq!(layer_of.len(), count);
        for e in g.edges() {
            prop_assert!(layer_of[e.blocker()] < layer_of[e.blocked()]);
        }
        // Tightness: a ticket is in layer d > 0 only because a blocker sits in layer d - 1.
        for (ticket, depth) in &layer_of {
            if *depth > 0 {
                let blockers = g.blockers_of(ticket).unwrap();
                prop_assert!(blockers.iter().any(|b| layer_of[b] + 1 == *depth));
            }
        }

        // Frontier soundness and completeness for an arbitrary done set.
        let done: BTreeSet<_> = (0..count).filter(|i| done_mask[*i]).map(n).collect();
        let frontier = g.frontier(&done).unwrap();
        for ticket in &frontier {
            prop_assert!(!done.contains(ticket));
            prop_assert!(g.blockers_of(ticket).unwrap().iter().all(|b| done.contains(b)));
        }
        for i in 0..count {
            let ticket = n(i);
            let ready = !done.contains(&ticket)
                && g.blockers_of(&ticket).unwrap().iter().all(|b| done.contains(b));
            prop_assert_eq!(frontier.contains(&ticket), ready);
        }
    }

    #[test]
    fn finishing_the_frontier_repeatedly_completes_a_dag_in_layer_order(
        (count, raw) in arbitrary_graph(),
    ) {
        let typed: Vec<_> = raw
            .iter()
            .filter(|(a, b, _)| a < b)
            .map(|(a, b, e)| (*a, *b, kind(*e)))
            .collect();
        let g = build(count, &typed);
        let mut completed = BTreeSet::new();
        let mut waves = Vec::new();
        loop {
            let frontier = g.frontier(&completed).unwrap();
            if frontier.is_empty() {
                break;
            }
            completed.extend(frontier.iter().cloned());
            waves.push(frontier);
        }
        prop_assert_eq!(completed.len(), count);
        // With every ticket finishing at once, the waves are exactly the topological layers.
        prop_assert_eq!(waves, g.layers().unwrap());
    }
}
