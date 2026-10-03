# Execution Profiles and Capacity

Vela models model/provider capacity generically.

## Principles

-   Do not hard-code a particular rolling-window duration.
-   Prefer supported usage/capacity signals.
-   Treat rate-limit responses as structured availability events.
-   Never automate password entry, 2FA, passkeys, CAPTCHA, or
    browser-account chooser flows.
-   Never design account rotation as a mechanism to evade provider
    restrictions.
-   Multiple already-authorized profiles may be used only through
    supported mechanisms and user policy.

## States

`AVAILABLE`, `BUSY`, `COOLDOWN`, `AUTH_REQUIRED`, `UNAVAILABLE`,
`DISABLED`, `POLICY_BLOCKED` (see "Provider Policy Blocks" below).

## Scheduler behavior

When a profile becomes unavailable: - preserve worker state, - retry
according to provider-safe backoff, - migrate to another compatible
authorized profile only if session semantics allow (via recovery-resume at a checkpoint boundary; see `RECOVERY.md`), - otherwise mark
worker `RATE_LIMITED`/`PAUSED`, - continue unrelated workers when safe.

Credentials are never stored in project Markdown or event logs.

# Authentication Ownership (ADR-016)

For v1 an execution profile denotes the user's normally authenticated Antigravity account/session. Authentication
is owned by Antigravity: Vela never reads, extracts, copies, exports, persists, or reuses tokens or credentials,
and does not switch accounts. `AUTH_REQUIRED` asks the user to authenticate through Antigravity's supported flow.
API-key/Vertex billing is not part of the normal v1 runtime and never silently replaces it. Quota exhaustion is
currently surfaced by Antigravity as human-readable text (`RESOURCE_EXHAUSTED`, with a reset time), and headless
exit code `0` does not imply success, so capacity detection must inspect result status and error text rather than
exit codes alone.

# Provider Policy Blocks (FR-024; decided 2026-10-03 after the H01 research)

A **provider policy block** is a provider-neutral condition: the provider indicates, through a supported interface or its
documented output, that the user's account or the service has been blocked, suspended, disabled, or denied for terms or policy
reasons. It is not a capacity limit and not a missing sign-in. The domain model never encodes a provider's exact wording; the
adapter maps provider evidence into the neutral condition.

**Classification.** Typed error `PROVIDER_POLICY_BLOCK` (class `POLICY_BLOCK`, origin provider), carried by the neutral normalized
agent event `ProviderPolicyBlock` with: provider id, scope (`ACCOUNT`, `SERVICE`, or `UNKNOWN`), an evidence reference (redacted
excerpt and journal reference), and the time first detected. The Antigravity mapping lives only in
`vela-adapters::antigravity` (ADR-008, ADR-018). It emits the event only on a documented or recorded signal. An access failure that
cannot be classified is never treated as a policy block and never retried automatically: it surfaces as `AUTH_REQUIRED` (when the
evidence shows a missing sign-in) or as `NEEDS_HUMAN` kind `OTHER`, with the evidence. The exact signals are **unverified `[U]`**:
a real block cannot and must not be provoked, so the mapping is accepted against documented or recorded evidence and clearly
labelled synthetic fixtures, and any real occurrence is recorded in `docs/research/` and the mapping updated.

**State.**
-   The profile becomes `POLICY_BLOCKED` from any state. Nothing but an explicit user action leaves it: not time, not restart, not
    recovery, not a successful call.
-   In-flight provider work for the profile is cancelled at the next safe point. The worker becomes `NEEDS_HUMAN` with kind
    `PROVIDER_POLICY_BLOCK` and its `resume_state`. Deterministic Vela operations that are not provider work (merge lane, validation,
    Git) continue to their next safe point. When no ticket can progress without the blocked profile, the run becomes `NEEDS_HUMAN`
    with the same kind (never `PAUSED` with reason `CAPACITY`, never `RATE_LIMITED`).
-   The scheduler starts no new agent worker and creates no new Antigravity conversation on a `POLICY_BLOCKED` profile, and
    recovery-resume never starts a session on it.
-   The block is persisted with its journal records (`ProviderPolicyBlockRecorded`) in one transaction, together with the worker and
    run state. Branches, worktrees, checkpoints, and the last safe commit are preserved.

**Never.** Vela does not retry (no backoff schedule applies), re-issue the blocked request in any form, switch, rotate, or migrate
to another profile or account, invoke another account to continue the work, extract, replace, or reuse authentication, silently fall
back to an API key, poll the provider, or otherwise attempt to circumvent the restriction.

**Surfacing.** An intervention card of kind `PROVIDER_POLICY_BLOCK` (`HUMAN_IN_THE_LOOP.md`) answers the five error questions: what
failed, what state is preserved, whether anything was committed, pushed, or merged, that Vela will not retry, and what the user can
do (re-check access once, view the evidence, follow the provider's own process, or cancel the run). Notification follows FR-028.

**Clearing.** Only an explicit user decision clears the block, and only after one user-initiated access re-check through the
supported interface (the same capability and authentication-state interface preflight uses). The re-check result is evidence, not
authority: while it fails the block stands. Where re-authentication is the officially appropriate remedy, the user performs it
through the supported flow (FR-049); Vela never signs in. Clearing records `ProviderPolicyBlockCleared` with the actor and evidence;
the worker or run then returns to its recorded `resume_state` through recovery-resume (a new session at the last checkpoint). There is
no automatic clearing and no re-check loop.

**Out of scope.** Any bypass, account rotation, API-key fallback (excluded from v1, ADR-016), and any judgment about whether the
provider's decision is correct.

# Runtime Profile Scope

Execution/capacity profiles in v1 describe supported, already-authorized Antigravity execution capacity. The data model may support future provider kinds, but that extensibility must not be interpreted as a requirement to rotate across or implement multiple providers.

Do not automate sign-in, passwords, 2FA, CAPTCHA, or account switching intended to evade service restrictions.
