//! The per-turn session driver (A03): one `agy` process per turn, run in the worker's worktree with an explicit print timeout,
//! cancellable through the process-tree facility, resumable by conversation id. Output is parsed as it streams so progress is
//! visible live, and the finished turn is interpreted by [`crate::antigravity::events::interpret_turn`], never by exit code.

use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use vela_domain::errors::{ErrorClass, ErrorCode, VelaError};
use vela_domain::ids::{WorkerId, WorktreePath};
use vela_domain::ports::{
    AgentEvent, AgentEventSink, AgentSession, CommandSpec, ConversationId, OutputChunk, OutputSink,
    OutputStream, PortFuture, ProcessRunner, SessionRef, SessionStatus, TurnRequest, TurnResult,
};

use super::events::{
    StepState, StreamEvent, TurnClass, TurnInput, TurnOutcome, interpret_turn, parse_line,
};

/// The longest prompt passed on the command line (Windows limits a command line to about 32 000 characters). Longer task
/// text is delivered through a file by the task envelope (A09).
pub const MAX_PROMPT_BYTES: usize = 24_000;

/// How much longer than the print timeout the runner waits before killing the process itself, so agy's own timeout fires first.
pub const TIMEOUT_GRACE_MS: u64 = 20_000;

fn default_probe_cwd() -> WorktreePath {
    let dir = std::env::temp_dir().to_string_lossy().replace('\\', "/");
    WorktreePath::new(dir)
        .unwrap_or_else(|_| WorktreePath::new("C:/").expect("a valid fallback path"))
}

/// The Antigravity CLI driver.
pub struct AgyDriver {
    runner: Arc<dyn ProcessRunner>,
    program: String,
    base_env: Vec<(String, String)>,
    next_turn: AtomicU64,
    active: Mutex<HashMap<WorkerId, String>>,
    outcomes: Mutex<HashMap<WorkerId, TurnOutcome>>,
    probe_cwd: WorktreePath,
    worker_env: Mutex<HashMap<WorkerId, Vec<(String, String)>>>,
}

impl std::fmt::Debug for AgyDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgyDriver")
            .field("program", &self.program)
            .finish()
    }
}

/// The `agy` arguments for one turn. A new session never passes `--conversation`; a resume passes exactly the recorded id.
pub fn turn_args(request: &TurnRequest) -> Result<Vec<String>, VelaError> {
    if request.prompt.trim().is_empty() {
        return Err(VelaError::of_class(
            ErrorClass::ValidationFailure,
            "the prompt is empty",
        ));
    }
    if request.prompt.len() > MAX_PROMPT_BYTES {
        return Err(VelaError::of_class(
            ErrorClass::ValidationFailure,
            "the prompt is too long for the command line; deliver the task through a file",
        ));
    }
    let seconds = request.timeout_ms.get().div_ceil(1000).max(1);
    let mut args = vec![
        "-p".to_owned(),
        request.prompt.clone(),
        "--output-format".to_owned(),
        "stream-json".to_owned(),
        "--print-timeout".to_owned(),
        format!("{seconds}s"),
    ];
    if let Some(conversation) = &request.conversation {
        args.push("--conversation".to_owned());
        args.push(conversation.as_str().to_owned());
    }
    Ok(args)
}

/// Collects the process output, forwarding each complete stdout line as an [`AgentEvent`] while it streams.
struct Forwarder<'a> {
    sink: &'a dyn AgentEventSink,
    partial: Mutex<String>,
    stdout: Mutex<String>,
    stderr: Mutex<String>,
}

impl<'a> Forwarder<'a> {
    fn new(sink: &'a dyn AgentEventSink) -> Self {
        Self {
            sink,
            partial: Mutex::new(String::new()),
            stdout: Mutex::new(String::new()),
            stderr: Mutex::new(String::new()),
        }
    }

    fn forward_line(&self, line: &str) {
        match parse_line(line) {
            Ok(StreamEvent::Init(init)) => self.sink.on_event(AgentEvent::SessionStarted {
                conversation: ConversationId::new(init.conversation_id).ok(),
            }),
            Ok(StreamEvent::Step(step))
                if step.step_type == "tool" && step.state == StepState::Active =>
            {
                let tool = step.tool_name.unwrap_or_default();
                let detail = step
                    .tool
                    .as_ref()
                    .and_then(|t| t.parameters.get("CommandLine").and_then(|v| v.as_str()))
                    .map(|c| format!(": {}", c.chars().take(120).collect::<String>()))
                    .unwrap_or_default();
                self.sink.on_event(AgentEvent::StepObserved {
                    summary: format!("{tool}{detail}"),
                });
            }
            _ => {}
        }
    }

