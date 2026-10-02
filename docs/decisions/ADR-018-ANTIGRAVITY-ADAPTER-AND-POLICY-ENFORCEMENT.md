# ADR-018: Antigravity Adapter and Policy-Enforcement Architecture

## Status

Accepted (2026-10-02). Realizes ADR-009 (native permission posture), ADR-010 (UI automation consent), ADR-016 (surface
priority) and ADR-007; does not reopen them. Every design element below is tagged with the verification status of the
behavior it depends on: **[V]** verified in the real environment, **[P]** partially verified, **[U]** unverified and
gated by a Phase 0 spike. A gated element must not be claimed ready until its spike passes.

## Context

Section S probes (`docs/research/EXTERNAL_VERIFICATION_2026-10-02.md`, sections T-V) established for `agy` 1.2.14 on
Windows: headless soft-denial of commands with exit 0 and `status SUCCESS` **[V]**; user-level `permissions.allow`
rules (exact, regex, path-scoped `write_file`) work and unlisted targets are denied **[V]**; a repo-level
`.gemini/config.json` is ignored **[V]**; settings can be isolated by redirecting `USERPROFILE`/`HOME` while
authentication still works **[V]**; workspace `.agents/hooks.json` loads in headless mode, `PreToolUse` input carries the
command line and working directory, `deny` is a hard block even under skip-permissions, hook crash/garbage/timeout/empty
output **fail closed**, and hook `allow` cannot grant **[V]**; skills resolve in print mode **[V]**; kill leaves no
orphans and `--conversation` resume keeps context **[V]**; the agent's shell is PowerShell **[V]**; the Desktop approval
card is readable by UI Automation after warm-up and one single-use allow was delivered by `SelectionItemPattern` plus
`InvokePattern` with no persistent permission **[V for one delivery; reliability P]**.

## Decision

### 1. Primary surface: one `agy` process per turn

The `AntigravityAdapter` drives `agy -p <prompt> --output-format stream-json --print-timeout <explicit>` with the
worktree as working directory, one process per turn, continuing a conversation with `--conversation <id>` **[V]**.
Multi-turn stdin streaming (`--input-format stream-json`) is not used in v1 **[U]**. Vela never trusts the exit code or
`result.status`; it interprets the event stream, stderr, and Git state **[V]**: a soft-denial appears as an empty `SUCCESS`
result plus a stderr "auto-denied" line, and a tool step can be `DONE` after a denial.

### 2. Isolated per-worker profile outside the worktree

Each worker has a Vela-owned profile directory under the Vela data root. The child process runs with `USERPROFILE` and
`HOME` pointing at it **[V]**. The profile contains the CLI `settings.json` (generated allow rules) and, where verified, the
hook configuration. Authentication stays with Antigravity's OS credential store and is never read or copied (ADR-016
section 7) **[V]**. The reviewer session uses its own profile.

### 3. Policy enforcement: generated allow rules plus a fail-closed deny hook

-   **ALLOW delivery (native):** Vela generates `permissions.allow` entries only from the user-confirmed command profile
    of a trusted repository (exact or regex `command(...)`) and a path-scoped `write_file(<worktree>)` (ADR-013) **[V]**.
-   **DENY and ASK enforcement:** a registered `PreToolUse` hook, the small `vela-hook` executable, evaluates each call
    against a rule table Vela writes into the profile. It returns `deny` for DENY-class and unknown operations and a
    structured reason for ASK-class operations, and writes a spool record for Vela **[V: deny, fail-closed, input
    fields]**. Because a hook cannot grant and an empty response is a deny **[V]**, the hook returns `allow` for
    allow-class operations; the combination "allow rule plus hook `allow`" is **[U]** and is spike S-NATIVE-POSTURE.
-   **Hook placement:** preferred location is the isolated profile's global hook file **[U]**; fallback is the worktree's
    `.agents/hooks.json` **[V loads]** protected by a path-scoped deny rule on `.agents` and a per-tick integrity check
    by Vela **[U: deny-over-allow in practice]**. Policy files are never placed where a path-scoped write allow covers
    them **[P: write inside an allowed worktree succeeds]**.
-   **Posture check:** preflight runs the spike scenarios (deny blocks, unlisted denied, allow runs, hook failure
    blocks) against the installed version; only a pass yields `MEETS` (ADR-009). Until then Autonomous mode stays
    blocked.

### 4. ASK path for headless sessions

An ASK-class operation is blocked by the hook with a reason carrying a Vela request id. Vela raises `APPROVAL_ASK`
(`ORCHESTRATION_ENGINE.md`). On a human "allow once", Vela adds an exact, expiring allow rule to that worker's profile and
resumes the conversation with an explicit instruction that the user approved the command **[U: whether the agent retries
after an earlier refusal; spike S-ASK-RESUME]**. A refusal leaves the block in place.

### 5. Secondary surface: Desktop UI Automation, off unless consented

`vela-uia` implements `ApprovalSource` and `ApprovalDeliveryAdapter` for Desktop-hosted approval cards (ADR-010 consent
required). Verified facts it must honor **[V]**: warm up and retry while the Chromium tree populates; the window must
be a visible normal window; match controls by role and label, never by identifier (identifiers contain a
session-unique part); correlate title, command fragments, working directory, and status text before acting; select
**only** the single-use allow or the refusal and **never** the persistent "always allow" options; invoke through
`SelectionItemPattern` and `InvokePattern`, never coordinates. Reliability beyond one delivery, other Desktop versions,
minimized windows, concurrent cards, and the refusal path are **[P/U]**. In the primary headless path no Desktop card
exists, so this adapter is dormant unless a Vela-correlated Desktop-hosted session is present; whether v1 creates such
sessions is an open question (CURRENT_STATE).

### 6. Visual fallback

`vela-uia::visual` is a separate, feature-gated module that is compiled in but disabled unless the user has consented
(ADR-010) and UIA discovery is unavailable; it implements ADR-007's last-resort rules and is **[U]** (nothing probed).

### 7. Capacity, authentication, and errors

Quota exhaustion is detected from `RESOURCE_EXHAUSTED` text in `result.error` or stderr **[V text]**; Vela sets explicit
`--print-timeout` values (the default is 0s, unlike the documentation) **[V]**. `AUTH_REQUIRED` asks the user to sign in
through Antigravity (ADR-016). A background `agy` self-update is a divergence handled by `RECOVERY.md`.

## Alternatives rejected

SDK in-process (excluded by ADR-016); hook `allow` as the grant mechanism (verified not to grant); repo-level settings
(verified ignored); editing the user's global CLI settings (rejected for safety; isolation verified); persistent
stdin-streaming sessions (unverified); Desktop GUI as the primary surface (no programmatic API).

## Consequences

-   `crates/vela-adapters::antigravity`, `crates/vela-hook`, and `crates/vela-uia` realize this ADR.
-   Phase 0 spikes: S-NATIVE-POSTURE, S-ASK-RESUME, S-HOOK-GLOBAL, S-SCHEMA-OUTPUT (`--json-schema`), S-INTERACTIVE-TRUST,
    S-UIA-BINDINGS, S-UIA-RELIABILITY, S-VISUAL.
