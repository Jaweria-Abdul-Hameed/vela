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
`DISABLED`.

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

# Runtime Profile Scope

Execution/capacity profiles in v1 describe supported, already-authorized Antigravity execution capacity. The data model may support future provider kinds, but that extensibility must not be interpreted as a requirement to rotate across or implement multiple providers.

Do not automate sign-in, passwords, 2FA, CAPTCHA, or account switching intended to evade service restrictions.
