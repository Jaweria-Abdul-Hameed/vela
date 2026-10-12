//! Deterministic, scriptable fakes for every port (F09), each recording its calls, plus a provider-neutral scripted agent.
//!
//! Fakes never touch the network, the user's home directory or the environment: they are plain in-memory state behind a
//! mutex. The shared conformance suite in [`conformance`] runs each fake against its port contract.

pub mod agent;
pub mod approval;
pub mod conformance;
pub mod git;
pub mod process;
pub mod simple;
pub mod tracker;

#[cfg(test)]
mod tests;

pub use agent::{Scenario, ScriptedAgent};
pub use approval::{FakeApprovalDelivery, FakeApprovalSource};
pub use git::FakeGit;
pub use process::{FakeProcessRunner, Scripted};
pub use simple::{FakeClock, FakeIds, FakeNotifier, FakePower, FakeSessionProbe, Recorder};
pub use tracker::FakeTracker;
