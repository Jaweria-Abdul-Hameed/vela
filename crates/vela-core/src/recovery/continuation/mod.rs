//! Reboot and restart continuation (R07, ADR-012): reconciliation always runs first; then the persisted
//! `recovery_continuation` setting decides what happens to a run that was running. `ask` (the default) always pauses for the
//! user; `auto_safe` continues only when reconciliation found nothing at all, and work that depends on UI automation
//! additionally waits for an interactive session.

use vela_domain::state::PausedReason;

/// The persisted choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    /// Pause and wait for the user (default).
    Ask,
    /// Continue by itself, but only when it is provably safe.
    AutoSafe,
}

impl Policy {
    /// Reads the setting value; anything unrecognised is the safe default, `ask`.
    pub fn parse(value: &str) -> Self {
        if value == "auto_safe" {
            Self::AutoSafe
        } else {
            Self::Ask
        }
    }
}

/// Whether the run depends on UI automation and whether an interactive session exists right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InteractiveGate {
    /// Some worker's approvals are delivered by UI automation.
    pub needs: bool,
    /// The desktop is unlocked and attached.
    pub available: bool,
}

impl Default for InteractiveGate {
    fn default() -> Self {
        Self {
            needs: false,
            available: true,
        }
    }
}

/// What to do with the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Continuation {
    /// Carry on without asking.
    Resume,
    /// Leave the run paused for this reason.
    Pause(PausedReason),
}

/// The decision table. `clean` means reconciliation found no discrepancy and holds nothing back.
pub fn decide(policy: Policy, clean: bool, gate: InteractiveGate) -> Continuation {
    match (policy, clean) {
        (Policy::Ask, _) => Continuation::Pause(PausedReason::AwaitingRecoveryConfirmation),
        (Policy::AutoSafe, false) => Continuation::Pause(PausedReason::RecoveredAwaitingResume),
        (Policy::AutoSafe, true) if gate.needs && !gate.available => {
            Continuation::Pause(PausedReason::InteractiveSessionUnavailable)
        }
        (Policy::AutoSafe, true) => Continuation::Resume,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HERE: InteractiveGate = InteractiveGate {
        needs: true,
        available: true,
    };
    const AWAY: InteractiveGate = InteractiveGate {
        needs: true,
        available: false,
    };
    const NONE: InteractiveGate = InteractiveGate {
        needs: false,
        available: false,
    };

    #[test]
    fn ask_always_waits_for_the_user_whatever_else_is_true() {
        for clean in [true, false] {
            for gate in [HERE, AWAY, NONE] {
                assert_eq!(
                    decide(Policy::Ask, clean, gate),
                    Continuation::Pause(PausedReason::AwaitingRecoveryConfirmation)
                );
            }
        }
    }

    #[test]
    fn auto_safe_continues_only_after_a_clean_reconciliation() {
        assert_eq!(decide(Policy::AutoSafe, true, NONE), Continuation::Resume);
        assert_eq!(decide(Policy::AutoSafe, true, HERE), Continuation::Resume);
        assert_eq!(
            decide(Policy::AutoSafe, false, HERE),
            Continuation::Pause(PausedReason::RecoveredAwaitingResume)
        );
    }

    #[test]
    fn ui_automation_work_waits_for_an_interactive_session() {
        assert_eq!(
            decide(Policy::AutoSafe, true, AWAY),
            Continuation::Pause(PausedReason::InteractiveSessionUnavailable)
        );
        // A discrepancy outranks it: the user must look first.
        assert_eq!(
            decide(Policy::AutoSafe, false, AWAY),
            Continuation::Pause(PausedReason::RecoveredAwaitingResume)
        );
    }

    #[test]
    fn unknown_setting_values_mean_ask() {
        assert_eq!(Policy::parse("auto_safe"), Policy::AutoSafe);
        assert_eq!(Policy::parse("ask"), Policy::Ask);
        assert_eq!(Policy::parse("always"), Policy::Ask);
    }
}
