//! A fake `agy` (TK1): a [`ProcessRunner`] that replays the recorded and synthetic transcripts in
//! `tests/fixtures/transcripts` instead of starting the real Antigravity CLI, and answers `--version`, `--help` and `models`
//! from recordings of the real CLI (agy 1.3.3, which the CLI updated itself to during the recording session; the earlier transcripts were captured on 1.2.14 with identical behavior). It is a fake at the process port, like [`crate::fake_gh::FakeGh`], so the
//! adapter code runs unchanged against it; the real-environment suite (A15) covers real spawning.
//!
//! It never reads the user's home directory, settings or credentials: the only files it reads are the fixtures, found
//! relative to this crate. The version of the CLI that produced the recordings is [`RECORDED_AGY_VERSION`].

use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::sync::Mutex;

use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::ports::{
    CommandSpec, OutputChunk, OutputSink, OutputStream, PortFuture, ProcessOutcome, ProcessRunner,
    ready,
};

/// The agy version the recordings were made with.
pub const RECORDED_AGY_VERSION: &str = "1.3.3";

/// What the next `agy -p ...` invocation does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scenario {
    /// Replay the named transcript from the fixtures (`success-pong`, `soft-denial`, `hook-deny`, ...).
    Replay(String),
    /// The process never finishes: the runner reports a timeout and no output.
    Hang,
}

/// One recorded invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgyCall {
    /// The arguments, without the program name.
    pub args: Vec<String>,
    /// The working directory the process was started in.
    pub cwd: String,
    /// The names (never the values) of the environment variables passed.
    pub env_names: Vec<String>,
    /// The id the caller gave the process.
    pub process_id: String,
}

#[derive(Debug, Default)]
struct State {
    queue: VecDeque<Scenario>,
    calls: Vec<AgyCall>,
    killed: Vec<String>,
    installed: bool,
}

/// The fake `agy`.
#[derive(Debug)]
pub struct FakeAgy {
    state: Mutex<State>,
    fixtures: PathBuf,
}

impl Default for FakeAgy {
    fn default() -> Self {
        Self::new()
    }
}

fn transcripts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/transcripts")
}

