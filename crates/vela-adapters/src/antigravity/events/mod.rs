//! The Antigravity `stream-json` event parser (A02) and the turn interpreter.
//!
//! Vela never trusts the exit code or `result.status`: a headless command that needs a permission nobody can grant is
//! **soft-denied** with exit code 0, status `SUCCESS`, an empty response, and a tool step that still says `DONE`
//! (verified on agy 1.2.14, `tests/fixtures/transcripts`). The interpreter therefore combines the events, `stderr` and the
//! exit code, and reports what actually happened. Unknown events and fields are tolerated (the provider changes them),
//! malformed lines are counted and skipped, and a stream with no `result` event is *incomplete*, never a success.

mod interpret;
mod types;

pub use interpret::{
    DeniedAction, Executed, ToolStepReport, TurnClass, TurnInput, TurnOutcome, classify_text,
    interpret_turn,
};
pub use types::{
    InitEvent, ParseError, ResultEvent, ResultStatus, StepState, StepUpdate, StreamEvent,
    ToolError, ToolInfo, Usage, parse_line,
};

#[cfg(test)]
mod tests;
