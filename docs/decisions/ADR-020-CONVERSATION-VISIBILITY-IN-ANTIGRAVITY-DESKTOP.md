# ADR-020: Worker Conversations Must Be Visible and Openable in Antigravity Desktop

## Status

Accepted (2026-10-02), recorded after the architecture freeze because it exposed an actual contradiction with ADR-018 section 2.
Refines ADR-016 and ADR-018; does not change the primary-surface decision. This ADR records a **required capability and its
constraints**, not an implementation mechanism: no mechanism that has not been verified is chosen here.

## Context

Product clarification from the user (2026-10-02): every Vela issue/worker must run in a **fresh Antigravity conversation**, and
that conversation must be **discoverable and openable in the Antigravity 2.0 Desktop GUI** so the user can inspect the real
conversation, not only a Vela rendering of it. Vela should still orchestrate through the supported programmatic `agy` surface
where possible; this does not require Vela to automate the Desktop GUI to create conversations.

Evidence in this environment (read-only filesystem checks and the probes):

-   Conversations created by `agy` in the real user profile are stored under `~/.gemini/antigravity-cli/`
    (`brain/<id>`, `conversations/<id>.db`, `annotations/<id>.pbtxt`, `presence/<id>.lock`). The Desktop app's conversation store is
    `~/.gemini/antigravity/conversations/`. **None** of the probe conversation ids from the CLI runs appear in the Desktop store.
-   The CLI registered a `CLI Project` entry (`~/.gemini/config/projects/default-cli-project.json`) that the user saw in the Desktop
    app. Whether that project lists CLI conversations, and whether they can be opened, has **not** been observed.
-   Vendor documentation says the CLI operates "alongside Antigravity 2.0 through shared agent infrastructure and synchronized
    settings" and that Remote Control lets the Desktop UI interact with a CLI session (documented for an interactive session). Neither
    statement establishes that a headless, per-turn conversation is visible or openable in the Desktop app.
-   ADR-018 section 2 gives each worker an isolated `USERPROFILE`/`HOME`. Conversations created that way are stored under the Vela
    profile directory, which is the location the Desktop app is least likely to read. **This conflicts with the requirement unless
    the spike below shows otherwise.**

## Decision

1.  **Requirement.** Vela shall create a fresh Antigravity conversation for every worker, reviewer, and analyst session (already required
    by ADR-015 and `AGENT_PROTOCOL.md`), shall record each conversation's identifier against its ticket/worker/review cycle, and
    shall make each conversation **discoverable and openable in Antigravity Desktop** (FR-050, capability CAP-12, acceptance test AT-026).
2.  **Preference order.** (a) A CLI-created conversation that appears in, and can be opened from, Antigravity Desktop, if verified; (b) another
    verified mechanism; (c) Desktop GUI automation to create conversations only if a later decision approves it (not assumed here).
3.  **Nothing is invented.** The mechanism by which a CLI-created conversation becomes visible or openable in Desktop is **unverified**.
    It is captured as Phase 0 spike **S-DESKTOP-VISIBILITY** and as a release-gating required capability. Until the spike closes with
    evidence, no ticket may claim the requirement satisfied.
4.  **Constraint on ADR-018.** The isolated per-worker profile in ADR-018 section 2 is **provisional**. It is kept only if the spike shows
    that conversations created under an isolated profile are visible/openable in Desktop (or that a verified mechanism makes them so).
    If not, the policy-enforcement design must be revisited using the verified alternatives, **none of which is chosen here**: running in
    the real profile (which conflicts with the "do not modify the user's global Antigravity settings" safety rule unless the user consents),
    a verified configuration location that Desktop reads, or a different Desktop-visible store. Any such change requires a new
    decision and, if it touches the user's global settings, explicit consent (ADR-010/013 principles).
5.  **No weakening of policy sovereignty.** Whatever resolves visibility must still satisfy ADR-009 (Vela policy remains enforceable) and
    ADR-016 (Vela never reads, copies, or persists credentials or tokens).
6.  **UI.** The ticket inspector and timeline show the conversation id and an "Open in Antigravity" action. Until a mechanism is verified,
    the action shows the id and the verified way to find the conversation, and states plainly when it cannot open it directly.
7.  **Persistence.** `workers.conversation_id`, `review_cycles.reviewer_conversation_id`, and the analyst conversation id are stored and
    journaled; they survive recovery and are used by recovery-resume (`RECOVERY.md`).

## Consequences

-   New requirement FR-050, capability CAP-12, acceptance test AT-026, spike S-DESKTOP-VISIBILITY, and a seed ticket.
-   ADR-018 section 2 and the architecture's isolated-profile statements are marked conditional on this ADR.
-   Open question for Prompt 6 and the spike: observe, from the Desktop app, whether conversations from (i) the real profile and (ii) an
    isolated profile appear, with which project, and whether they can be opened (a user-assisted step).
