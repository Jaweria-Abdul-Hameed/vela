# Autonomy Modes

## Supervised

Automatically: - implement, - test, - review/fix.

Ask before policy-selected external transitions such as push/merge.

## Autonomous

Automatically: - implement, - test, - checkpoint, - review/fix, -
push, - controlled merge, - frontier advancement.

Still stop for: - ambiguous product requirement, -
prohibited/destructive operation, - unresolved semantic merge
conflict, - credentials/2FA, - external divergence, - repeated review
failure, - integration regression that cannot be safely
attributed/reverted.

Autonomous mode is not "approve everything." It is a wider allow policy
inside explicit boundaries.

# Approval Behavior by Mode

**Supervised:** policy-allowed routine requests may be delivered
automatically if the user enabled this compatibility feature; sensitive
classes ask.

**Autonomous:** a broader project-scoped allow policy may be used, but
`DENY` and `ASK` classes remain enforced. Autonomous never means "click
every approval."

Both modes include approval-loop detection.

# Antigravity Runtime Meaning

For v1, Supervised and Autonomous modes govern Vela's control of Antigravity. They are Vela policy modes, not promises that equivalent autonomy exists across multiple coding-agent providers.
