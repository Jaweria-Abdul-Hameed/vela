//! Deterministic layered graph layout (D04, FR-025): coordinates per ticket computed from a [`GraphSnapshot`] alone, so the
//! same snapshot always gives the same layout and a ticket's status can never move a node. Pure logic, no I/O.
//!
//! Layers come from the snapshot's topological layers; order inside a layer is refined by barycenter sweeps with ticket id as
//! the tie-break. Coordinates are integers on a fixed grid.

#[cfg(feature = "full")]
mod layered;

#[cfg(feature = "full")]
pub use layered::{GraphLayout, LAYER_GAP, NODE_GAP, NodePosition, layout};
