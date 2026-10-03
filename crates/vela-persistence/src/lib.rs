//! vela-persistence: module skeleton (F01). Modules are empty and implemented by the tickets that own them;
//! see docs/issues/graph/SHARED_SURFACE_PROTOCOL.md section 1. Never edit this declaration list in a ticket.

pub mod integrity;
pub mod journal;
pub mod migrations;
pub mod recovery;
pub mod repositories;
pub mod writer;

#[cfg(test)]
mod tests {
    #[test]
    fn bootstrap_placeholder() {}
}
