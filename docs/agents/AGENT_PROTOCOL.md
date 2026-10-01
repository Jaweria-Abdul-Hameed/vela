# Agent Session Protocol

## Input envelope

Every worker receives: - run ID, - ticket full identifier/URL, - ticket
content, - integration base SHA, - worker branch/worktree, - relevant
spec paths, - relevant ADR paths, - repository instructions, - test
commands/policy, - review policy, - explicit prohibited actions.

Use context pointers instead of duplicating entire documents whenever
the agent can read the repository.

## Session freshness

Each ticket starts in a fresh context. Do not carry conversational state
from a previous ticket as an authority. Shared knowledge must be in
code, commits, issues, research notes, or Markdown.

## Exploration

Expensive exploration that benefits multiple workers should be performed
once and saved as durable notes. Ticket workers may still inspect local
code necessary for implementation.

## Output envelope

Workers must return machine-readable fields where supported: - status, -
ticket, - base_sha, - head_sha, - commits, - files_changed, - tests, -
review_cycles, - warnings, - discovered_dependencies, - merge_risk, -
branch, - worktree.

Free-form prose may accompany but never replace structured completion
facts.

## Session boundaries

A worker may not silently begin another ticket. Completion returns
control to the scheduler.

# V1 Runtime Agent

When this document refers to the production runtime agent in v1, the concrete target is Antigravity unless a section explicitly discusses future extensibility or test doubles. Do not interpret generic terminology as a requirement for equal provider support.
