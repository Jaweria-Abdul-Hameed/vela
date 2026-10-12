//! Dependency graph engine (D02, `IMPLEMENTATION_ARCHITECTURE.md` section 10, component C02): a validated DAG of tickets with
//! typed edges, cycle reports, the ready frontier and topological layers. Pure logic, no I/O.
//!
//! The engine needs `petgraph`, which is behind the `full` feature (the hook binary builds without it), so everything here is
//! compiled only with `full`.

#[cfg(feature = "full")]
mod cycle;
#[cfg(feature = "full")]
mod engine;
#[cfg(feature = "full")]
mod snapshot;
#[cfg(all(test, feature = "full"))]
mod tests;
#[cfg(feature = "full")]
mod types;

#[cfg(feature = "full")]
pub use engine::DependencyGraph;
#[cfg(feature = "full")]
pub use snapshot::GraphSnapshot;
#[cfg(feature = "full")]
pub use types::{Cycle, CycleReport, GraphError};
