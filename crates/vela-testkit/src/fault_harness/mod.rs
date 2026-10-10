//! Kill-and-restart fault harness (F03), built on the F02 `fault-injection` mechanism.
//!
//! A crash test names a `#[test]` function (the *child*) in its own test binary. The harness re-executes that binary
//! with [`vela_domain::fault::ARMED_ENV_VAR`] set to a named [`FaultPoint`]; when the child reaches
//! `fault_point!(that point)` it aborts, which is what a crash looks like (no unwinding, no destructors). The harness
//! then snapshots the state directory the dead child left behind, restarts the child against the same directory with
//! nothing armed, and reports what the restarted process found and what it did about it.
//!
//! The child side is one line: [`child_context`] returns `None` in a normal test run (so the child test is a no-op)
//! and the state directory and [`Phase`] when the harness started it. [`sample`] is a complete worked example, which is
//! also the harness's self-test.
//!
//! The `fault-injection` feature of `vela-domain` is switched on by this crate's own manifest, so any test binary that
//! links `vela-testkit` has live fault points. Owning tickets add [`FaultPoint`] variants in `vela-domain`; R03 owns the
//! matrix that runs this harness over every variant.

#![warn(missing_docs)]

pub mod sample;

#[cfg(test)]
mod feature_off;

use crate::fixture_repo::{FixtureError, ScratchDir};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use vela_domain::fault::{ARMED_ENV_VAR, FaultPoint};

/// Environment variable that tells a child where the shared state directory is.
pub const STATE_DIR_ENV: &str = "VELA_HARNESS_STATE_DIR";
/// Environment variable that tells a child which [`Phase`] it is in.
pub const PHASE_ENV: &str = "VELA_HARNESS_PHASE";
/// Prefix of the lines a child prints (via [`report`]) for the harness to collect.
pub const REPORT_PREFIX: &str = "VELA-HARNESS-REPORT: ";
/// `SIGABRT` as a shell reports it (128 + 6).
const ABORT_SHELL_STATUS: i32 = 134;
/// `STATUS_STACK_BUFFER_OVERRUN` (`0xC0000409`), the status `std::process::abort` produces on Windows.
const WINDOWS_FAST_FAIL_STATUS: i32 = 0xC000_0409_u32 as i32;

/// Which launch of the child this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// The first launch, possibly with a fault point armed.
    Run,
    /// The relaunch after a crash, with nothing armed.
    Restart,
}

impl Phase {
    /// The value carried in [`PHASE_ENV`].
    pub const fn as_str(self) -> &'static str {
        match self {
            Phase::Run => "run",
            Phase::Restart => "restart",
        }
    }

    /// Parses a [`PHASE_ENV`] value.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "run" => Some(Phase::Run),
            "restart" => Some(Phase::Restart),
            _ => None,
        }
    }
}

/// What a child learns from the harness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildContext {
    /// The state directory shared by every launch of the child.
    pub state_dir: PathBuf,
    /// Which launch this is.
    pub phase: Phase,
}

/// The child side: `Some` when the harness launched this process, `None` in a normal test run.
pub fn child_context() -> Option<ChildContext> {
    let state_dir = std::env::var_os(STATE_DIR_ENV)?;
    let phase = Phase::parse(&std::env::var(PHASE_ENV).ok()?)?;
    Some(ChildContext {
        state_dir: PathBuf::from(state_dir),
        phase,
    })
}

/// The child side: prints one line for the harness to collect into [`ChildRun::reports`].
pub fn report(line: &str) {
    println!("{REPORT_PREFIX}{line}");
}

/// Collects the report lines from child output. A line may carry other text before the prefix (libtest prints
/// `test name ... ` before a `--nocapture` test's own output).
pub fn parse_reports(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter_map(|line| {
            line.find(REPORT_PREFIX)
                .map(|at| line[at + REPORT_PREFIX.len()..].trim_end().to_owned())
        })
        .collect()
}

