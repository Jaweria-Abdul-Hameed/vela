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
