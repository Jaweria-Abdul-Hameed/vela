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

# Verification Items Required by Specification Decisions (for Prompt 4)

These are **not facts**; they are questions the specification now depends on. Prompt 4 must verify
them against current primary documentation and the installed environment and must not assume answers:

1.  Which native Antigravity permission modes or settings satisfy the Native Permission Posture
    (NPP-1..3, ADR-009), and whether an unconditional mode can be detected at user or machine level.
2.  Whether each capability in the Antigravity Required Capability Contract (`ADAPTERS.md`,
    CAP-01..CAP-11) exists, through which interface (CLI, SDK, daemon, other), and how parallel
    sessions map to processes and windows.
3.  Whether installed skills and slash commands (`/implement`, `/code-review`) can be invoked
    programmatically in a Vela-created session, and where skills are installed.
4.  Whether Antigravity exposes structured events (commands, tests, approval requests) usable under
    EBR-1.
5.  What the approval surface exposes through Windows UI Automation, and whether control identity can
    be correlated to a Vela-created session; behavior under lock, display-off, RDP, and elevation.
6.  How rate-limit and capacity conditions are surfaced.
7.  Whether session resume exists (CAP-10).
8.  Tauri behavior for tray, background lifecycle, keep-awake power requests, and login auto-start.

# Verification Status (2026-10-02)

The assumptions and the eight verification items above were verified in Prompt 4. The full, dated
evidence and classification (VERIFIED, CHANGED, UNDOCUMENTED, UNSUPPORTED, RUNTIME) are in
`EXTERNAL_VERIFICATION_2026-10-02.md`, which supersedes this snapshot wherever they differ. In short:

-   **Confirmed:** the three Windows auto-execution modes, the OS difference in permission engines, the
    Matt Pocock skills (`implement`, `code-review`, `implement-spec`, plus `to-tickets`), fixed-point
    review semantics, and the shared-stash caveat of Git worktrees.
-   **Changed:** Antigravity 2.0 now has three surfaces, a Desktop app, an `agy` CLI with a headless
    mode, and a Python SDK; native allow/deny/ask rules and `PreToolUse` hooks exist; Windows sandboxing
    shipped in desktop v2.15.1 although some pages still say otherwise; `/code-review` emits no severity
    or structured output.
- **Not erased:** the observed-environment requirement below stands. Documentation claims a permissive
  mode exists; open bugs in the official CLI tracker (for example #548, #1053) and the user's own
  observation show prompts and soft-denials persist. Vela stays defensive and capability-detected.
