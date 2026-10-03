//! Fake approval window (test harness for the UIA adapter). Bootstrap stub (F01): the window is
//! implemented by the ticket that owns it. It exits non-zero so it can never be mistaken for a
//! working harness.

use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!("fake-approval-window: not implemented (F01 bootstrap stub)");
    ExitCode::FAILURE
}