/// Why the harness could not complete a scenario.
#[derive(Debug)]
pub enum HarnessError {
    /// The state directory could not be created (including a synced-folder refusal).
    Scratch(FixtureError),
    /// A filesystem or process-spawn failure.
    Io {
        /// What was being done.
        context: String,
        /// The underlying error.
        source: std::io::Error,
    },
    /// A fault point name that no [`FaultPoint`] has.
    UnknownFaultPoint(String),
    /// The child filter matched no test (libtest ran 0 tests), so nothing was exercised.
    ChildNotFound {
        /// The exact test name that was requested.
        test_name: String,
        /// What the child printed.
        stdout: String,
    },
    /// The child did not exit in time and was killed.
    Timeout {
        /// The limit that was exceeded.
        after: Duration,
        /// Which launch hung.
        phase: Phase,
        /// What the child had written to standard output when it was killed.
        stdout: String,
        /// What the child had written to standard error when it was killed.
        stderr: String,
    },
    /// The armed child did not crash: it completed or failed some other way, so the fault point was never reached.
    NotKilled {
        /// The armed point.
        point: FaultPoint,
        /// The run that was supposed to crash.
        run: Box<ChildRun>,
    },
    /// The restarted child failed.
    RestartFailed {
        /// The restart run.
        run: Box<ChildRun>,
    },
}

impl fmt::Display for HarnessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HarnessError::Scratch(e) => write!(f, "state directory: {e}"),
            HarnessError::Io { context, source } => write!(f, "{context}: {source}"),
            HarnessError::UnknownFaultPoint(name) => write!(f, "unknown fault point {name:?}"),
            HarnessError::ChildNotFound { test_name, .. } => {
                write!(
                    f,
                    "the child test {test_name:?} was not found in this test binary"
                )
            }
            HarnessError::Timeout {
                after,
                phase,
                stdout,
                stderr,
            } => {
                write!(
                    f,
                    "the {} child did not exit within {after:?}; stdout:
{stdout}
stderr:
{stderr}",
                    phase.as_str()
                )
            }
            HarnessError::NotKilled { point, run } => write!(
                f,
                "armed {:?} but the child did not crash (exit {:?}); stdout:\n{}",
                point.name(),
                run.exit_code,
                run.stdout
            ),
            HarnessError::RestartFailed { run } => write!(
                f,
                "the restarted child failed (exit {:?}); stdout:\n{}\nstderr:\n{}",
                run.exit_code, run.stdout, run.stderr
            ),
        }
    }
}

impl std::error::Error for HarnessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            HarnessError::Scratch(e) => Some(e),
            HarnessError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

fn io(context: String, source: std::io::Error) -> HarnessError {
    HarnessError::Io { context, source }
}

/// What one launch of the child did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildRun {
    /// Which launch this was.
    pub phase: Phase,
    /// The point that was armed, if any.
    pub armed: Option<FaultPoint>,
    /// True when the process exited with status zero.
    pub success: bool,
    /// The exit code (`None` when a signal ended the process).
    pub exit_code: Option<i32>,
    /// Everything the child wrote to standard output.
    pub stdout: String,
    /// Everything the child wrote to standard error.
    pub stderr: String,
    /// The lines the child sent through [`report`].
    pub reports: Vec<String>,
}

impl ChildRun {
    /// True when the process ended the way `std::process::abort` ends it: killed by a signal (no exit code),
    /// `SIGABRT` seen as a shell status (134), or the Windows fast-fail status `0xC0000409`. That is what an armed
    /// fault point produces. A failed test (101), an ordinary non-zero exit, or another fatal status (an access
    /// violation, a missing DLL) is deliberately not a crash, so an unrelated early death is never reported as a kill
    /// at the armed point.
    pub fn crashed(&self) -> bool {
        !self.success
            && match self.exit_code {
                None => true,
                Some(code) => code == ABORT_SHELL_STATUS || code == WINDOWS_FAST_FAIL_STATUS,
            }
    }

    /// The value of a `key=value` report line.
    pub fn report_value(&self, key: &str) -> Option<&str> {
        let prefix = format!("{key}=");
        self.reports
            .iter()
            .find_map(|line| line.strip_prefix(prefix.as_str()))
    }
}

/// The files in a state directory: relative path (forward slashes) to contents (lossy UTF-8).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StateSnapshot {
    files: BTreeMap<String, String>,
}

