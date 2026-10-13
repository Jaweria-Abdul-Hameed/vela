//! Fault-injection mechanism (F02; `SHARED_SURFACE_PROTOCOL.md` section 6).
//!
//! The mechanism: a [`FaultPoint`] enum, the [`fault_point!`](crate::fault_point) macro, and an environment-variable
//! armed abort hook. The macro compiles to nothing unless the `fault-injection` feature is enabled, so production builds
//! carry no injection code at their call sites. The kill-and-restart harness is F03; each ticket that owns a
//! non-idempotent external action or a persisted transition adds its own variants (one per line, with a stable name)
//! as it implements them, and R03 owns the crash matrix that must cover every variant.
//!
//! Arming: set [`ARMED_ENV_VAR`] to the point's [`FaultPoint::name`] in the process environment. When execution reaches
//! that point the process aborts immediately, with no unwinding and no destructors, which is what a crash looks like.

#![warn(missing_docs)]

/// Environment variable that arms one fault point by name.
pub const ARMED_ENV_VAR: &str = "VELA_FAULT_POINT";

/// A named place where a crash can be injected.
///
/// Owning tickets register their points by adding a variant here and a matching arm in [`FaultPoint::name`] and
/// [`FaultPoint::ALL`]; the completeness test below fails if a variant is missing from either.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FaultPoint {
    /// Used by the mechanism's own tests and by the F03 harness self-test; no production code places it.
    SelfTest,
    /// The state store writer, after a transaction's state and journal rows were written and before they commit (F06).
    PersistenceBeforeCommit,
    /// The state store writer, right after a transaction committed and before its events are published (F06).
    PersistenceAfterCommit,
    /// A checkpoint commit: the worktree is staged and verified, the commit has not been made yet (G03).
    GitCheckpointBeforeCommit,
    /// A checkpoint commit: the commit exists, nothing has recorded it yet (G03).
    GitCheckpointAfterCommit,
    /// A push: the remote has not been contacted for the push yet (G05).
    GitPushBefore,
    /// A push: the remote accepted it, nothing has recorded it yet (G05).
    GitPushAfter,
    /// Starting a build: the integration branch does not exist yet (S08).
    BuildBeforeBranch,
    /// Starting a build: the integration branch and worktree exist, nothing records them yet (S08).
    BuildAfterWorktree,
    /// Starting a build: everything is recorded, the run has not entered `RUNNING` yet (S08).
    BuildBeforeRunning,
    /// A worker: provisioned, the next state transition has not happened (S10).
    WorkerAfterProvisioning,
    /// A worker: the agent turn finished, validation has not run (S10).
    WorkerAfterImplementing,
    /// A worker: validation passed, the checkpoint has not been requested (S10).
    WorkerBeforeCheckpoint,
    /// A worker: the checkpoint exists in Git, nothing has recorded it (S10).
    WorkerAfterCheckpoint,
    /// The merge lane: the merge is recorded as started, Git has not merged yet (S12).
    LaneBeforeMerge,
    /// The merge lane: Git merged, integration validation has not run (S12).
    LaneAfterMerge,
    /// The merge lane: validation finished, the result is not recorded (S12).
    LaneAfterValidation,
    /// The merge lane: a failed merge was discarded, nothing records it (S12).
    LaneAfterDiscard,
    /// Finalization: validated, the final push has not happened (S14).
    FinalBeforePush,
    /// Finalization: pushed, nothing records it (S14).
    FinalAfterPush,
    /// Finalization: the promotion is recorded, the run is not completed (S14).
    FinalAfterPromotion,
    /// Finalization: worktree cleanup is in progress (S14).
    FinalDuringCleanup,
    /// The approval broker: a decision was persisted, nothing has been delivered (K01).
    ApprovalDecisionPersisted,
    /// The approval broker: a delivery was attempted, its result is not recorded (K01).
    ApprovalDeliveryAttempted,
    /// Stop All: the bounded wait for the merge lane expired (R08).
    StopBoundedWaitExpired,
    /// Stop All: the discard-versus-abandon decision for an unfinished merge or push was made, nothing recorded it (R08).
    StopDiscardDecision,
}

