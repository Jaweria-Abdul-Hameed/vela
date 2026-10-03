//! Identifiers and value objects (F02; `DOMAIN_MODEL.md` "Value objects").
//!
//! Identifier newtypes validate at construction and on deserialization, so a malformed identifier read back from the
//! journal or received over IPC is a typed error rather than a silently accepted value. Everything here is pure: no I/O,
//! no clock, no randomness (identifier generation is the `IdGenerator` port).

#![warn(missing_docs)]

mod identifiers;
mod values;

pub use identifiers::{BranchName, CommitSha, IdError, RunId, TicketId, WorkerId, WorktreePath};
pub use values::{DependencyEdge, EdgeOrigin, GateResult, GateStatus, ReviewSeverity, RiskClass};
