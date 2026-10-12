//! The authoritative reviewer session (A10) is not implemented yet. The driver already satisfies the `AgentAdapter` bound
//! so the rest of the system can be composed; asking it to review reports the capability as missing. A10 replaces this impl.
//! With `review_mode = off` (ADR-024) no reviewer session is ever started.

use vela_domain::errors::{ErrorCode, VelaError};
use vela_domain::ports::{AgentEventSink, AgentReviewer, PortFuture, ReviewReport, ReviewRequest};

use super::session::AgyDriver;

impl AgentReviewer for AgyDriver {
    fn review<'a>(
        &'a self,
        _request: &'a ReviewRequest,
        _sink: &'a dyn AgentEventSink,
    ) -> PortFuture<'a, ReviewReport> {
        Box::pin(async {
            Err(VelaError::from_code(
                ErrorCode::CapabilityMissing,
                "review is off by default (review_mode=off, ADR-024); the reviewer session is not implemented",
            ))
        })
    }
}