impl FaultPoint {
    /// Every registered point.
    pub const ALL: &'static [FaultPoint] = &[
        FaultPoint::SelfTest,
        FaultPoint::PersistenceBeforeCommit,
        FaultPoint::PersistenceAfterCommit,
        FaultPoint::GitCheckpointBeforeCommit,
        FaultPoint::GitCheckpointAfterCommit,
        FaultPoint::GitPushBefore,
        FaultPoint::GitPushAfter,
        FaultPoint::BuildBeforeBranch,
        FaultPoint::BuildAfterWorktree,
        FaultPoint::BuildBeforeRunning,
        FaultPoint::WorkerAfterProvisioning,
        FaultPoint::WorkerAfterImplementing,
        FaultPoint::WorkerBeforeCheckpoint,
        FaultPoint::WorkerAfterCheckpoint,
        FaultPoint::LaneBeforeMerge,
        FaultPoint::LaneAfterMerge,
        FaultPoint::LaneAfterValidation,
        FaultPoint::LaneAfterDiscard,
        FaultPoint::FinalBeforePush,
        FaultPoint::FinalAfterPush,
        FaultPoint::FinalAfterPromotion,
        FaultPoint::FinalDuringCleanup,
        FaultPoint::ApprovalDecisionPersisted,
        FaultPoint::ApprovalDeliveryAttempted,
        FaultPoint::StopBoundedWaitExpired,
        FaultPoint::StopDiscardDecision,
    ];

    /// The stable name used to arm the point.
    pub const fn name(self) -> &'static str {
        match self {
            FaultPoint::SelfTest => "self_test",
            FaultPoint::PersistenceBeforeCommit => "persistence_before_commit",
            FaultPoint::PersistenceAfterCommit => "persistence_after_commit",
            FaultPoint::GitCheckpointBeforeCommit => "git.checkpoint.before_commit",
            FaultPoint::GitCheckpointAfterCommit => "git.checkpoint.after_commit",
            FaultPoint::GitPushBefore => "git.push.before",
            FaultPoint::GitPushAfter => "git.push.after",
            FaultPoint::BuildBeforeBranch => "build.before_branch",
            FaultPoint::BuildAfterWorktree => "build.after_worktree",
            FaultPoint::BuildBeforeRunning => "build.before_running",
            FaultPoint::WorkerAfterProvisioning => "worker.after_provisioning",
            FaultPoint::WorkerAfterImplementing => "worker.after_implementing",
            FaultPoint::WorkerBeforeCheckpoint => "worker.before_checkpoint",
            FaultPoint::WorkerAfterCheckpoint => "worker.after_checkpoint",
            FaultPoint::LaneBeforeMerge => "lane.before_merge",
            FaultPoint::LaneAfterMerge => "lane.after_merge",
            FaultPoint::LaneAfterValidation => "lane.after_validation",
            FaultPoint::LaneAfterDiscard => "lane.after_discard",
            FaultPoint::FinalBeforePush => "final.before_push",
            FaultPoint::FinalAfterPush => "final.after_push",
            FaultPoint::FinalAfterPromotion => "final.after_promotion",
            FaultPoint::FinalDuringCleanup => "final.during_cleanup",
            FaultPoint::ApprovalDecisionPersisted => "approval.decision_persisted",
            FaultPoint::ApprovalDeliveryAttempted => "approval.delivery_attempted",
            FaultPoint::StopBoundedWaitExpired => "stop.bounded_wait_expired",
            FaultPoint::StopDiscardDecision => "stop.discard_decision",
        }
    }

    /// Looks a point up by its armed name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|p| p.name() == name)
    }
}

/// True when `armed` (the value of [`ARMED_ENV_VAR`], if set) names `point`.
pub fn is_armed(point: FaultPoint, armed: Option<&str>) -> bool {
    armed == Some(point.name())
}

/// Aborts the process when `point` is armed in the environment; otherwise does nothing.
///
/// This is the body behind [`fault_point!`](crate::fault_point) when the `fault-injection` feature is on. It is always
/// compiled so the abort behavior is tested in every build, but nothing calls it unless the feature is on.
pub fn hit(point: FaultPoint) {
    let armed = std::env::var(ARMED_ENV_VAR).ok();
    if is_armed(point, armed.as_deref()) {
        std::process::abort();
    }
}

/// The armed body of [`fault_point!`](crate::fault_point): aborts the process when the point is armed. It is always
/// compiled, so the macro path that runs when `fault-injection` is on is exercised by the tests of every build. Not for
/// direct use; place points with `fault_point!`.
#[doc(hidden)]
#[macro_export]
macro_rules! __fault_point_armed {
    ($point:expr) => {
        $crate::fault::hit($point)
    };
}

