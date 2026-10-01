# Autonomy Modes

Both modes require the repository to be `TRUSTED` before Build (ADR-013).

## Supervised

Automatically: - implement, - test, - review/fix.

Ask before policy-selected external transitions such as push/merge/promotion. In Supervised
mode Vela itself (not the worker) performs push and merge after the user approves (ADR-015).

## Autonomous

Automatically: - implement, - test, - checkpoint, - review/fix, -
push, - controlled merge, - frontier advancement.

Autonomous mode requires the Effective Posture Check to report `MEETS` (ADR-009). It is not
available when the posture is `DOES_NOT_MEET` or `UNKNOWN`.

Still stop for: - ambiguous product requirement, -
prohibited/destructive operation, - unresolved semantic merge
conflict, - credentials/2FA, - external divergence, - repeated review
failure, - integration regression that cannot be safely
attributed/reverted.

Autonomous mode is not "approve everything." It is a wider allow policy
inside explicit boundaries.

# Approval Behavior by Mode

**Supervised:** policy-allowed routine requests may be delivered
automatically through the native tier, and through the UI-automation and visual tiers only if the
user has enabled guarded UI automation (ADR-010); sensitive classes ask. If the posture check does
not report `MEETS`, Supervised mode requires a recorded user acknowledgement and shows a
persistent banner that Vela cannot enforce policy over natively auto-executed operations
(ADR-009).

**Autonomous:** a broader project-scoped allow policy may be used, but
`DENY` and `ASK` classes remain enforced. Autonomous never means "click
every approval." UI-automation and visual delivery also require the persisted onboarding consent
(ADR-010); without it, routine prompts that native delivery cannot resolve surface as
interventions and preflight reports the reduced unattended readiness.

Both modes include approval-loop detection.

# Antigravity Runtime Meaning

For v1, Supervised and Autonomous modes govern Vela's control of Antigravity. They are Vela policy modes, not promises that equivalent autonomy exists across multiple coding-agent providers.
