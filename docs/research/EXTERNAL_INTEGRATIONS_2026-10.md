# External Integration Research Snapshot --- 2026-10

This file is a dated snapshot, not a permanent truth.

## Antigravity

Official documentation currently describes Windows
`Terminal Command Auto Execution` modes: - Request Review, - Proceed in
Sandbox, - Always Proceed.

It also documents project-level settings and a CLI permission model. The
newer unified desktop permission system is documented as differing by
OS, so Vela must capability-detect rather than assume parity.

Primary docs: - https://antigravity.google/docs/agent-settings -
https://antigravity.google/docs/permissions -
https://antigravity.google/docs/settings

## Matt Pocock engineering skills

Current upstream repository/docs describe: - `implement`: implement
decided work, TDD where appropriate, tests/typechecks, code review,
commit. - `code-review`: fixed-point Standards + Spec review. -
`implement-spec`: tickets as a task graph; parallel implementers use
separate worktrees/branches; work merges into an integration branch;
final code review follows.

Important upstream documentation also notes that
`implement`/`code-review` semantics around uncommitted diffs can require
care because fixed-point review is based on Git history. Vela's
checkpoint-before-authoritative-review ADR addresses this.

Primary source: - https://github.com/mattpocock/skills

Revalidate these assumptions before implementing adapters.

# Observed User Environment

Product discovery establishes a real-world compatibility requirement: in
the user's Antigravity 2.0 Windows workflow, approval prompts have
persisted despite attempts to use the permissive native setting. Treat
this as an observed environment constraint rather than assuming
documentation guarantees prompt-free execution. The adapter must be
tested against the actual installed version during development.

# Product-Scope Interpretation

External research about Claude, Codex, Gemini, or other development tools must not be interpreted as evidence that they are Vela v1 runtime targets. Current runtime research priority is Antigravity because it is the first-class production integration.

Research should continue to capability-detect Antigravity rather than assuming documented behavior always matches the installed environment.
