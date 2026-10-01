# ADR-008: Antigravity-First, Provider-Extensible

## Status

Accepted.

## Context

Vela's core architecture deliberately avoids coupling scheduler, persistence, Git orchestration, policy, and recovery logic directly to one external coding-agent implementation. ADR-006 established provider independence as an architectural property.

That architectural decision can be misread as a product requirement for equal runtime support across Antigravity, Claude, Codex, and other coding agents.

That is not Vela v1's intended product scope.

Vela exists first to make the user's Google Antigravity workflow autonomous, reliable, recoverable, observable, and safe on Windows. This includes the fresh-session issue workflow, Matt Pocock skills, implementation/test/review loops, approval interruptions, worktrees, integration, and unattended operation.

Claude, Gemini, Codex, Antigravity, or other agents may all be used to **develop Vela**. Development-agent portability is not the same thing as Vela's shipped runtime support.

## Decision

Vela v1 is:

> **Antigravity-first, provider-extensible.**

Google Antigravity on Windows is the default, first-class, required production runtime integration for v1.

`AgentAdapter` remains the core architectural boundary. `AntigravityAdapter` is the required v1 production implementation.

Fake/test adapters remain part of the testing architecture.

Claude, Codex, and other production runtime adapters are future extension points and are not v1 deliverables unless an explicit approved scope change introduces them.

## Consequences

### Positive

- Vela can go deep on the workflow it is actually intended to solve.
- Antigravity-specific reliability work is not displaced by artificial provider parity.
- Core architecture remains resilient to external-product changes.
- Future providers can be added without rewriting scheduler/orchestration logic.
- Build-time use of multiple AI agents remains unrestricted.

### Tradeoff

Vela v1 will not claim equal runtime support for multiple coding-agent providers.

### Implementation rule

Do not scatter direct Antigravity conditionals throughout the core. Put external behavior behind typed adapter/capability boundaries while still implementing the Antigravity path completely.

### Release rule

V1 is not runtime-ready until the Antigravity path passes its required real-environment/compatibility acceptance gates. A fake adapter or a different coding agent cannot substitute for that evidence.

## Relationship to ADR-006

ADR-006 remains accepted.

ADR-006 answers **how the core is architected**: provider-independent boundaries.

ADR-008 answers **which production runtime v1 prioritizes and requires**: Antigravity.

They are complementary, not contradictory.
