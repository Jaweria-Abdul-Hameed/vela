# Cross-Agent Context Sharing

## Goal

Gemini/Antigravity, Claude, Codex, and future agents must reach the same
product conclusions without relying on proprietary memory.

## Durable hierarchy

1.  Code/tests --- actual behavior.
2.  ADRs --- why architecture decisions exist.
3.  Product/architecture/UI specs --- desired behavior.
4.  GitHub issues --- bounded implementation scope.
5.  Research notes --- dated external facts.
6.  Execution journal --- what happened during a run.
7.  Chat history --- never canonical.

## Rules

-   Put decisions in repository files before distributing work.
-   Use stable paths and links in tickets.
-   Do not paste giant duplicated specs into every issue.
-   If an agent discovers a durable fact that affects later tickets,
    update the appropriate doc/ADR/research note as part of the ticket
    or create a follow-up.
-   Provider-specific instruction files defer to `AGENTS.md`.
-   Contradictions are surfaced, not resolved silently by whichever
    model runs last.

# Build Agents vs Runtime Agent

Repository context is intentionally portable across Claude, Gemini, Codex, Antigravity, and future development agents so any of them can help **build Vela**. This cross-model development portability is separate from shipped runtime scope: Vela v1 itself targets Antigravity first.
