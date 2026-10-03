//! vela-core: module skeleton (F01). Modules are empty and implemented by the tickets that own them;
//! see docs/issues/graph/SHARED_SURFACE_PROTOCOL.md section 1. Never edit this declaration list in a ticket.
#![forbid(unsafe_code)]

pub mod analysis;
pub mod broker;
pub mod capacity;
pub mod context;
pub mod conversations;
pub mod diagnostics;
pub mod finalize;
pub mod intervention;
pub mod merge_lane;
pub mod notifications;
pub mod operations;
pub mod orchestrator;
pub mod preflight;
pub mod project;
pub mod promotion;
pub mod provisioning;
pub mod recovery;
pub mod remote_queue;
pub mod review;
pub mod run;
pub mod scheduler;
pub mod settings;
pub mod stop;
pub mod trust;
pub mod worker;

#[cfg(test)]
mod tests {
    #[test]
    fn bootstrap_placeholder() {}
}
