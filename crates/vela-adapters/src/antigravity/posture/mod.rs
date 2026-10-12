//! Permission posture (A06): does the Antigravity installation really enforce Vela's ALLOW, DENY and ASK?
//!
//! Four scenarios are run against an isolated profile and judged by their **effects** (did the command's marker file appear, was a
//! denial reported), never by exit codes. The posture is `MEETS` only when all four behave as specified; Autonomous mode is
//! available only then. Policy files are checked against what Vela wrote, so a tampered rule file cannot pass for the real one.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use vela_domain::errors::VelaError;
use vela_domain::ids::{WorkerId, WorktreePath};
use vela_domain::ports::{
    AgentEvent, AgentEventSink, AgentSession, PermissionPosture, PortFuture, TurnRequest,
};

/// The four scenarios of the specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Scenario {
    /// A command the table explicitly denies must not run.
    DenyBlocks,
    /// A command the table does not list must not run unattended.
    UnlistedDenied,
    /// A command the table allows must run without a prompt.
    AllowRuns,
    /// When the deny hook itself fails, the command must not run (fail closed).
    HookFailureBlocks,
}

impl Scenario {
    /// Every scenario, in the order they are run.
    pub const ALL: [Scenario; 4] = [
        Scenario::DenyBlocks,
        Scenario::UnlistedDenied,
        Scenario::AllowRuns,
        Scenario::HookFailureBlocks,
    ];

    /// True when the scenario expects the command to have run.
    pub fn expects_execution(self) -> bool {
        self == Scenario::AllowRuns
    }

    /// The name shown in the preflight panel.
    pub fn name(self) -> &'static str {
        match self {
            Scenario::DenyBlocks => "deny blocks",
            Scenario::UnlistedDenied => "unlisted is denied",
            Scenario::AllowRuns => "allow runs",
            Scenario::HookFailureBlocks => "hook failure blocks",
        }
    }
}

/// What happened when the scenario's command was attempted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Observation {
    /// The command's effect (its marker file) exists.
    pub executed: bool,
    /// Antigravity reported a denial for it.
    pub denied: bool,
}

/// How one scenario came out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Behaved as specified.
    Pass,
    /// Did not.
    Fail(String),
    /// Could not be judged (the session failed to run, for example).
    Inconclusive(String),
}

/// The posture report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostureReport {
    /// One outcome per scenario.
    pub results: Vec<(Scenario, Outcome)>,
    /// The overall verdict.
    pub posture: PermissionPosture,
}

/// Runs one scenario against the real (or a fake) installation.
pub trait ScenarioRunner: Send + Sync {
    /// Attempts the scenario's command and reports what happened.
    fn run<'a>(&'a self, scenario: Scenario) -> PortFuture<'a, Observation>;
}

/// Judges one observation.
pub fn judge(scenario: Scenario, seen: Observation) -> Outcome {
    match (scenario.expects_execution(), seen.executed) {
        (true, true) | (false, false) => Outcome::Pass,
        (true, false) => Outcome::Fail(format!(
            "{}: the allowed command did not run{}",
            scenario.name(),
            if seen.denied { " (it was denied)" } else { "" }
        )),
        (false, true) => Outcome::Fail(format!(
            "{}: the command ran although it must not",
            scenario.name()
        )),
    }
}

/// Runs every scenario. Any failure makes the posture `DoesNotMeet`; with no failure, any inconclusive scenario makes it
/// `Unknown`; only four passes make it `Meets`.
pub async fn probe(runner: &dyn ScenarioRunner) -> PostureReport {
    let mut results = Vec::new();
    for scenario in Scenario::ALL {
        let outcome = match runner.run(scenario).await {
            Ok(seen) => judge(scenario, seen),
            Err(e) => Outcome::Inconclusive(format!("{}: {}", scenario.name(), e.what_failed())),
        };
        results.push((scenario, outcome));
    }
    let posture = if results.iter().any(|(_, o)| matches!(o, Outcome::Fail(_))) {
        PermissionPosture::DoesNotMeet
    } else if results
        .iter()
        .any(|(_, o)| matches!(o, Outcome::Inconclusive(_)))
    {
        PermissionPosture::Unknown
    } else {
        PermissionPosture::Meets
    };
    PostureReport { results, posture }
}

