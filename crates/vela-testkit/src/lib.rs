//! vela-testkit: module skeleton (F01). Modules are empty and implemented by the tickets that own them;
//! see docs/issues/graph/SHARED_SURFACE_PROTOCOL.md section 1. Never edit this declaration list in a ticket.

pub mod fake_agy;
pub mod fake_gh;
pub mod fakes;
pub mod fault_harness;
pub mod fixture_repo;
pub mod recovery_matrix;

#[cfg(test)]
mod tests {
    #[test]
    fn bootstrap_placeholder() {}
}