/// Marks a fault point. With the `fault-injection` feature on, aborts the process when the point is armed. With the
/// feature off it expands to `()`: no code is generated and the argument is not even evaluated.
#[cfg(feature = "fault-injection")]
#[macro_export]
macro_rules! fault_point {
    ($point:expr) => {
        $crate::__fault_point_armed!($point)
    };
}

/// Marks a fault point. With the `fault-injection` feature off (this build) it expands to `()`: no code is generated and
/// the argument is not even evaluated.
#[cfg(not(feature = "fault-injection"))]
#[macro_export]
macro_rules! fault_point {
    ($point:expr) => {
        ()
    };
}

#[cfg(test)]
mod feature_matrix;

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    const CHILD_ENV: &str = "VELA_FAULT_TEST_CHILD";

    #[test]
    fn every_point_has_a_unique_name_that_round_trips() {
        let mut names: Vec<&str> = FaultPoint::ALL.iter().map(|p| p.name()).collect();
        for point in FaultPoint::ALL {
            assert_eq!(FaultPoint::from_name(point.name()), Some(*point));
        }
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before, "duplicate fault point name");
        assert_eq!(FaultPoint::from_name("nope"), None);
    }

    #[test]
    fn a_point_is_armed_only_by_its_exact_name() {
        assert!(is_armed(FaultPoint::SelfTest, Some("self_test")));
        assert!(!is_armed(FaultPoint::SelfTest, Some("self_test2")));
        assert!(!is_armed(FaultPoint::SelfTest, Some("")));
        assert!(!is_armed(FaultPoint::SelfTest, None));
    }

    // Compile tests: with the feature off the macro is the unit value in a constant context (so it generates no runtime
    // code) and never evaluates its argument (a const-evaluated `panic!` would fail the build).
    #[cfg(not(feature = "fault-injection"))]
    const _: () = crate::fault_point!(FaultPoint::SelfTest);
    #[cfg(not(feature = "fault-injection"))]
    const _: () = crate::fault_point!(panic!(
        "a disabled fault point must not evaluate its argument"
    ));

    /// With the feature on, an unarmed point does nothing.
    #[cfg(feature = "fault-injection")]
    #[test]
    fn the_macro_is_a_no_op_for_an_unarmed_point_when_the_feature_is_on() {
        if std::env::var(ARMED_ENV_VAR).is_err() {
            crate::fault_point!(FaultPoint::SelfTest);
        }
    }

    /// Body of the child process used by the abort tests. A normal test run skips it.
    #[test]
    fn child_reaches_the_fault_point() {
        if std::env::var(CHILD_ENV).is_err() {
            return;
        }
        // The public macro when the feature is on; otherwise the armed macro body it forwards to, so the default build
        // still exercises the abort path.
        #[cfg(feature = "fault-injection")]
        crate::fault_point!(FaultPoint::SelfTest);
        #[cfg(not(feature = "fault-injection"))]
        crate::__fault_point_armed!(FaultPoint::SelfTest);
        println!("SURVIVED-THE-FAULT-POINT");
    }

    fn run_child(armed: Option<&str>) -> (bool, String) {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "fault::tests::child_reaches_the_fault_point",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD_ENV, "1")
            .env_remove(ARMED_ENV_VAR);
        if let Some(name) = armed {
            command.env(ARMED_ENV_VAR, name);
        }
        let output = command.output().expect("spawn the test binary");
        (
            output.status.success(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
        )
    }

    #[test]
    fn an_armed_point_aborts_the_process() {
        let (success, stdout) = run_child(Some(FaultPoint::SelfTest.name()));
        assert!(!success, "the armed child must not exit successfully");
        assert!(
            !stdout.contains("SURVIVED-THE-FAULT-POINT"),
            "the armed child ran past the fault point:\n{stdout}"
        );
    }

    #[test]
    fn an_unarmed_or_differently_armed_point_does_not_abort() {
        for armed in [None, Some("some_other_point")] {
            let (success, stdout) = run_child(armed);
            assert!(
                success,
                "the child must exit normally ({armed:?}):\n{stdout}"
            );
            assert!(stdout.contains("SURVIVED-THE-FAULT-POINT"), "{stdout}");
        }
    }
}
