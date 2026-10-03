//! `ProcessRunner`: runs configured commands (`ADAPTERS.md` "ProcessAdapter", component C10).

use crate::ids::WorktreePath;

use super::PortFuture;

/// A command to run. The caller assigns the id so it can cancel the process tree while `run` is still pending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    /// Caller-assigned identifier of this process, used by [`ProcessRunner::kill_tree`].
    pub id: String,
    /// The program to execute.
    pub program: String,
    /// Arguments, passed without a shell.
    pub args: Vec<String>,
    /// Working directory.
    pub cwd: WorktreePath,
    /// Allowlisted environment variables; nothing else is inherited.
    pub env: Vec<(String, String)>,
    /// Timeout in milliseconds; exceeding it kills the whole process tree.
    pub timeout_ms: u64,
}

/// Which stream a chunk of output came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputStream {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}

/// A chunk of redacted process output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputChunk {
    /// Source stream.
    pub stream: OutputStream,
    /// Redacted text.
    pub text: String,
}

/// Receives streamed output while a command runs.
pub trait OutputSink: Send + Sync {
    /// Called for each chunk of redacted output, in order.
    fn on_output(&self, chunk: OutputChunk);
}

/// How a command ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessOutcome {
    /// Exit code, when the process exited by itself.
    pub exit_code: Option<i32>,
    /// True when the timeout elapsed and the tree was killed.
    pub timed_out: bool,
    /// True when the tree was killed through [`ProcessRunner::kill_tree`].
    pub cancelled: bool,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
}

/// Runs commands with an environment allowlist, a timeout, cancellation, streamed redacted output and exit metadata.
///
/// Commands defined by an untrusted repository are never run (ADR-013); that decision is made before this port is
/// called. Process trees are tracked so cancellation and Stop All terminate the whole tree.
pub trait ProcessRunner: Send + Sync {
    /// Runs `spec` to completion, streaming output to `sink`.
    fn run<'a>(
        &'a self,
        spec: &'a CommandSpec,
        sink: &'a dyn OutputSink,
    ) -> PortFuture<'a, ProcessOutcome>;

    /// Kills the process tree started for `id`. Killing an unknown or finished id succeeds.
    fn kill_tree<'a>(&'a self, id: &'a str) -> PortFuture<'a, ()>;
}
