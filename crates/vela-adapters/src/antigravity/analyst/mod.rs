//! The Antigravity side of the dependency analyst (A11): one read-only turn that returns the agent's final text.
//!
//! The analyst runs in the worktree of the integration base under a profile whose rules deny every write, so even a prompt that
//! asks for changes cannot make any. The text is handed to the core, which validates it field by field.

use std::num::NonZeroU64;
use std::sync::Arc;

use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::ids::{WorkerId, WorktreePath};
use vela_domain::ports::{AgentEvent, AgentEventSink, PortFuture, TurnRequest};

use super::events::TurnClass;
use super::session::AgyDriver;

struct Ignore;

impl AgentEventSink for Ignore {
    fn on_event(&self, _event: AgentEvent) {}
}

/// Runs analysis turns through the driver.
pub struct AgyTextRunner {
    driver: Arc<AgyDriver>,
    worker: WorkerId,
    worktree: WorktreePath,
    timeout_ms: NonZeroU64,
}

impl AgyTextRunner {
    /// A runner for the analyst worker `worker` (its profile must be read-only) in `worktree`.
    pub fn new(driver: Arc<AgyDriver>, worker: WorkerId, worktree: WorktreePath) -> Self {
        Self {
            driver,
            worker,
            worktree,
            timeout_ms: NonZeroU64::new(600_000).expect("non-zero"),
        }
    }

    /// Runs `prompt` in a fresh conversation and returns the final text. A turn that did not end normally is an
    /// `AGENT_FAILURE`, so the caller falls back to sequential scheduling.
    pub fn run_text<'a>(&'a self, prompt: &'a str) -> PortFuture<'a, String> {
        Box::pin(async move {
            let outcome = self
                .driver
                .run_turn_outcome(
                    &TurnRequest {
                        worker: self.worker.clone(),
                        worktree: self.worktree.clone(),
                        prompt: prompt.to_owned(),
                        conversation: None,
                        timeout_ms: self.timeout_ms,
                    },
                    &Ignore,
                )
                .await?;
            if !matches!(outcome.class, TurnClass::Completed) {
                return Err(VelaError::of_class(
                    ErrorClass::AgentFailure,
                    format!("the analyst turn ended as {:?}", outcome.class),
                ));
            }
            Ok(outcome.response)
        })
    }
}
