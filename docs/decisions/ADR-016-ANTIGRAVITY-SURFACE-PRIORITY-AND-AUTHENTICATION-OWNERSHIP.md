# ADR-016: Antigravity Execution Surface Priority and Authentication Ownership

## Status

Accepted (2026-10-02). Refines ADR-008 (Antigravity-first), ADR-007, ADR-009 and ADR-010. These are
**product decisions, not evidence** that any external capability exists: every capability remains
subject to the real-environment probes in `docs/research/EXTERNAL_VERIFICATION_2026-10-02.md`.

## Context

Prompt 4 verified that Antigravity 2.0 has three surfaces (Desktop app, `agy` CLI with a headless mode,
and a Python SDK) while the specification had assumed a single GUI-driven surface. The user observed
persistent approval prompts in the **Antigravity Desktop GUI on Windows**, including under permissive and
Always Proceed-style settings. The human decisions DR-1 and DR-3 were taken on 2026-10-02.

## Decision

### Execution surface priority (DR-1)

1.  **Primary surface:** the official `agy` CLI headless interface, **conditional on real-environment probes
    confirming** that it satisfies the Antigravity Required Capability Contract (`ADAPTERS.md`,
    CAP-01..CAP-11).
2.  Use supported **hooks, session, and conversation mechanisms** where they are verified in the real
    environment.
3.  **Windows UI Automation is the secondary compatibility surface**, used for capabilities the supported
    programmatic surface cannot provide, particularly delivery of approvals shown in the Desktop GUI. The
    **guarded visual adapter remains the last-resort** approval fallback (ADR-007). UI automation stays
    opt-in (ADR-010).
4.  **The SDK is not a v1 dependency.** Prompt 4 found it official but Alpha/Research Preview, authenticated
    only by API key or Vertex, and without documented resume or cancellation. It may be reconsidered only if a
    later verification finds an officially supported SDK appropriate for these capabilities and DR-3.
5.  **No substitution:** if a Required capability cannot be verified or is unsupported, preflight reports the
    `BLOCK` or degradation defined in `ADAPTERS.md`; this ADR authorizes no adapter workaround and no other
    provider. A failing probe leads to a specification-change decision.

### Authentication ownership (DR-3)

6.  Vela v1 uses the **official Antigravity binary with the user's normally authenticated Antigravity
    account/session**, provided the supported product permits this. The official standing of external
    orchestration of the headless CLI is **not confirmed by a vendor source** (only a forum answer and a
    Google statement prohibiting token reuse were found); confirming it is a release prerequisite.
7.  **Authentication is owned by Antigravity.** Vela must not read, extract, copy, export, persist, or reuse
    Antigravity authentication tokens or credentials, including entries in the Windows Credential Manager or
    authentication material under the Antigravity application-data directories.
8.  `AUTH_REQUIRED` causes Vela to ask the user to authenticate through Antigravity's supported flow; Vela does
    not automate sign-in.
9.  **API-key/Vertex billing is not required** for the normal v1 runtime and must not silently replace it. It
    may remain a future explicit runtime/provider option under a separate approved scope.

### Open design question (to be resolved by probes and Prompt 5)

An approval prompt is a GUI card in the Desktop app. A headless CLI session has no GUI prompt: documented
behavior is a soft-denial of tools that need approval. The approval-delivery path for a **primary-surface
(headless) session** is therefore native allow/deny/ask rules and `PreToolUse` hooks (themselves subject to
open vendor bugs), while the UI-automation surface applies where a Desktop-hosted prompt exists. How these
combine for Vela-created sessions is not decided here.

## Consequences

-   `ADAPTERS.md`, `PRODUCT_SPEC.md` (FR-049), `PREFLIGHT.md`, `CAPACITY_AND_PROFILES.md`, and
    `SECURITY_AND_PERMISSIONS.md` reference this ADR.
-   Execution profiles (`CAPACITY_AND_PROFILES.md`) denote the user's Antigravity-authenticated session; Vela
    does not switch accounts or manage credentials.
-   Probe results gate release: unverified capabilities cannot be claimed.
