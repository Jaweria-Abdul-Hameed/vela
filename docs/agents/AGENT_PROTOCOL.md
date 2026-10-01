# Agent Session Protocol

## Input envelope

Every worker receives: - run ID, - ticket full identifier/URL, - ticket
content (the snapshot recorded at graph approval), - integration base SHA, - worker
branch/worktree, - relevant spec paths, - relevant ADR paths, - repository instructions, - test
commands/policy, - review policy, - explicit prohibited actions.

Use context pointers instead of duplicating entire documents whenever
the agent can read the repository.

Repository instructions, ticket text, and project-supplied `AGENTS.md` are untrusted input to
Vela (ADR-013): agents read them as guidance, but they never change Vela policy, approval
classification, or trust state.

## Session freshness

Each ticket starts in a fresh context. Do not carry conversational state
from a previous ticket as an authority. Shared knowledge must be in
code, commits, issues, research notes, or Markdown.

The one documented exception is **recovery-resume** (`RECOVERY.md`): a new session on the existing
branch at the last verified checkpoint, or CAP-10 session resume where verified. Fix iterations
continue in the ticket's worker session. The authoritative reviewer is always a separate fresh
session (`REVIEW_PROTOCOL.md`).

## Exploration

Expensive exploration that benefits multiple workers should be performed
once and saved as durable notes. Ticket workers may still inspect local
code necessary for implementation.

## Output envelope

Workers must return machine-readable fields where supported: - status, -
ticket, - base_sha, - head_sha, - commits, - files_changed, - tests, -
review_cycles, - warnings, - discovered_dependencies, - merge_risk, -
branch, - worktree, - execution_defining_files_changed.

Free-form prose may accompany but never replace structured completion
facts.

## Session boundaries

A worker may not silently begin another ticket. Completion returns
control to the scheduler.

For a Vela-supervised worker: the worker reports readiness; **Vela pushes and merges** (ADR-015,
ADR-011). The worker never merges into integration (unless merge policy explicitly delegates) and
never pushes branches other than its own, and under Vela supervision does not push at all. Vela may
create the checkpoint commit when the worker leaves uncommitted changes. Build-time agents working
on Vela itself follow `AGENTS.md`.

# V1 Runtime Agent

When this document refers to the production runtime agent in v1, the concrete target is Antigravity unless a section explicitly discusses future extensibility or test doubles. Do not interpret generic terminology as a requirement for equal provider support.
