//! vela-git: module skeleton (F01). Modules are empty and implemented by the tickets that own them;
//! see docs/issues/graph/SHARED_SURFACE_PROTOCOL.md section 1. Never edit this declaration list in a ticket.

pub mod checkpoint;
pub mod lock;
pub mod merge;
pub mod read;
pub mod remote;
pub mod worktree;

#[cfg(test)]
mod tests {
    #[test]
    fn bootstrap_placeholder() {}
}
