//! Parallel-safety engine (D03, FR-007, FR-008, AT-003, `PARALLELIZATION.md`): decides whether two ready tickets may run
//! together from predicted write sets, deterministic hard blockers and the dependency graph. Pure logic, no I/O.
//!
//! Rule: dependency-safe AND write-safe AND contract-safe AND integration-safe. Anything uncertain is sequential, and
//! analyst hazards can only add serialization, never remove a deterministic hard blocker.

#[cfg(feature = "full")]
mod safety;

#[cfg(feature = "full")]
pub use safety::{
    AnalystHazard, EvidenceLabel, PairVerdict, PredictedWriteSet, Reason, SurfaceRules,
    classify_pair, select_parallel,
};
