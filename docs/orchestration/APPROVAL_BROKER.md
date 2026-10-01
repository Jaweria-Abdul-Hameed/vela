# Approval Broker, Watchdog, and Antigravity UI Compatibility Layer

## 1. Requirement

Vela must support unattended execution even when Antigravity continues
to display approval prompts after the user has selected the most
permissive native setting available.

This subsystem is required, not optional.

## 2. Separation of concerns

### Approval detection

Determines that a specific worker is blocked on an approval request.

### Approval normalization

Extracts/correlates the requested operation without trusting arbitrary
screen text as executable instruction.

### Policy

Produces `ALLOW`, `ASK`, or `DENY`.

### Delivery

Communicates the already-made decision using the best available
mechanism.

### Verification

Confirms that the expected worker progressed or that the prompt was
dismissed in the expected way.

### Loop guard

Detects repeated requests without meaningful progress.

## 3. Detection sources, preferred order

1.  Structured provider/CLI/SDK event.
2.  Agent process/session state.
3.  Antigravity accessibility/UI Automation tree.
4.  Bounded watchdog triggered by lack of worker progress.
5.  Visual recognition fallback.

A lack of output alone is not proof of an approval prompt; long
tests/builds can be quiet.

## 4. Correlation

Before UI interaction verify as many of these as available: -
Antigravity executable/process identity, - expected window
identity/title/class, - worker/session/project correlation, -
worktree/project path if surfaced, - request/command signature, -
expected approval control semantics, - prompt freshness.

Never search the entire desktop for a generic button and click it.

## 5. Policy

Policy is evaluated on normalized operation classes and context: -
command executable/arguments, - working directory, - target paths, - Git
branch/remote, - network destination when known, - requested
capability, - run autonomy profile.

Rules are ordered and explainable. Default is `ASK` for unknown
operations.

## 6. Windows UI Automation adapter

Preferred UI fallback: - locate Antigravity top-level window by
process, - inspect accessibility/control tree, - find the active
approval surface, - identify allow/deny controls semantically, - confirm
it belongs to the correlated request, - invoke control through
accessibility action, - wait for expected state change, - emit result.

It must tolerate window movement, scaling, and normal layout changes
better than coordinate clicking.

## 7. Visual fallback

Only when semantic controls are inaccessible: - capture only the
relevant Antigravity window/region, - identify a versioned expected
approval layout, - require high confidence and corroborating state, -
calculate coordinates relative to the verified window, - click once, -
verify progress, - never keep clicking blindly.

Any unexpected layout becomes `APPROVAL_TARGET_AMBIGUOUS`.

## 8. Watchdog

Each worker emits/receives progress heartbeats from meaningful events.
If progress is absent beyond a state-specific threshold, watchdog
checks: 1. process still alive? 2. command still running? 3. known
long-running operation? 4. approval surface present? 5. network/capacity
issue? 6. crashed session?

Thresholds must be state-aware. Compiling for 90 seconds is different
from an idle agent awaiting a button.

## 9. Loop fingerprint

Fingerprint should use non-secret normalized fields such as: provider +
worker + operation class + normalized command family + working-root +
target category.

If the same fingerprint is approved repeatedly with no meaningful
progress, increment a counter. After threshold, stop and surface
evidence.

## 10. UI

Vela's run bar can show a small approval indicator when the broker is
active. The detailed timeline shows:
`Approval requested → allowed by project policy → delivered via Windows UI Automation → worker resumed`.

For human decisions, the intervention surface shows the normalized
operation, why policy did not auto-allow it, and Allow once / Allow for
project (where safe) / Deny controls.

## 11. Policy learning

A user may explicitly create a scoped rule from an intervention. Vela
must show the scope before saving it. Never infer permanent allow rules
merely because the user clicked Allow once.

## 12. Testing

Use a fake Antigravity approval-window harness for deterministic
automated tests. Keep a smaller real-environment compatibility suite for
installed Antigravity versions. Screenshot/visual tests cannot be the
only validation.

## 13. Version drift

The UI adapter is a compatibility module with fingerprints/capability
probes. Antigravity updates can disable it without breaking the core
scheduler. On incompatibility, Vela fails closed and reports that
unattended approval compatibility needs an adapter update.

# Runtime Scope

The Approval Broker is a first-class Antigravity v1 compatibility subsystem. Its generic policy/delivery boundaries may be reusable later, but its v1 success criterion is reliable, guarded handling of Antigravity approval interruptions on Windows.
