//! PreToolUse hook helper (ADR-018). Bootstrap stub (F01): the hook protocol is not implemented yet.
//!
//! It fails closed: a hook that cannot decide must never look like an allow, so it exits non-zero.

use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!("vela-hook: not implemented (F01 bootstrap stub); failing closed");
    ExitCode::FAILURE
}