impl StateSnapshot {
    /// Reads every file under `dir` (recursively). A missing directory is an empty snapshot.
    pub fn capture(dir: &Path) -> Result<Self, HarnessError> {
        let mut files = BTreeMap::new();
        if dir.exists() {
            collect(dir, dir, &mut files)?;
        }
        Ok(Self { files })
    }

    /// The contents of one file.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.files.get(name).map(String::as_str)
    }

    /// The file names, sorted.
    pub fn names(&self) -> Vec<&str> {
        self.files.keys().map(String::as_str).collect()
    }

    /// True when the directory held no files.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

fn collect(
    root: &Path,
    dir: &Path,
    files: &mut BTreeMap<String, String>,
) -> Result<(), HarnessError> {
    let entries = std::fs::read_dir(dir).map_err(|e| io(format!("read {}", dir.display()), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| io(format!("read {}", dir.display()), e))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|e| io(format!("stat {}", path.display()), e))?;
        if file_type.is_dir() {
            collect(root, &path, files)?;
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let bytes =
                std::fs::read(&path).map_err(|e| io(format!("read {}", path.display()), e))?;
            files.insert(relative, String::from_utf8_lossy(&bytes).into_owned());
        }
    }
    Ok(())
}

/// The outcome of [`KillRestartHarness::kill_and_restart`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KillRestartReport {
    /// The point that was armed.
    pub point: FaultPoint,
    /// The first launch, which crashed.
    pub killed: ChildRun,
    /// The state directory as the dead child left it, before the restart.
    pub state_found: StateSnapshot,
    /// The restarted launch, which ran to completion.
    pub restart: ChildRun,
    /// The state directory after the restart.
    pub state_after: StateSnapshot,
}

static LOG_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Re-executes a child test with a fault point armed, then restarts it against the same state directory.
#[derive(Debug)]
pub struct KillRestartHarness {
    test_name: String,
    scratch: ScratchDir,
    state_dir: PathBuf,
    logs_dir: PathBuf,
    timeout: Duration,
    extra_env: Vec<(String, String)>,
}

impl KillRestartHarness {
    /// Default limit for one launch.
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

    /// Creates a harness for the child test `test_name` (the exact libtest name, for example
    /// `fault_harness::sample::tests::child_sample_state_machine`) with a fresh state directory in the OS temp
    /// directory, outside synced folders.
    pub fn new(test_name: &str) -> Result<Self, HarnessError> {
        let scratch = ScratchDir::create("fault-harness").map_err(HarnessError::Scratch)?;
        Self::from_scratch(test_name, scratch)
    }

    /// Like [`KillRestartHarness::new`], but under `parent` (which must exist). The synced-folder guard applies.
    pub fn new_in(test_name: &str, parent: &Path) -> Result<Self, HarnessError> {
        let scratch =
            ScratchDir::create_in(parent, "fault-harness").map_err(HarnessError::Scratch)?;
        Self::from_scratch(test_name, scratch)
    }

    fn from_scratch(test_name: &str, scratch: ScratchDir) -> Result<Self, HarnessError> {
        let state_dir = scratch.path().join("state");
        let logs_dir = scratch.path().join("logs");
        for dir in [&state_dir, &logs_dir] {
            std::fs::create_dir(dir).map_err(|e| io(format!("create {}", dir.display()), e))?;
        }
        Ok(Self {
            test_name: test_name.to_owned(),
            scratch,
            state_dir,
            logs_dir,
            timeout: Self::DEFAULT_TIMEOUT,
            extra_env: Vec::new(),
        })
    }

    /// Replaces the per-launch time limit.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Adds an environment variable passed to every launch (a child can use it to select a behavior).
    pub fn with_env(mut self, key: &str, value: &str) -> Self {
        self.extra_env.push((key.to_owned(), value.to_owned()));
        self
    }

    /// The state directory shared by every launch.
    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    /// The scratch directory that holds the state and the captured logs.
    pub fn root(&self) -> &Path {
        self.scratch.path()
    }

    /// The current contents of the state directory.
    pub fn snapshot(&self) -> Result<StateSnapshot, HarnessError> {
        StateSnapshot::capture(&self.state_dir)
    }

