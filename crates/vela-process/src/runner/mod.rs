//! `ProcessRunner` implementation (P01, FR-012): no shell, a cleared environment plus the caller's allowlist, a hard timeout,
//! streamed redacted output with a size cap, exit metadata, and a Job Object per tree so cancellation finds every child.
//!
//! Commands defined by an untrusted repository are never passed here; that decision is made before this port is called.

use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::ports::{
    CommandSpec, OutputChunk, OutputSink, OutputStream, PortFuture, ProcessOutcome, ProcessRunner,
};

use crate::redact::{SharedRedactor, StreamRedactor};
use crate::tree::{JobTree, TreeRegistry};

/// Default cap on streamed output per run (both streams together).
pub const DEFAULT_MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

/// Shown once when the cap is reached; the rest of the output is drained and dropped.
pub const OUTPUT_CAPPED: &str = "\n[output truncated: size cap reached]\n";

/// Environment variables that are safe to pass on so ordinary tools can start on Windows. Callers build the real allowlist;
/// this is a convenience that reads the parent environment and returns only these names.
pub const BASELINE_ENV: &[&str] = &[
    "PATH",
    "PATHEXT",
    "SystemRoot",
    "SYSTEMROOT",
    "ComSpec",
    "TEMP",
    "TMP",
    "USERPROFILE",
    "LOCALAPPDATA",
    "APPDATA",
    "ProgramFiles",
    "ProgramFiles(x86)",
    "ProgramData",
    "windir",
];

/// The parent's values for [`BASELINE_ENV`], in that order, skipping unset names.
pub fn baseline_env() -> Vec<(String, String)> {
    BASELINE_ENV
        .iter()
        .filter_map(|name| std::env::var(name).ok().map(|v| ((*name).to_owned(), v)))
        .collect()
}

/// Tokio-based runner.
#[derive(Clone)]
pub struct TokioProcessRunner {
    redactor: SharedRedactor,
    registry: Arc<TreeRegistry>,
    max_output_bytes: usize,
    cancel_grace: Duration,
}

impl std::fmt::Debug for TokioProcessRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokioProcessRunner")
            .field("max_output_bytes", &self.max_output_bytes)
            .field("cancel_grace", &self.cancel_grace)
            .finish()
    }
}

impl TokioProcessRunner {
    /// A runner that redacts through `redactor` and tracks its trees in `registry`.
    pub fn new(redactor: SharedRedactor, registry: Arc<TreeRegistry>) -> Self {
        Self {
            redactor,
            registry,
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
            cancel_grace: Duration::from_millis(500),
        }
    }

    /// Changes the output cap.
    pub fn with_max_output_bytes(mut self, max: usize) -> Self {
        self.max_output_bytes = max;
        self
    }

    /// Changes how long a cancelled tree gets to exit politely before it is forced.
    pub fn with_cancel_grace(mut self, grace: Duration) -> Self {
        self.cancel_grace = grace;
        self
    }

    /// The registry of running trees.
    pub fn registry(&self) -> &Arc<TreeRegistry> {
        &self.registry
    }
}

fn spawn_error(spec: &CommandSpec, e: &std::io::Error) -> VelaError {
    let class = if e.kind() == std::io::ErrorKind::NotFound {
        ErrorClass::UserActionRequired
    } else {
        ErrorClass::ExternalTemporary
    };
    VelaError::of_class(
        class,
        format!("could not start `{}`: {}", spec.program, e.kind()),
    )
}

async fn pump<R: AsyncRead + Unpin>(
    mut reader: R,
    stream: OutputStream,
    redactor: SharedRedactor,
    sink: &dyn OutputSink,
    used: &AtomicUsize,
    cap: usize,
) {
    let mut redact = StreamRedactor::new(redactor);
    let mut buf = [0u8; 8192];
    let mut capped = false;
    loop {
        let n = match reader.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        if capped {
            continue;
        }
        let text = redact.push_bytes(&buf[..n]);
        if text.is_empty() {
            continue;
        }
        let before = used.fetch_add(text.len(), Ordering::SeqCst);
        if before + text.len() > cap {
            capped = true;
            sink.on_output(OutputChunk {
                stream,
                text: OUTPUT_CAPPED.to_owned(),
            });
            continue;
        }
        sink.on_output(OutputChunk { stream, text });
    }
    if !capped {
        let rest = redact.finish();
        if !rest.is_empty() {
            sink.on_output(OutputChunk { stream, text: rest });
        }
    }
}

impl ProcessRunner for TokioProcessRunner {
    fn run<'a>(
        &'a self,
        spec: &'a CommandSpec,
        sink: &'a dyn OutputSink,
    ) -> PortFuture<'a, ProcessOutcome> {
        Box::pin(async move {
            let mut cmd = Command::new(&spec.program);
            cmd.args(&spec.args)
                .current_dir(spec.cwd.as_str())
                .env_clear()
                .envs(spec.env.iter().map(|(k, v)| (k, v)))
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            #[cfg(windows)]
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW

            let started = Instant::now();
            let mut child = cmd.spawn().map_err(|e| spawn_error(spec, &e))?;
            let tree = JobTree::new().map_err(|e| {
                VelaError::of_class(
                    ErrorClass::InternalBug,
                    format!("could not create a job object: {e}"),
                )
            })?;
            tree.assign(&child).map_err(|e| {
                VelaError::of_class(
                    ErrorClass::InternalBug,
                    format!("could not track the process tree: {e}"),
                )
            })?;
            let tracked = self.registry.register(&spec.id, tree).map_err(|e| {
                VelaError::of_class(ErrorClass::InternalBug, format!("process id reuse: {e}"))
            })?;

            let stdout = child.stdout.take();
            let stderr = child.stderr.take();
            let used = AtomicUsize::new(0);
            let cap = self.max_output_bytes;
            let timeout = Duration::from_millis(spec.timeout_ms.get());

            let wait = async {
                match tokio::time::timeout(timeout, child.wait()).await {
                    Ok(status) => (status.ok(), false),
                    Err(_) => {
                        let _ = tracked.tree.terminate();
                        (child.wait().await.ok(), true)
                    }
                }
            };
            let read_out = async {
                if let Some(r) = stdout {
                    pump(
                        r,
                        OutputStream::Stdout,
                        self.redactor.clone(),
                        sink,
                        &used,
                        cap,
                    )
                    .await;
                }
            };
            let read_err = async {
                if let Some(r) = stderr {
                    pump(
                        r,
                        OutputStream::Stderr,
                        self.redactor.clone(),
                        sink,
                        &used,
                        cap,
                    )
                    .await;
                }
            };
            let ((status, timed_out), (), ()) = tokio::join!(wait, read_out, read_err);

            // The root has exited; make sure nothing it left behind survives the run.
            let _ = tracked.tree.terminate();
            let cancelled = tracked.was_cancelled();
            self.registry.remove(&spec.id);
            Ok(ProcessOutcome {
                exit_code: if timed_out || cancelled {
                    None
                } else {
                    status.and_then(|s| s.code())
                },
                timed_out,
                cancelled,
                duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            })
        })
    }

    fn kill_tree<'a>(&'a self, id: &'a str) -> PortFuture<'a, ()> {
        Box::pin(async move {
            self.registry
                .cancel(id, self.cancel_grace)
                .await
                .map_err(|e| {
                    VelaError::of_class(ErrorClass::InternalBug, format!("tree kill failed: {e}"))
                })
        })
    }
}

#[cfg(all(test, windows))]
mod tests;