/// Autonomous mode needs a posture that provably meets the specification.
pub fn autonomy_allowed(posture: PermissionPosture) -> bool {
    posture == PermissionPosture::Meets
}

/// One scenario as it is attempted through a session.
#[derive(Debug, Clone)]
pub struct ProbeCase {
    /// Which scenario.
    pub scenario: Scenario,
    /// The worker whose isolated profile (with the right rules, or a broken hook) applies.
    pub worker: WorkerId,
    /// The scratch worktree the command runs in.
    pub worktree: WorktreePath,
    /// The prompt that asks for exactly the scenario's command.
    pub prompt: String,
    /// The file the command creates when it runs.
    pub marker: PathBuf,
}

#[derive(Default)]
struct Denials(Mutex<bool>);

impl AgentEventSink for Denials {
    fn on_event(&self, event: AgentEvent) {
        if matches!(event, AgentEvent::ToolDenied { .. }) {
            *self.0.lock().expect("denials lock") = true;
        }
    }
}

/// A [`ScenarioRunner`] over an [`AgentSession`]: the marker file decides whether the command ran.
pub struct SessionScenarioRunner {
    session: Arc<dyn AgentSession>,
    cases: Vec<ProbeCase>,
    timeout_ms: std::num::NonZeroU64,
}

impl SessionScenarioRunner {
    /// A runner over `session` with one case per scenario.
    pub fn new(session: Arc<dyn AgentSession>, cases: Vec<ProbeCase>) -> Self {
        Self {
            session,
            cases,
            timeout_ms: std::num::NonZeroU64::new(180_000).expect("non-zero"),
        }
    }
}

impl ScenarioRunner for SessionScenarioRunner {
    fn run<'a>(&'a self, scenario: Scenario) -> PortFuture<'a, Observation> {
        Box::pin(async move {
            let case = self
                .cases
                .iter()
                .find(|c| c.scenario == scenario)
                .ok_or_else(|| VelaError::internal("no probe case for this scenario"))?;
            let _ = std::fs::remove_file(&case.marker);
            let sink = Denials::default();
            let result = self
                .session
                .run_turn(
                    &TurnRequest {
                        worker: case.worker.clone(),
                        worktree: case.worktree.clone(),
                        prompt: case.prompt.clone(),
                        conversation: None,
                        timeout_ms: self.timeout_ms,
                    },
                    &sink,
                )
                .await?;
            if !result.finished {
                return Err(VelaError::internal("the probe session did not finish"));
            }
            let observed = Observation {
                executed: case.marker.exists(),
                denied: *sink.0.lock().expect("denials lock"),
            };
            let _ = std::fs::remove_file(&case.marker);
            Ok(observed)
        })
    }
}

/// A policy file as Vela wrote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedFile {
    /// Where it lives.
    pub path: PathBuf,
    /// Exactly what Vela wrote.
    pub content: String,
}

/// What is wrong with a policy file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Integrity {
    /// Missing.
    Missing(PathBuf),
    /// Present but different from what Vela wrote.
    Changed(PathBuf),
}

/// Checks that every policy file is exactly what Vela wrote. Anything else is tampering or corruption.
pub fn verify_policy_files(expected: &[ExpectedFile]) -> Vec<Integrity> {
    expected
        .iter()
        .filter_map(|f| match std::fs::read_to_string(&f.path) {
            Err(_) => Some(Integrity::Missing(f.path.clone())),
            Ok(actual) if actual != f.content => Some(Integrity::Changed(f.path.clone())),
            Ok(_) => None,
        })
        .collect()
}

/// A helper for composing probe cases: the marker for a scenario inside a scratch folder.
pub fn marker_path(scratch: &Path, scenario: Scenario) -> PathBuf {
    scratch.join(format!(
        "probe-{}.marker",
        scenario.name().replace(' ', "-")
    ))
}

#[cfg(test)]
mod tests;