impl FakeAgy {
    /// Installed, with no scenarios queued (a turn with an empty queue replays `success-pong`).
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State {
                installed: true,
                ..State::default()
            }),
            fixtures: transcripts_dir(),
        }
    }

    /// Queues the scenario for the next prompt invocation.
    pub fn then(&self, scenario: Scenario) -> &Self {
        self.state
            .lock()
            .expect("agy lock")
            .queue
            .push_back(scenario);
        self
    }

    /// Queues a replay of the named transcript.
    pub fn then_replay(&self, transcript: &str) -> &Self {
        self.then(Scenario::Replay(transcript.to_owned()))
    }

    /// Makes `agy` absent (spawning it fails with "not found").
    pub fn set_installed(&self, installed: bool) {
        self.state.lock().expect("agy lock").installed = installed;
    }

    /// Every invocation, in order.
    pub fn calls(&self) -> Vec<AgyCall> {
        self.state.lock().expect("agy lock").calls.clone()
    }

    /// Process ids passed to `kill_tree`.
    pub fn killed(&self) -> Vec<String> {
        self.state.lock().expect("agy lock").killed.clone()
    }

    fn fixture(&self, name: &str) -> Result<String, VelaError> {
        std::fs::read_to_string(self.fixtures.join(name)).map_err(|e| {
            VelaError::internal(format!(
                "the fake agy could not read the fixture {name}: {e}"
            ))
        })
    }

    /// The transcript names the fixtures define (from the manifest), with their recorded exit codes.
    pub fn transcript_exit_codes(&self) -> Result<BTreeMap<String, i32>, VelaError> {
        let manifest: serde_json::Value = serde_json::from_str(&self.fixture("manifest.json")?)
            .map_err(|e| VelaError::internal(e.to_string()))?;
        Ok(manifest["transcripts"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|t| {
                        Some((
                            t["file"].as_str()?.to_owned(),
                            i32::try_from(t["exitCode"].as_i64()?).ok()?,
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    fn handle(
        &self,
        spec: &CommandSpec,
        sink: &dyn OutputSink,
    ) -> Result<ProcessOutcome, VelaError> {
        let call = AgyCall {
            args: spec.args.clone(),
            cwd: spec.cwd.as_str().to_owned(),
            env_names: spec.env.iter().map(|(k, _)| k.clone()).collect(),
            process_id: spec.id.clone(),
        };
        let scenario = {
            let mut s = self.state.lock().expect("agy lock");
            if !s.installed {
                s.calls.push(call);
                return Err(VelaError::of_class(
                    ErrorClass::UserActionRequired,
                    "could not start `agy`: NotFound",
                ));
            }
            s.calls.push(call);
            None::<Scenario>.or_else(|| {
                let prompting = spec.args.iter().any(|a| a == "-p" || a == "--print");
                if prompting {
                    s.queue
                        .pop_front()
                        .or(Some(Scenario::Replay("success-pong".to_owned())))
                } else {
                    None
                }
            })
        };
        let emit = |stream: OutputStream, text: &str| {
            if !text.is_empty() {
                sink.on_output(OutputChunk {
                    stream,
                    text: text.to_owned(),
                });
            }
        };
        let done = |exit_code: Option<i32>, timed_out: bool| ProcessOutcome {
            exit_code,
            timed_out,
            cancelled: false,
            duration_ms: 1,
        };
        let first = spec.args.first().map(String::as_str);
        match (first, scenario) {
            (Some("--version"), _) => {
                emit(OutputStream::Stdout, &self.fixture("agy-version.txt")?);
                Ok(done(Some(0), false))
            }
            (Some("--help"), _) => {
                emit(OutputStream::Stdout, &self.fixture("agy-help.txt")?);
                Ok(done(Some(0), false))
            }
            (Some("models"), _) => {
                emit(OutputStream::Stdout, &self.fixture("agy-models.txt")?);
                Ok(done(Some(0), false))
            }
            (_, Some(Scenario::Hang)) => Ok(done(None, true)),
            (_, Some(Scenario::Replay(name))) => {
                let mut stdout = self.fixture(&format!("{name}.jsonl"))?;
                let stderr = self.fixture(&format!("{name}.stderr.txt"))?;
                let exit = *self
                    .transcript_exit_codes()?
                    .get(&name)
                    .ok_or_else(|| VelaError::internal(format!("no transcript named {name}")))?;
                // A resume keeps the conversation it was asked to resume.
                if let Some(at) = spec.args.iter().position(|a| a == "--conversation")
                    && let (Some(wanted), Some(recorded)) =
                        (spec.args.get(at + 1), recorded_conversation(&stdout))
                {
                    stdout = stdout.replace(&recorded, wanted);
                }
                for line in stdout.split_inclusive('\n') {
                    emit(OutputStream::Stdout, line);
                }
                emit(OutputStream::Stderr, &stderr);
                Ok(done(Some(exit), false))
            }
            _ => {
                emit(OutputStream::Stderr, "unknown command\n");
                Ok(done(Some(2), false))
            }
        }
    }
}

/// The conversation id of the first `init` event in a transcript.
fn recorded_conversation(stdout: &str) -> Option<String> {
    let first = stdout.lines().next()?;
    let value: serde_json::Value = serde_json::from_str(first).ok()?;
    value.get("conversation_id")?.as_str().map(str::to_owned)
}

impl ProcessRunner for FakeAgy {
    fn run<'a>(
        &'a self,
        spec: &'a CommandSpec,
        sink: &'a dyn OutputSink,
    ) -> PortFuture<'a, ProcessOutcome> {
        if spec.program != "agy" {
            return ready(Err(VelaError::of_class(
                ErrorClass::UserActionRequired,
                format!("could not start `{}`: NotFound", spec.program),
            )));
        }
        ready(self.handle(spec, sink))
    }

    fn kill_tree<'a>(&'a self, id: &'a str) -> PortFuture<'a, ()> {
        self.state
            .lock()
            .expect("agy lock")
            .killed
            .push(id.to_owned());
        ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;
    use std::sync::Mutex;

    use vela_domain::ids::WorktreePath;

    use super::*;

    #[derive(Default)]
    struct Collect(Mutex<(String, String)>);

    impl OutputSink for Collect {
        fn on_output(&self, chunk: OutputChunk) {
            let mut g = self.0.lock().unwrap();
            match chunk.stream {
                OutputStream::Stdout => g.0.push_str(&chunk.text),
                OutputStream::Stderr => g.1.push_str(&chunk.text),
            }
        }
    }

    fn spec(args: &[&str]) -> CommandSpec {
        CommandSpec {
            id: "agy-1".into(),
            program: "agy".into(),
            args: args.iter().map(|s| (*s).to_owned()).collect(),
            cwd: WorktreePath::new("C:/work/wt").unwrap(),
            env: vec![("PATH".into(), "x".into())],
            timeout_ms: NonZeroU64::new(1000).unwrap(),
        }
    }

    #[tokio::test]
    async fn answers_version_help_and_models_from_the_recordings() {
        let fake = FakeAgy::new();
        let sink = Collect::default();
        fake.run(&spec(&["--version"]), &sink).await.unwrap();
        assert_eq!(sink.0.lock().unwrap().0.trim(), RECORDED_AGY_VERSION);
        let sink = Collect::default();
        fake.run(&spec(&["--help"]), &sink).await.unwrap();
        assert!(sink.0.lock().unwrap().0.contains("--output-format"));
        let sink = Collect::default();
        fake.run(&spec(&["models"]), &sink).await.unwrap();
        assert!(sink.0.lock().unwrap().0.contains("gemini"));
    }

    #[tokio::test]
    async fn replays_queued_scenarios_in_order_and_records_calls_without_env_values() {
        let fake = FakeAgy::new();
        fake.then_replay("soft-denial").then(Scenario::Hang);
        let sink = Collect::default();
        let out = fake
            .run(&spec(&["-p", "x", "--output-format", "stream-json"]), &sink)
            .await
            .unwrap();
        assert_eq!(out.exit_code, Some(0));
        assert!(sink.0.lock().unwrap().0.contains("denied_actions"));
        assert!(sink.0.lock().unwrap().1.contains("auto-denied"));
        let sink = Collect::default();
        let hung = fake.run(&spec(&["-p", "y"]), &sink).await.unwrap();
        assert!(hung.timed_out && hung.exit_code.is_none());
        let sink = Collect::default();
        fake.run(&spec(&["-p", "z"]), &sink).await.unwrap();
        assert!(
            sink.0.lock().unwrap().0.contains("PONG"),
            "an empty queue replays success-pong"
        );
        let calls = fake.calls();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].env_names, vec!["PATH".to_owned()]);
        assert_eq!(calls[0].cwd, "C:/work/wt");
    }

    #[tokio::test]
    async fn a_resume_keeps_the_requested_conversation_and_a_missing_agy_is_a_typed_error() {
        let fake = FakeAgy::new();
        fake.then_replay("resume-pong");
        let sink = Collect::default();
        fake.run(&spec(&["-p", "x", "--conversation", "abc-123"]), &sink)
            .await
            .unwrap();
        let out = sink.0.lock().unwrap().0.clone();
        assert!(out.contains("abc-123") && !out.contains("4f7475be"));
        fake.set_installed(false);
        let err = fake
            .run(&spec(&["--version"]), &Collect::default())
            .await
            .unwrap_err();
        assert_eq!(err.class(), ErrorClass::UserActionRequired);
    }

    #[test]
    fn the_fake_reads_only_fixtures_never_the_home_directory_or_the_environment() {
        let source = include_str!("mod.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        for banned in [
            "std::env::var",
            "home_dir",
            "USERPROFILE",
            "APPDATA",
            ".gemini",
            "std::net",
        ] {
            assert!(!production.contains(banned), "uses {banned}");
        }
    }
}