    /// Resolves a fault point by its armed name.
    pub fn point_named(name: &str) -> Result<FaultPoint, HarnessError> {
        FaultPoint::from_name(name).ok_or_else(|| HarnessError::UnknownFaultPoint(name.to_owned()))
    }

    /// Launches the child in [`Phase::Run`] with `point` armed (or nothing armed for `None`) and waits for it.
    pub fn launch(&self, point: Option<FaultPoint>) -> Result<ChildRun, HarnessError> {
        self.spawn(Phase::Run, point)
    }

    /// Launches the child in [`Phase::Restart`] with nothing armed and waits for it.
    pub fn restart(&self) -> Result<ChildRun, HarnessError> {
        self.spawn(Phase::Restart, None)
    }

    /// Arms `point`, launches the child, requires that it crashed, snapshots the state it left, restarts the child,
    /// requires that it succeeded, and reports both launches and both snapshots.
    pub fn kill_and_restart(&self, point: FaultPoint) -> Result<KillRestartReport, HarnessError> {
        let killed = self.launch(Some(point))?;
        if !killed.crashed() {
            return Err(HarnessError::NotKilled {
                point,
                run: Box::new(killed),
            });
        }
        let state_found = self.snapshot()?;
        let restart = self.restart()?;
        if !restart.success {
            return Err(HarnessError::RestartFailed {
                run: Box::new(restart),
            });
        }
        let state_after = self.snapshot()?;
        Ok(KillRestartReport {
            point,
            killed,
            state_found,
            restart,
            state_after,
        })
    }

    fn spawn(&self, phase: Phase, armed: Option<FaultPoint>) -> Result<ChildRun, HarnessError> {
        let exe =
            std::env::current_exe().map_err(|e| io("locate the test binary".to_owned(), e))?;
        let id = LOG_COUNTER.fetch_add(1, Ordering::Relaxed);
        let stdout_path = self.logs_dir.join(format!("{id}.stdout"));
        let stderr_path = self.logs_dir.join(format!("{id}.stderr"));
        let stdout_file = std::fs::File::create(&stdout_path)
            .map_err(|e| io(format!("create {}", stdout_path.display()), e))?;
        let stderr_file = std::fs::File::create(&stderr_path)
            .map_err(|e| io(format!("create {}", stderr_path.display()), e))?;

        let mut command = Command::new(exe);
        command
            .args([
                "--exact",
                self.test_name.as_str(),
                "--nocapture",
                "--test-threads=1",
            ])
            .env(STATE_DIR_ENV, &self.state_dir)
            .env(PHASE_ENV, phase.as_str())
            .env_remove(ARMED_ENV_VAR)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout_file))
            .stderr(Stdio::from(stderr_file));
        for (key, value) in &self.extra_env {
            command.env(key, value);
        }
        if let Some(point) = armed {
            command.env(ARMED_ENV_VAR, point.name());
        }
        let mut child = command
            .spawn()
            .map_err(|e| io(format!("spawn the {} child", phase.as_str()), e))?;

        let started = Instant::now();
        let status = loop {
            match child
                .try_wait()
                .map_err(|e| io("wait for the child".to_owned(), e))?
            {
                Some(status) => break status,
                None if started.elapsed() >= self.timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(HarnessError::Timeout {
                        after: self.timeout,
                        phase,
                        stdout: read_lossy(&stdout_path).unwrap_or_default(),
                        stderr: read_lossy(&stderr_path).unwrap_or_default(),
                    });
                }
                None => std::thread::sleep(Duration::from_millis(20)),
            }
        };

        let stdout = read_lossy(&stdout_path)?;
        let stderr = read_lossy(&stderr_path)?;
        if stdout.lines().any(|line| line.trim() == "running 0 tests") {
            return Err(HarnessError::ChildNotFound {
                test_name: self.test_name.clone(),
                stdout,
            });
        }
        Ok(ChildRun {
            phase,
            armed,
            success: status.success(),
            exit_code: status.code(),
            reports: parse_reports(&stdout),
            stdout,
            stderr,
        })
    }
}

fn read_lossy(path: &Path) -> Result<String, HarnessError> {
    let bytes = std::fs::read(path).map_err(|e| io(format!("read {}", path.display()), e))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
