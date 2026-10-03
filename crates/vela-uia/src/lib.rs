//! vela-uia: module skeleton (F01). Modules are empty and implemented by the tickets that own them;
//! see docs/issues/graph/SHARED_SURFACE_PROTOCOL.md section 1. Never edit this declaration list in a ticket.
#![cfg(windows)]

pub mod delivery;
pub mod discovery;
pub mod visual;

#[cfg(test)]
mod tests {
    #[test]
    fn bootstrap_placeholder() {}
}
