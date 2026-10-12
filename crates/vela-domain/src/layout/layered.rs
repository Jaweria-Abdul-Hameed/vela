use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::graph::GraphSnapshot;
use crate::ids::TicketId;

/// Horizontal distance between consecutive layers.
pub const LAYER_GAP: i64 = 240;
/// Vertical distance between neighbours inside a layer (even, so centring stays on the grid).
pub const NODE_GAP: i64 = 120;

const SWEEPS: usize = 4;

/// Where one ticket sits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodePosition {
    /// The ticket.
    pub ticket: TicketId,
    /// Layer index (0 is the first layer).
    pub layer: u32,
    /// Position inside the layer after ordering.
    pub order: u32,
    /// Grid x (`layer * LAYER_GAP`).
    pub x: i64,
    /// Grid y, centred on 0 per layer.
    pub y: i64,
}

/// The layout of a whole snapshot, nodes sorted by (layer, order).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphLayout {
    /// Every ticket exactly once.
    pub nodes: Vec<NodePosition>,
}

/// Lays out `snapshot`. Depends on nothing but the snapshot, so status changes cannot affect it.
pub fn layout(snapshot: &GraphSnapshot) -> GraphLayout {
    let mut layers: Vec<Vec<TicketId>> = snapshot
        .layers
        .iter()
        .map(|l| {
            let mut l = l.clone();
            l.sort();
            l
        })
        .collect();

    let mut neighbours: BTreeMap<&TicketId, Vec<&TicketId>> = BTreeMap::new();
    for e in &snapshot.edges {
        neighbours.entry(e.blocker()).or_default().push(e.blocked());
        neighbours.entry(e.blocked()).or_default().push(e.blocker());
    }

    for sweep in 0..SWEEPS {
        let order: Vec<usize> = if sweep % 2 == 0 {
            (0..layers.len()).collect()
        } else {
            (0..layers.len()).rev().collect()
        };
        for li in order {
            let pos = normalized_positions(&layers);
            let mut keyed: Vec<(f64, TicketId)> = layers[li]
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let own = pos[t];
                    let ns = neighbours.get(t).map(Vec::as_slice).unwrap_or(&[]);
                    let key = if ns.is_empty() {
                        own
                    } else {
                        ns.iter().map(|n| pos[*n]).sum::<f64>() / ns.len() as f64
                    };
                    let _ = i;
                    (key, t.clone())
                })
                .collect();
            keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
            layers[li] = keyed.into_iter().map(|(_, t)| t).collect();
        }
    }

    let mut nodes = Vec::with_capacity(snapshot.tickets.len());
    for (li, layer) in layers.iter().enumerate() {
        let n = layer.len() as i64;
        for (oi, t) in layer.iter().enumerate() {
            nodes.push(NodePosition {
                ticket: t.clone(),
                layer: li as u32,
                order: oi as u32,
                x: li as i64 * LAYER_GAP,
                y: (2 * oi as i64 - (n - 1)) * (NODE_GAP / 2),
            });
        }
    }
    GraphLayout { nodes }
}

fn normalized_positions(layers: &[Vec<TicketId>]) -> BTreeMap<&TicketId, f64> {
    let mut map = BTreeMap::new();
    for layer in layers {
        let denom = (layer.len().max(2) - 1) as f64;
        for (i, t) in layer.iter().enumerate() {
            map.insert(t, i as f64 / denom);
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::DependencyGraph;
    use crate::ids::{DependencyEdge, EdgeOrigin};
    use std::collections::BTreeSet;

    fn id(s: &str) -> TicketId {
        TicketId::new(s).unwrap()
    }

    fn graph(n: usize, edges: &[(usize, usize)]) -> GraphSnapshot {
        let mut g = DependencyGraph::new();
        for i in 0..n {
            g.add_ticket(id(&format!("T{i:03}"))).unwrap();
        }
        for (a, b) in edges {
            let e = DependencyEdge::new(
                id(&format!("T{a:03}")),
                id(&format!("T{b:03}")),
                EdgeOrigin::Authored,
            )
            .unwrap();
            g.add_edge(&e).unwrap();
        }
        g.snapshot().unwrap()
    }

    #[test]
    fn same_input_same_coordinates() {
        let s = graph(6, &[(0, 2), (1, 2), (2, 4), (3, 4), (4, 5)]);
        assert_eq!(layout(&s), layout(&s.clone()));
    }

    #[test]
    fn every_ticket_once_and_no_overlap() {
        let s = graph(
            40,
            &(0..39)
                .filter(|i| i % 3 != 0)
                .map(|i| (i, i + 1))
                .collect::<Vec<_>>(),
        );
        let l = layout(&s);
        assert_eq!(l.nodes.len(), 40);
        let coords: BTreeSet<(i64, i64)> = l.nodes.iter().map(|n| (n.x, n.y)).collect();
        assert_eq!(coords.len(), 40);
        let tickets: BTreeSet<_> = l.nodes.iter().map(|n| n.ticket.clone()).collect();
        assert_eq!(tickets.len(), 40);
    }

    #[test]
    fn blockers_sit_in_earlier_layers() {
        let s = graph(5, &[(0, 1), (1, 2), (0, 3), (3, 4)]);
        let l = layout(&s);
        let layer_of = |t: &str| l.nodes.iter().find(|n| n.ticket == id(t)).unwrap().layer;
        assert!(layer_of("T000") < layer_of("T001"));
        assert!(layer_of("T001") < layer_of("T002"));
    }

    #[test]
    fn five_hundred_nodes() {
        let edges: Vec<(usize, usize)> = (0..500)
            .flat_map(|i| [(i, i + 7), (i, i + 13)])
            .filter(|(_, b)| *b < 500)
            .collect();
        let s = graph(500, &edges);
        let start = std::time::Instant::now();
        let l = layout(&s);
        assert_eq!(l.nodes.len(), 500);
        assert!(
            start.elapsed().as_secs() < 5,
            "layout took {:?}",
            start.elapsed()
        );
    }
}
