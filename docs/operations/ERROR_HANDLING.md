# Error Handling

Errors are typed: - USER_ACTION_REQUIRED - EXTERNAL_TEMPORARY -
EXTERNAL_AUTH - POLICY_BLOCK - REPOSITORY_CONFLICT -
VALIDATION_FAILURE - AGENT_FAILURE - INTERNAL_BUG

Every surfaced error answers: 1. What failed? 2. What state is
preserved? 3. Was anything committed/pushed/merged? 4. Will Vela retry?
5. What can the user do?

Retries require bounded exponential backoff with jitter for transient
external operations. Never retry destructive/non-idempotent actions
blindly.

# Approval Errors

Add typed errors: - `APPROVAL_SURFACE_NOT_FOUND` -
`APPROVAL_TARGET_AMBIGUOUS` - `APPROVAL_POLICY_UNKNOWN` -
`APPROVAL_DELIVERY_FAILED` - `APPROVAL_STALLED` -
`APPROVAL_WINDOW_MISMATCH`

Never respond to these by clicking repeatedly. Preserve the worker and
surface an intervention when bounded recovery fails.

# Additional Typed Errors

-   `APPROVAL_SURFACE_UNAVAILABLE`: interactive session unavailable (locked, disconnected, secure
    desktop) (ADR-012). Not a failure: the worker pauses and reconciles on return.
-   `APPROVAL_UNDELIVERABLE`: no permitted delivery tier can deliver a decision.
-   `POLICY_DENIED`: a `DENY` is repeated or required by the ticket.
-   `POLICY_VIOLATION`: a detective control fired (ADR-009).
-   `TRUST_REQUIRED`: Build or a repository-controlled execution was attempted in an `UNTRUSTED`
    repository (ADR-013).
-   `CAPABILITY_MISSING`: a required Antigravity capability is absent (`ADAPTERS.md`).
-   `POSTURE_NOT_MET`: the Native Permission Posture check is `DOES_NOT_MEET` or `UNKNOWN`
    (ADR-009).
-   `STATE_STORE_CORRUPT`, `STATE_STORE_UNRECOVERABLE`, `MIGRATION_FAILED`: see `RECOVERY.md`.
-   `PROVIDER_POLICY_BLOCK`: the provider indicates the account or service is blocked, suspended, disabled, or denied for
    terms or policy reasons (`CAPACITY_AND_PROFILES.md`). Class `POLICY_BLOCK`, origin provider. Never retried, never worked
    around, never cleared automatically; the answer to "Will Vela retry?" is always no.

Each maps to the typed classes above (`POLICY_BLOCK`, `USER_ACTION_REQUIRED`, `INTERNAL_BUG`,
`EXTERNAL_TEMPORARY`, and so on) and answers the five error questions.

# Antigravity Result Interpretation (verified 2026-10-02)

-   Exit code `0` and `result.status` `SUCCESS` are **not proof** that a tool ran or a task succeeded: a headless soft-denial of a tool exits `0`
    with an empty `SUCCESS` result and a stderr line; a tool step can report `DONE` after a denial; a quota error can exit `0` after retrying to the timeout.
-   Vela decides outcomes from the event stream, step errors (for example "denied by pre-tool hook", "permission check failed"), stderr, and Git/worktree
    state, and always passes an explicit `--print-timeout` (the default is `0s`, meaning wait for the turn).
-   Exit code `1` is shared by authentication failures and other general errors; `AUTH_REQUIRED` is determined from the error text and the capability
    probe, not the code alone.
