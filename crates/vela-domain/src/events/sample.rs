//! Test-only sample values, so that every event payload can be round-tripped with non-default data in every field.

use crate::errors::{ErrorCode, VelaError};
use crate::ids::{
    BranchName, CommitSha, GateResult, GateStatus, ReviewSeverity, RunId, TicketId, WorkerId,
    WorktreePath,
};

pub(crate) trait Sample {
    fn sample() -> Self;
}

impl Sample for String {
    fn sample() -> Self {
        "sample".to_owned()
    }
}
impl Sample for bool {
    fn sample() -> Self {
        true
    }
}
impl Sample for u32 {
    fn sample() -> Self {
        7
    }
}
impl Sample for u64 {
    fn sample() -> Self {
        1_234
    }
}
impl Sample for i32 {
    fn sample() -> Self {
        -3
    }
}
impl Sample for i64 {
    fn sample() -> Self {
        1_700_000_000_000
    }
}
impl<T: Sample> Sample for Option<T> {
    fn sample() -> Self {
        Some(T::sample())
    }
}
impl<T: Sample> Sample for Vec<T> {
    fn sample() -> Self {
        vec![T::sample()]
    }
}
impl Sample for RunId {
    fn sample() -> Self {
        Self::new("run-1").unwrap()
    }
}
impl Sample for TicketId {
    fn sample() -> Self {
        Self::new("F02").unwrap()
    }
}
impl Sample for WorkerId {
    fn sample() -> Self {
        Self::new("worker-1").unwrap()
    }
}
impl Sample for CommitSha {
    fn sample() -> Self {
        Self::new("3e5aafe745ef11887380e0ec9a5dad76fdb3d41d").unwrap()
    }
}
impl Sample for BranchName {
    fn sample() -> Self {
        Self::new("f02/domain-primitives").unwrap()
    }
}
impl Sample for WorktreePath {
    fn sample() -> Self {
        Self::new(r"C:\Vela\worktrees\f02").unwrap()
    }
}
impl Sample for ReviewSeverity {
    fn sample() -> Self {
        Self::Medium
    }
}
impl Sample for GateResult {
    fn sample() -> Self {
        Self {
            gate: "cargo test".to_owned(),
            status: GateStatus::Pass,
            exit_code: Some(0),
            duration_ms: Some(42),
            summary: "ok".to_owned(),
        }
    }
}
impl Sample for VelaError {
    fn sample() -> Self {
        Self::from_code(ErrorCode::PolicyViolation, "sample failure")
    }
}
