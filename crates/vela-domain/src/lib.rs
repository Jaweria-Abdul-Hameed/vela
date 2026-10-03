//! vela-domain: module skeleton (F01). Modules are empty and implemented by the tickets that own them;
//! see docs/issues/graph/SHARED_SURFACE_PROTOCOL.md section 1. Never edit this declaration list in a ticket.
#![forbid(unsafe_code)]

pub mod approval;
pub mod errors;
pub mod events;
pub mod fault;
pub mod graph;
pub mod ids;
pub mod layout;
pub mod parallel;
pub mod policy;
pub mod ports;
pub mod review;
pub mod state;

#[cfg(test)]
mod tests {
    #[test]
    fn bootstrap_placeholder() {}
}
