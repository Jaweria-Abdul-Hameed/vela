//! `ProcessRunner`: runs configured commands (`ADAPTERS.md` "ProcessAdapter", component C10).

use std::fmt;

use crate::ids::WorktreePath;

use super::PortFuture;

/// A command to run. The caller assigns the id so it can cancel the process tree while `run` is still pending.
#[derive(Clone, PartialEq, Eq)]
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

/// `Debug` shows the environment variable *names* only: allowlisted values can still be sensitive (paths, tokens) and
/// specs end up in logs and panics.
impl fmt::Debug for CommandSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.env.iter().map(|(name, _)| name.as_str()).collect();
        f.debug_struct("CommandSpec")
            .field("id", &self.id)
            .field("program", &self.program)
            .field("args", &self.args)
            .field("cwd", &self.cwd)
            .field("env", &names)
            .field("timeout_ms", &self.timeout_ms)
            .finish()
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_never_contains_environment_values() {
        let spec = CommandSpec {
            id: "p-1".into(),
            program: "git".into(),
            args: vec!["status".into()],
            cwd: WorktreePath::new("/repo").unwrap(),
            env: vec![("TOKEN".into(), "s3cr3t-value".into())],
            timeout_ms: 1_000,
        };
        let shown = format!("{spec:?}");
        assert!(shown.contains("TOKEN"));
        assert!(!shown.contains("s3cr3t-value"), "{shown}");
    }
}
