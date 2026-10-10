//! A sample persisted state machine and the harness self-test that runs against it.
//!
//! The machine has three states, `fresh`, `prepared`, and `committed`, persisted in one file with an atomic
//! write-then-rename. [`SampleStateMachine::run`] persists `prepared`, passes the [`FaultPoint::SelfTest`] fault point
//! (the window in which a crash leaves a half-done transition), and persists `committed`.
//! [`SampleStateMachine::recover`] is what a restarted process does with whatever it finds. It is the smallest
//! complete example of the pattern every persisted transition in Vela follows, and R03's crash matrix copies it.

use super::{HarnessError, io};
use std::path::{Path, PathBuf};
use vela_domain::fault::FaultPoint;

const STATE_FILE: &str = "state.txt";
const STATE_TMP_FILE: &str = "state.tmp";

/// The persisted states of the sample machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleState {
    /// Nothing has been persisted.
    Fresh,
    /// The transition has started and is not yet complete.
    Prepared,
    /// The transition is complete.
    Committed,
}

impl SampleState {
    /// The persisted spelling.
    pub const fn name(self) -> &'static str {
        match self {
            SampleState::Fresh => "fresh",
            SampleState::Prepared => "prepared",
            SampleState::Committed => "committed",
        }
    }

    /// Parses the persisted spelling.
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "fresh" => Some(SampleState::Fresh),
            "prepared" => Some(SampleState::Prepared),
            "committed" => Some(SampleState::Committed),
            _ => None,
        }
    }
}

/// What recovery did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    /// There was no persisted state: nothing to recover.
    NothingToRecover,
    /// The transition had started and was completed.
    RolledForward,
    /// The transition was already complete.
    AlreadyCommitted,
}

impl RecoveryAction {
    /// The reported spelling.
    pub const fn name(self) -> &'static str {
        match self {
            RecoveryAction::NothingToRecover => "nothing_to_recover",
            RecoveryAction::RolledForward => "rolled_forward",
            RecoveryAction::AlreadyCommitted => "already_committed",
        }
    }
}

/// The state recovery found and what it did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryOutcome {
    /// The state on disk when recovery began.
    pub found: SampleState,
    /// What recovery did about it.
    pub action: RecoveryAction,
}

/// The sample machine over a state directory.
#[derive(Debug, Clone)]
pub struct SampleStateMachine {
    dir: PathBuf,
}