    fn finish(self) -> (String, String) {
        let rest = std::mem::take(&mut *self.partial.lock().expect("partial lock"));
        if !rest.trim().is_empty() {
            self.stdout.lock().expect("stdout lock").push_str(&rest);
        }
        (
            self.stdout.into_inner().expect("stdout lock"),
            self.stderr.into_inner().expect("stderr lock"),
        )
    }
}

impl OutputSink for Forwarder<'_> {
    fn on_output(&self, chunk: OutputChunk) {
        match chunk.stream {
            OutputStream::Stderr => self
                .stderr
                .lock()
                .expect("stderr lock")
                .push_str(&chunk.text),
            OutputStream::Stdout => {
                let mut partial = self.partial.lock().expect("partial lock");
                partial.push_str(&chunk.text);
                while let Some(at) = partial.find('\n') {
                    let line: String = partial.drain(..=at).collect();
                    self.stdout.lock().expect("stdout lock").push_str(&line);
                    self.forward_line(line.trim_end());
                }
            }
        }
    }
}

impl AgyDriver {
    /// A driver that runs `agy` through `runner` with the given environment allowlist.
    pub fn new(runner: Arc<dyn ProcessRunner>, base_env: Vec<(String, String)>) -> Self {
        Self {
            runner,
            program: "agy".to_owned(),
            base_env,
            next_turn: AtomicU64::new(0),
            active: Mutex::new(HashMap::new()),
            outcomes: Mutex::new(HashMap::new()),
            probe_cwd: default_probe_cwd(),
            worker_env: Mutex::new(HashMap::new()),
        }
    }

    /// Sets environment overrides for one worker's turns (its isolated profile: `USERPROFILE` and `HOME`). They are added after
    /// the base environment, so they win.
    pub fn set_worker_env(&self, worker: WorkerId, env: Vec<(String, String)>) {
        self.worker_env
            .lock()
            .expect("worker env lock")
            .insert(worker, env);
    }

    /// Forgets a worker's environment overrides (the worker ended).
    pub fn clear_worker_env(&self, worker: &WorkerId) {
        self.worker_env
            .lock()
            .expect("worker env lock")
            .remove(worker);
    }

    /// Sets the folder discovery commands run in (they need no repository; the temp directory is the default).
    pub fn with_probe_cwd(mut self, cwd: WorktreePath) -> Self {
        self.probe_cwd = cwd;
        self
    }

    /// A capability probe that uses this driver's runner, program and environment.
    pub fn capability_probe(&self) -> super::capability::CapabilityProbe {
        super::capability::CapabilityProbe::new(
            self.runner.clone(),
            self.base_env.clone(),
            self.probe_cwd.clone(),
        )
    }

    /// Adds environment variables for every turn (for example the per-worker profile's `USERPROFILE`).
    pub fn with_env(mut self, extra: Vec<(String, String)>) -> Self {
        self.base_env.extend(extra);
        self
    }

    /// Sets the program (a path or name).
    pub fn with_program(mut self, program: impl Into<String>) -> Self {
        self.program = program.into();
        self
    }

    /// The interpreted outcome of the worker's last turn (denied actions, per-tool execution, usage).
    pub fn last_outcome(&self, worker: &WorkerId) -> Option<TurnOutcome> {
        self.outcomes
            .lock()
            .expect("outcomes lock")
            .get(worker)
            .cloned()
    }

    /// Runs one turn and returns both the interpreted outcome and the process timing.
    pub async fn run_turn_outcome(
        &self,
        request: &TurnRequest,
        sink: &dyn AgentEventSink,
    ) -> Result<TurnOutcome, VelaError> {
        let args = turn_args(request)?;
        let process_id = format!(
            "agy-{}-{}",
            request.worker,
            self.next_turn.fetch_add(1, Ordering::SeqCst) + 1
        );
        self.active
            .lock()
            .expect("active lock")
            .insert(request.worker.clone(), process_id.clone());
        let spec = CommandSpec {
            id: process_id,
            program: self.program.clone(),
            args,
            cwd: request.worktree.clone(),
            env: {
                let mut env = self.base_env.clone();
                if let Some(extra) = self
                    .worker_env
                    .lock()
                    .expect("worker env lock")
                    .get(&request.worker)
                {
                    env.extend(extra.iter().cloned());
                }
                env
            },
            timeout_ms: NonZeroU64::new(request.timeout_ms.get().saturating_add(TIMEOUT_GRACE_MS))
                .unwrap_or(request.timeout_ms),
        };
        let forwarder = Forwarder::new(sink);
        let ran = self.runner.run(&spec, &forwarder).await;
        self.active
            .lock()
            .expect("active lock")
            .remove(&request.worker);
        let process = ran?;
        let (stdout, stderr) = forwarder.finish();
        let mut outcome = interpret_turn(TurnInput {
            stdout: &stdout,
            stderr: &stderr,
            exit_code: process.exit_code,
            timed_out: process.timed_out,
            cancelled: process.cancelled,
        });
        // agy's own print timeout: the stream just stops, usually with exit code 0 (never a success).
        if matches!(outcome.class, TurnClass::Incomplete)
            && process.duration_ms >= request.timeout_ms.get()
        {
            outcome.class = TurnClass::TimedOut;
        }
        self.outcomes
            .lock()
            .expect("outcomes lock")
            .insert(request.worker.clone(), outcome.clone());
        Ok(outcome)
    }
}

