//! A scriptable `ProcessRunner`.

use std::collections::VecDeque;
use std::sync::Mutex;

use vela_domain::errors::VelaError;
use vela_domain::ports::{
    CommandSpec, OutputChunk, OutputSink, PortFuture, ProcessOutcome, ProcessRunner, ready,
};

use super::simple::Recorder;

/// One scripted run: the output to stream and how it ends.
#[derive(Debug, Clone)]
pub struct Scripted {
    /// Chunks streamed to the sink, in order.
    pub output: Vec<OutputChunk>,
    /// The result, or an error to return instead of an outcome.
    pub result: Result<ProcessOutcome, VelaError>,
}

impl Scripted {
    /// A run that exits with `code`.
    pub fn exit(code: i32) -> Self {
        Self {
            output: Vec::new(),
            result: Ok(ProcessOutcome {
                exit_code: Some(code),
                timed_out: false,
                cancelled: false,
                duration_ms: 1,
            }),
        }
    }

    /// A run that hits its timeout.
    pub fn timeout() -> Self {
        Self {
            output: Vec::new(),
            result: Ok(ProcessOutcome {
                exit_code: None,
                timed_out: true,
                cancelled: false,
                duration_ms: 1,
            }),
        }
    }

    /// Adds a chunk of output.
    pub fn with_output(mut self, chunk: OutputChunk) -> Self {
        self.output.push(chunk);
        self
    }
}

/// Runs nothing: pops the next [`Scripted`] (exit code 0 when the script is empty) and records the call. Environment values
/// are never recorded.
#[derive(Debug, Default)]
pub struct FakeProcessRunner {
    script: Mutex<VecDeque<Scripted>>,
    log: Recorder,
    killed: Mutex<Vec<String>>,
}

impl FakeProcessRunner {
    /// An empty script.
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues the next run.
    pub fn push(&self, scripted: Scripted) {
        self.script.lock().expect("script lock").push_back(scripted);
    }

    /// The recorded calls (`run:<program> <args>`).
    pub fn log(&self) -> &Recorder {
        &self.log
    }

    /// Ids passed to `kill_tree`, in order.
    pub fn killed(&self) -> Vec<String> {
        self.killed.lock().expect("killed lock").clone()
    }
}

impl ProcessRunner for FakeProcessRunner {
    fn run<'a>(
        &'a self,
        spec: &'a CommandSpec,
        sink: &'a dyn OutputSink,
    ) -> PortFuture<'a, ProcessOutcome> {
        self.log
            .record(format!("run:{} {}", spec.program, spec.args.join(" ")));
        let scripted = self
            .script
            .lock()
            .expect("script lock")
            .pop_front()
            .unwrap_or_else(|| Scripted::exit(0));
        for chunk in scripted.output {
            sink.on_output(chunk);
        }
        ready(scripted.result)
    }

    fn kill_tree<'a>(&'a self, id: &'a str) -> PortFuture<'a, ()> {
        self.killed.lock().expect("killed lock").push(id.to_owned());
        self.log.record(format!("kill:{id}"));
        ready(Ok(()))
    }
}