impl SampleStateMachine {
    /// A machine persisting into `dir`, which must exist.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The persisted state (`Fresh` when nothing is persisted). An unreadable value is an error, never a guess.
    pub fn load(&self) -> Result<SampleState, HarnessError> {
        let path = self.dir.join(STATE_FILE);
        match std::fs::read_to_string(&path) {
            Ok(text) => SampleState::parse(&text).ok_or_else(|| {
                io(
                    format!("parse {}", path.display()),
                    std::io::Error::new(std::io::ErrorKind::InvalidData, text),
                )
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(SampleState::Fresh),
            Err(e) => Err(io(format!("read {}", path.display()), e)),
        }
    }

    fn persist(&self, state: SampleState) -> Result<(), HarnessError> {
        let tmp = self.dir.join(STATE_TMP_FILE);
        let target = self.dir.join(STATE_FILE);
        std::fs::write(&tmp, state.name().as_bytes())
            .map_err(|e| io(format!("write {}", tmp.display()), e))?;
        std::fs::rename(&tmp, &target).map_err(|e| io(format!("rename to {}", target.display()), e))
    }

    /// Runs the transition: persist `prepared`, pass the fault point, persist `committed`.
    pub fn run(&self) -> Result<(), HarnessError> {
        self.persist(SampleState::Prepared)?;
        vela_domain::fault_point!(FaultPoint::SelfTest);
        self.persist(SampleState::Committed)
    }

    /// What a restarted process does: look at the persisted state and finish a half-done transition. Idempotent.
    pub fn recover(&self) -> Result<RecoveryOutcome, HarnessError> {
        let found = self.load()?;
        let action = match found {
            SampleState::Fresh => RecoveryAction::NothingToRecover,
            SampleState::Prepared => {
                self.persist(SampleState::Committed)?;
                RecoveryAction::RolledForward
            }
            SampleState::Committed => RecoveryAction::AlreadyCommitted,
        };
        Ok(RecoveryOutcome { found, action })
    }

    /// The state directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        ChildRun, KillRestartHarness, Phase, StateSnapshot, child_context, parse_reports, report,
    };
    use super::*;
    use crate::fixture_repo::ScratchDir;
    use std::time::Duration;

    /// The exact libtest name of the child below.
    const CHILD: &str = "fault_harness::sample::tests::child_sample_state_machine";
    /// Selects a misbehaving child: `panic` fails before the fault point, `hang` never exits.
    const MODE_ENV: &str = "VELA_HARNESS_SAMPLE_MODE";

    /// The child process body. A normal test run has no harness context, so this returns at once.
    #[test]
    fn child_sample_state_machine() {
        let Some(context) = child_context() else {
            return;
        };
        match std::env::var(MODE_ENV).as_deref() {
            Ok("panic") => panic!("deliberate child failure before the fault point"),
            Ok("hang") => std::thread::sleep(Duration::from_secs(300)),
            _ => {}
        }
        let machine = SampleStateMachine::new(context.state_dir);
        match context.phase {
            Phase::Run => {
                machine.run().expect("run the transition");
                report("completed=true");
            }
            Phase::Restart => {
                let outcome = machine.recover().expect("recover");
                report(&format!("found={}", outcome.found.name()));
                report(&format!("action={}", outcome.action.name()));
            }
        }
    }

    #[test]
    fn the_fault_feature_is_compiled_in_through_this_crates_manifest() {
        // With the feature on, `fault_point!` is the aborting expansion; an armed child must not survive it.
        let harness = KillRestartHarness::new(CHILD).unwrap();
        let run = harness.launch(Some(FaultPoint::SelfTest)).unwrap();
        assert!(run.crashed(), "the armed child survived: {run:?}");
    }

    #[test]
    fn an_unarmed_run_completes_and_commits() {
        let harness = KillRestartHarness::new(CHILD).unwrap();
        let run = harness.launch(None).unwrap();
        assert!(run.success, "{run:?}");
        assert!(!run.crashed());
        assert_eq!(run.report_value("completed"), Some("true"));
        assert_eq!(
            harness.snapshot().unwrap().get(STATE_FILE),
            Some("committed")
        );
    }

    #[test]
    fn arm_kill_restart_reports_the_state_found() {
        let harness = KillRestartHarness::new(CHILD).unwrap();
        let report = harness.kill_and_restart(FaultPoint::SelfTest).unwrap();

        // The child was killed at the point: it never reported completion and the half-done state is on disk.
        assert_eq!(report.point, FaultPoint::SelfTest);
        assert_eq!(report.killed.armed, Some(FaultPoint::SelfTest));
        assert!(report.killed.crashed(), "{:?}", report.killed);
        assert_eq!(report.killed.report_value("completed"), None);
        assert_eq!(report.state_found.get(STATE_FILE), Some("prepared"));
        assert_eq!(report.state_found.names(), vec![STATE_FILE]);

        // The restarted process saw exactly that state and rolled it forward.
        assert!(report.restart.success, "{:?}", report.restart);
        assert_eq!(report.restart.phase, Phase::Restart);
        assert_eq!(report.restart.report_value("found"), Some("prepared"));
        assert_eq!(
            report.restart.report_value("action"),
            Some("rolled_forward")
        );
        assert_eq!(report.state_after.get(STATE_FILE), Some("committed"));

        // A second restart is idempotent.
        let again = harness.restart().unwrap();
        assert_eq!(again.report_value("found"), Some("committed"));
        assert_eq!(again.report_value("action"), Some("already_committed"));
    }

    #[test]
    fn a_fresh_state_directory_is_found_empty() {
        let harness = KillRestartHarness::new(CHILD).unwrap();
        let run = harness.restart().unwrap();
        assert_eq!(run.report_value("found"), Some("fresh"));
        assert_eq!(run.report_value("action"), Some("nothing_to_recover"));
        assert!(harness.snapshot().unwrap().is_empty());
    }

    #[test]
    fn a_child_that_fails_before_the_point_is_not_mistaken_for_a_crash() {
        let harness = KillRestartHarness::new(CHILD)
            .unwrap()
            .with_env(MODE_ENV, "panic");
        match harness.kill_and_restart(FaultPoint::SelfTest) {
            Err(HarnessError::NotKilled { point, run }) => {
                assert_eq!(point, FaultPoint::SelfTest);
                assert_eq!(run.exit_code, Some(101));
                assert!(!run.crashed());
            }
            other => panic!("expected NotKilled, got {other:?}"),
        }
    }

    #[test]
    fn a_hanging_child_is_killed_at_the_timeout() {
        let harness = KillRestartHarness::new(CHILD)
            .unwrap()
            .with_env(MODE_ENV, "hang")
            .with_timeout(Duration::from_secs(2));
        let started = std::time::Instant::now();
        match harness.launch(None) {
            Err(HarnessError::Timeout { phase, .. }) => assert_eq!(phase, Phase::Run),
            other => panic!("expected a timeout, got {other:?}"),
        }
        assert!(started.elapsed() < Duration::from_secs(60));
    }

    #[test]
    fn a_missing_child_test_is_an_error_not_a_pass() {
        let harness =
            KillRestartHarness::new("fault_harness::sample::tests::no_such_child").unwrap();
        assert!(matches!(
            harness.launch(None),
            Err(HarnessError::ChildNotFound { .. })
        ));
    }

    #[test]
    fn fault_points_are_armed_by_name_and_unknown_names_are_rejected() {
        assert_eq!(
            KillRestartHarness::point_named("self_test").unwrap(),
            FaultPoint::SelfTest
        );
        assert!(matches!(
            KillRestartHarness::point_named("no_such_point"),
            Err(HarnessError::UnknownFaultPoint(name)) if name == "no_such_point"
        ));
        // Every registered point can be named, so the matrix can drive them all.
        for point in FaultPoint::ALL {
            assert_eq!(
                KillRestartHarness::point_named(point.name()).unwrap(),
                *point
            );
        }
    }

    #[test]
    fn the_state_directory_refuses_a_synced_parent() {
        let parent = Path::new(r"C:\Users\Nobody\Dropbox\work");
        assert!(matches!(
            KillRestartHarness::new_in(CHILD, parent),
            Err(HarnessError::Scratch(_))
        ));
    }

    #[test]
    fn child_context_is_absent_in_a_normal_run() {
        assert_eq!(child_context(), None);
    }

    #[test]
    fn report_lines_are_found_after_libtest_prefixes() {
        let stdout = "running 1 test\ntest a::b ... VELA-HARNESS-REPORT: found=prepared\nVELA-HARNESS-REPORT: action=x  \nok\n";
        assert_eq!(parse_reports(stdout), vec!["found=prepared", "action=x"]);
    }

    #[test]
    fn crash_classification_separates_aborts_from_test_failures() {
        let run = |success: bool, exit_code: Option<i32>| ChildRun {
            phase: Phase::Run,
            armed: None,
            success,
            exit_code,
            stdout: String::new(),
            stderr: String::new(),
            reports: Vec::new(),
        };
        assert!(!run(true, Some(0)).crashed());
        assert!(!run(false, Some(101)).crashed(), "a failed test");
        assert!(run(false, None).crashed(), "killed by a signal");
        assert!(
            run(false, Some(-1_073_740_791)).crashed(),
            "Windows fast-fail abort"
        );
        assert!(run(false, Some(134)).crashed(), "SIGABRT as a shell status");
    }

    #[test]
    fn the_state_machine_persists_atomically_and_recovers_each_state() {
        let scratch = ScratchDir::create("sample").unwrap();
        let machine = SampleStateMachine::new(scratch.path());
        assert_eq!(machine.load().unwrap(), SampleState::Fresh);
        assert_eq!(
            machine.recover().unwrap(),
            RecoveryOutcome {
                found: SampleState::Fresh,
                action: RecoveryAction::NothingToRecover
            }
        );
        // No fault point is armed in this process, so run completes.
        machine.run().unwrap();
        assert_eq!(machine.load().unwrap(), SampleState::Committed);
        assert_eq!(
            StateSnapshot::capture(scratch.path()).unwrap().names(),
            vec![STATE_FILE],
            "no temporary file is left behind"
        );
        std::fs::write(scratch.path().join(STATE_FILE), "prepared").unwrap();
        assert_eq!(
            machine.recover().unwrap().action,
            RecoveryAction::RolledForward
        );
        assert_eq!(machine.load().unwrap(), SampleState::Committed);
    }

    #[test]
    fn a_corrupt_state_file_is_an_error_not_a_guess() {
        let scratch = ScratchDir::create("sample-corrupt").unwrap();
        std::fs::write(scratch.path().join(STATE_FILE), "garbage").unwrap();
        let machine = SampleStateMachine::new(scratch.path());
        assert!(machine.load().is_err());
        assert!(machine.recover().is_err());
        assert_eq!(machine.dir(), scratch.path());
    }
}