fn describe_denial(outcome: &TurnOutcome) -> String {
    match outcome.denied.first() {
        Some(d) => format!(
            "{} was denied: headless mode cannot prompt for permission and no allow rule covers it",
            d.display_name
        ),
        None => "a tool was denied: headless mode cannot prompt for permission".to_owned(),
    }
}

impl AgentSession for AgyDriver {
    fn run_turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        sink: &'a dyn AgentEventSink,
    ) -> PortFuture<'a, TurnResult> {
        Box::pin(async move {
            let outcome = self.run_turn_outcome(request, sink).await?;
            let conversation = outcome
                .conversation_id
                .as_deref()
                .and_then(|id| ConversationId::new(id).ok());
            let result = |finished| TurnResult {
                conversation: conversation.clone(),
                finished,
                response: outcome.response.clone(),
            };
            match &outcome.class {
                TurnClass::Completed => {
                    sink.on_event(AgentEvent::TurnFinished);
                    Ok(result(true))
                }
                TurnClass::SoftDenied => {
                    sink.on_event(AgentEvent::ToolDenied {
                        tool: outcome
                            .denied
                            .first()
                            .map_or_else(|| "command".to_owned(), |d| d.action.clone()),
                        reason: describe_denial(&outcome),
                    });
                    sink.on_event(AgentEvent::TurnFinished);
                    Ok(result(true))
                }
                TurnClass::HookDenied { reason } => {
                    sink.on_event(AgentEvent::ToolDenied {
                        tool: "hook".to_owned(),
                        reason: reason.clone(),
                    });
                    sink.on_event(AgentEvent::TurnFinished);
                    Ok(result(true))
                }
                TurnClass::HookFailed { message } => {
                    sink.on_event(AgentEvent::ToolDenied {
                        tool: "hook".to_owned(),
                        reason: format!("a policy hook failed, so the tool did not run: {message}"),
                    });
                    sink.on_event(AgentEvent::TurnFinished);
                    Ok(result(true))
                }
                TurnClass::Waiting => {
                    sink.on_event(AgentEvent::ApprovalBlocked {
                        description: "the provider reported WAITING; an approval may be pending"
                            .to_owned(),
                    });
                    Ok(result(false))
                }
                TurnClass::TimedOut | TurnClass::Cancelled | TurnClass::Incomplete => {
                    Ok(result(false))
                }
                TurnClass::QuotaExhausted { text } => {
                    sink.on_event(AgentEvent::CapacityError {
                        message: text.clone(),
                    });
                    Err(VelaError::of_class(
                        ErrorClass::ExternalTemporary,
                        format!("the provider's capacity is exhausted: {text}"),
                    ))
                }
                TurnClass::AuthRequired { text } => {
                    sink.on_event(AgentEvent::AuthRequired {
                        message: text.clone(),
                    });
                    Err(VelaError::from_code(
                        ErrorCode::AuthRequired,
                        "Antigravity needs you to sign in (run agy once and sign in with your Google account)",
                    ))
                }
                TurnClass::ProviderPolicyBlock { text } => {
                    sink.on_event(AgentEvent::ProviderPolicyBlock {
                        evidence: text.clone(),
                    });
                    Err(VelaError::from_code(
                        ErrorCode::ProviderPolicyBlock,
                        "the provider blocked the account or service; Vela will not retry or work around it",
                    ))
                }
                TurnClass::Failed { message } => Err(VelaError::of_class(
                    ErrorClass::AgentFailure,
                    format!("the agent turn failed: {message}"),
                )),
            }
        })
    }

    fn cancel<'a>(&'a self, session: &'a SessionRef) -> PortFuture<'a, ()> {
        Box::pin(async move {
            let id = self
                .active
                .lock()
                .expect("active lock")
                .get(&session.worker)
                .cloned();
            match id {
                Some(id) => self.runner.kill_tree(&id).await,
                None => Ok(()),
            }
        })
    }

    fn status<'a>(&'a self, session: &'a SessionRef) -> PortFuture<'a, SessionStatus> {
        Box::pin(async move {
            let running = self
                .active
                .lock()
                .expect("active lock")
                .contains_key(&session.worker);
            Ok(if running {
                SessionStatus::Running
            } else {
                SessionStatus::Idle
            })
        })
    }
}

#[cfg(test)]
mod tests;
