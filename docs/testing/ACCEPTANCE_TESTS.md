# Product Acceptance Tests

## AT-001 Unattended single ticket

Given a prepared repository and one ready ticket, when Build is pressed,
Vela completes implementation, validation, checkpoint, review,
push/merge policy, and marks the ticket done without ordinary command
approvals.

## AT-002 Safe parallel pair

Two independent tickets run in separate worktrees concurrently and merge
serially without lost changes.

## AT-003 Unsafe pair

Two tickets modifying a shared schema are not run concurrently; UI
explains why.

## AT-004 Review fix loop

A seeded review defect produces finding → fix → tests → checkpoint →
re-review → pass.

## AT-005 Crash recovery

Terminate Vela after checkpoint but before recorded worker completion.
Relaunch detects the commit and resumes without duplicate
implementation.

## AT-006 Stop All

During multiple workers, Stop All prevents new work, cancels safely,
preserves worktrees/commits, and allows later resume.

## AT-007 Capacity interruption

A worker receives a rate-limit/cooldown condition. Other safe work
continues; blocked worker preserves state and resumes/migrates only
through supported profile semantics.

## AT-008 Visual identity

Home and graph views match the design principles: black spatial canvas,
reactive dots, violet/blue/cyan ambient light, restrained glass, no
conventional dashboard dominance.

## AT-009 Efficiency

Minimizing the application substantially reduces/suspends rendering
while orchestration remains correct.

## AT-010 Human ambiguity

An intentionally contradictory ticket stops in `NEEDS_HUMAN` with a
precise decision request and no speculative product change.

# Approval Acceptance Tests

## AT-011 Persistent Antigravity prompt

Given Antigravity is configured permissively but still shows a routine
approval prompt, when Vela has classified the underlying project-scoped
action as allowed, Vela delivers approval through the best supported
adapter and the worker continues without user presence.

## AT-012 Unsafe approval

Given a destructive or credential-sensitive request, Vela does not
auto-approve it even if an approval button is detectable.

## AT-013 Approval loop

Given Antigravity repeatedly asks for the same approval without
progress, Vela stops automatic delivery after the configured threshold
and surfaces `APPROVAL_STALLED`.

## AT-014 Wrong window protection

Given another application exposes an "Approve" control, Vela never
activates it because process/window/session correlation fails.

# Antigravity-First Release Gate

V1 acceptance must include at least one complete representative workflow through the real/supported Antigravity integration path, including execution, validation/review flow, observable state, and recovery/approval behavior applicable to the environment.

A fake adapter or alternate development agent cannot substitute for this release gate.
