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
authorized profile only if session semantics allow, - otherwise mark
worker `RATE_LIMITED`/`PAUSED`, - continue unrelated workers when safe.

Credentials are never stored in project Markdown or event logs.

# Runtime Profile Scope

Execution/capacity profiles in v1 describe supported, already-authorized Antigravity execution capacity. The data model may support future provider kinds, but that extensibility must not be interpreted as a requirement to rotate across or implement multiple providers.

Do not automate sign-in, passwords, 2FA, CAPTCHA, or account switching intended to evade service restrictions.
