//! A provider-neutral scripted agent: behaviour comes only from scenario definitions, never from any provider.
//! (Replaying recorded Antigravity transcripts is TK1; this ticket never consumes the A02 parser.)

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use vela_domain::errors::{ErrorClass, ErrorCode, VelaError};
use vela_domain::ids::WorkerId;
use vela_domain::ports::{
    AgentCapabilities, AgentCapability, AgentEvent, AgentEventSink, AgentProfile, AgentReviewer,
    AgentSession, ConversationId, PermissionPosture, PortFuture, ReviewReport, ReviewRequest,
    SessionRef, SessionStatus, TurnRequest, TurnResult, ready,
};

use super::simple::Recorder;

/// How a worker's turns end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scenario {
    /// Works, finishes.
    Success,
    /// Finishes but reports failing tests.
    FailingTests,
    /// The agent fails the work (an agent failure error).
    Failure,
    /// Never finishes within the turn timeout.
    Timeout,
    /// Stops on an approval it cannot get answered.
    Ambiguity,
    /// The provider reports exhausted capacity.
    Capacity,
    /// The provider blocks the account.
    ProviderPolicyBlock,
}

/// Plays a [`Scenario`] per worker (default [`Scenario::Success`]) and records every call.
#[derive(Debug, Default)]
pub struct ScriptedAgent {
    scenarios: Mutex<BTreeMap<WorkerId, Scenario>>,
    next_conversation: AtomicU64,
    log: Recorder,
}

impl ScriptedAgent {
    /// Every worker succeeds until told otherwise.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the scenario for `worker`.
    pub fn script(&self, worker: WorkerId, scenario: Scenario) {
        self.scenarios
            .lock()
            .expect("agent lock")
            .insert(worker, scenario);
    }

    /// The recorded calls.
    pub fn log(&self) -> &Recorder {
        &self.log
    }

    fn scenario_of(&self, worker: &WorkerId) -> Scenario {
        self.scenarios
            .lock()
            .expect("agent lock")
            .get(worker)
            .copied()
            .unwrap_or(Scenario::Success)
    }
}

impl AgentSession for ScriptedAgent {
    fn run_turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        sink: &'a dyn AgentEventSink,
    ) -> PortFuture<'a, TurnResult> {
        let scenario = self.scenario_of(&request.worker);
        self.log
            .record(format!("turn:{}:{scenario:?}", request.worker));
        let conversation = match &request.conversation {
            Some(c) => c.clone(),
            None => {
                let n = self.next_conversation.fetch_add(1, Ordering::SeqCst) + 1;
                ConversationId::new(format!("conv-{n}")).expect("valid conversation id")
            }
        };
        sink.on_event(AgentEvent::SessionStarted {
            conversation: Some(conversation.clone()),
        });
        let finished = |finished| {
            Ok(TurnResult {
                conversation: Some(conversation.clone()),
                finished,
            })
        };
        let step = |s: &str| AgentEvent::StepObserved {
            summary: s.to_owned(),
        };
        let result = match scenario {
            Scenario::Success => {
                sink.on_event(step("implemented the ticket"));
                sink.on_event(AgentEvent::TurnFinished);
                finished(true)
            }
            Scenario::FailingTests => {
                sink.on_event(step("tests failed"));
                sink.on_event(AgentEvent::TurnFinished);
                finished(true)
            }
            Scenario::Failure => {
                sink.on_event(step("gave up"));
                Err(VelaError::of_class(
                    ErrorClass::AgentFailure,
                    "the scripted agent failed the work",
                ))
            }
            Scenario::Timeout => {
                sink.on_event(step("still working"));
                finished(false)
            }
            Scenario::Ambiguity => {
                sink.on_event(AgentEvent::ApprovalBlocked {
                    description: "waiting on an approval".to_owned(),
                });
                finished(false)
            }
            Scenario::Capacity => {
                sink.on_event(AgentEvent::CapacityError {
                    message: "capacity exhausted".to_owned(),
                });
                Err(VelaError::of_class(
                    ErrorClass::ExternalTemporary,
                    "provider capacity exhausted",
                ))
            }
            Scenario::ProviderPolicyBlock => {
                sink.on_event(AgentEvent::ProviderPolicyBlock {
                    evidence: "account blocked".to_owned(),
                });
                Err(VelaError::from_code(
                    ErrorCode::ProviderPolicyBlock,
                    "the provider blocked the account",
                ))
            }
        };
        ready(result)
    }

    fn cancel<'a>(&'a self, session: &'a SessionRef) -> PortFuture<'a, ()> {
        self.log.record(format!("cancel:{}", session.worker));
        ready(Ok(()))
    }

    fn status<'a>(&'a self, session: &'a SessionRef) -> PortFuture<'a, SessionStatus> {
        self.log.record(format!("status:{}", session.worker));
        ready(Ok(SessionStatus::Idle))
    }
}

impl AgentProfile for ScriptedAgent {
    fn capabilities(&self) -> PortFuture<'_, AgentCapabilities> {
        ready(Ok(AgentCapabilities {
            version: "scripted-1".to_owned(),
            supported: vec![
                AgentCapability::Discovery,
                AgentCapability::FreshSession,
                AgentCapability::TaskDelivery,
                AgentCapability::LifecycleObservation,
                AgentCapability::Cancel,
                AgentCapability::StatusQuery,
                AgentCapability::SessionIdentity,
                AgentCapability::Resume,
            ],
            max_concurrent_sessions: Some(4),
        }))
    }

    fn permission_posture(&self) -> PortFuture<'_, PermissionPosture> {
        ready(Ok(PermissionPosture::Meets))
    }

    fn provision_profile<'a>(&'a self, session: &'a SessionRef) -> PortFuture<'a, ()> {
        self.log.record(format!("provision:{}", session.worker));
        ready(Ok(()))
    }
}

impl AgentReviewer for ScriptedAgent {
    fn review<'a>(
        &'a self,
        request: &'a ReviewRequest,
        sink: &'a dyn AgentEventSink,
    ) -> PortFuture<'a, ReviewReport> {
        self.log.record(format!("review:{}", request.worker));
        sink.on_event(AgentEvent::TurnFinished);
        ready(Ok(ReviewReport {
            output: "{\"findings\":[]}".to_owned(),
            conversation: None,
        }))
    }
}
