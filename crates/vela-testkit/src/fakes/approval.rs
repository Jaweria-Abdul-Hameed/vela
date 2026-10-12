//! Fakes for `ApprovalSource` and `ApprovalDeliveryAdapter`.

use std::collections::VecDeque;
use std::sync::Mutex;

use vela_domain::errors::{ErrorCode, VelaError};
use vela_domain::ports::{
    ApprovalDeliveryAdapter, ApprovalObservation, ApprovalSource, DeliveryCapability,
    DeliveryRequest, DeliveryResult, DeliveryTier, PortFuture, ready,
};

use super::simple::Recorder;

/// Hands out queued observations one at a time.
#[derive(Debug, Default)]
pub struct FakeApprovalSource {
    queue: Mutex<VecDeque<ApprovalObservation>>,
}

impl FakeApprovalSource {
    /// An empty queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues an observation.
    pub fn push(&self, observation: ApprovalObservation) {
        self.queue
            .lock()
            .expect("source lock")
            .push_back(observation);
    }
}

impl ApprovalSource for FakeApprovalSource {
    fn next_observation(&self) -> PortFuture<'_, Option<ApprovalObservation>> {
        ready(Ok(self.queue.lock().expect("source lock").pop_front()))
    }
}

#[derive(Debug)]
struct DeliveryState {
    available: bool,
    verified: bool,
}

/// Delivers nothing for real: records the request, and fails closed when it is made unavailable.
#[derive(Debug)]
pub struct FakeApprovalDelivery {
    tier: DeliveryTier,
    state: Mutex<DeliveryState>,
    log: Recorder,
}

impl FakeApprovalDelivery {
    /// Available, and every delivery is verified.
    pub fn new(tier: DeliveryTier) -> Self {
        Self {
            tier,
            state: Mutex::new(DeliveryState {
                available: true,
                verified: true,
            }),
            log: Recorder::new(),
        }
    }

    /// Makes the adapter unavailable (the interactive session is locked, for example).
    pub fn set_available(&self, available: bool) {
        self.state.lock().expect("delivery lock").available = available;
    }

    /// Makes the next deliveries report that the state transition was not observed.
    pub fn set_verified(&self, verified: bool) {
        self.state.lock().expect("delivery lock").verified = verified;
    }

    /// The recorded calls.
    pub fn log(&self) -> &Recorder {
        &self.log
    }
}

impl ApprovalDeliveryAdapter for FakeApprovalDelivery {
    fn detect(&self) -> PortFuture<'_, DeliveryCapability> {
        let available = self.state.lock().expect("delivery lock").available;
        ready(Ok(DeliveryCapability {
            tier: self.tier,
            available,
        }))
    }

    fn deliver<'a>(&'a self, request: &'a DeliveryRequest) -> PortFuture<'a, DeliveryResult> {
        let (available, verified) = {
            let s = self.state.lock().expect("delivery lock");
            (s.available, s.verified)
        };
        if !available {
            self.log.record("deliver:unavailable");
            return ready(Err(VelaError::from_code(
                ErrorCode::ApprovalUndeliverable,
                "no delivery surface is available",
            )));
        }
        self.log.record(format!(
            "deliver:{}:{:?}",
            request.observation.source_id, request.decision
        ));
        ready(Ok(DeliveryResult {
            verified,
            detail: "fake delivery".to_owned(),
        }))
    }

    fn cancel<'a>(&'a self, observation: &'a ApprovalObservation) -> PortFuture<'a, ()> {
        self.log.record(format!("cancel:{}", observation.source_id));
        ready(Ok(()))
    }
}
