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
screen text as executable instruction. Evidence is bound to an operation
only under the Evidence-Binding Rules (EBR-1..6) of ADR-009, summarized in
section 14.

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
tests/builds can be quiet. Sources 3 (accessibility tree) and 5 (visual) are part of the
opt-in UI-automation capability (ADR-010); sources 1, 2, and 4 are always available.

## 4. Correlation

Before UI interaction verify as many of these as available: -
Antigravity executable/process identity, - expected window
identity/title/class, - worker/session/project correlation, -
worktree/project path if surfaced, - request/command signature, -
expected approval control semantics, - prompt freshness.

Never search the entire desktop for a generic button and click it.

Only windows and sessions that belong to Antigravity sessions **created by
Vela** (identities recorded at creation) are eligible targets. An Antigravity
window the user is using independently is never targeted, even if it shows
an approval prompt.

## 5. Policy

Policy is evaluated on normalized operation classes and context: -
command executable/arguments, - working directory, - target paths, - Git
branch/remote, - network destination when known, - requested
capability, - run autonomy profile.

Rules are ordered and explainable. Default is `ASK` for unknown
operations.

Policy is sovereign (ADR-009): Vela's decision does not depend on, and is
not replaced by, any native Antigravity auto-execution mode. Auto-allow of
test/lint/typecheck/build commands applies only to commands in the
user-confirmed command profile of a `TRUSTED` repository (ADR-013); other
commands are `ASK`.

## 6. Windows UI Automation adapter

Available only after the user has enabled guarded UI automation during
onboarding or in Settings (ADR-010). Preferred UI fallback: - locate Antigravity top-level window by
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

Initial defaults (configurable; to be validated by Prompt 4 and Prompt 15):
for agent-driven states with no tool, command, or file activity, a
no-progress window of 120 seconds before the watchdog inspects; while a
known command is still running (process alive and tracked), the watchdog
does not fire on silence alone. Values live in `CONFIGURATION.md`.

## 9. Loop fingerprint

Fingerprint should use non-secret normalized fields such as: provider +
worker + operation class + normalized command family + working-root +
target category.

**Meaningful progress** between two deliveries of the same fingerprint means
at least one of: a different normalized operation completed; a new commit or a
change of the worktree diff hash; a test or validation run completed; a worker
state transition; or a different approval fingerprint was resolved. Repetition
of the same fingerprint with none of these increments the counter; any of them
resets it. The counter is per worker and persisted for the run.

Default threshold: 3 consecutive same-fingerprint deliveries without progress
(configurable, bounded between 2 and 10). After the threshold, stop automatic
delivery, transition the worker to `NEEDS_HUMAN` (kind `APPROVAL_STALLED`),
and surface evidence. Legitimate repeats (for example repeated test runs in a
fix loop) reset the counter through the progress signals above.

## 10. UI

Vela's run bar can show a small approval indicator when the broker is
active. The detailed timeline shows:
`Approval requested → allowed by project policy → delivered via Windows UI Automation → worker resumed`.

For human decisions, the intervention surface shows the normalized
operation, why policy did not auto-allow it, and Allow once / Allow for
project (where safe) / Deny controls.

**Where a human `ASK` decision is delivered:** the decision is made in the Vela
intervention surface. Vela then delivers it through the same delivery chain used
for `ALLOW` (basis recorded as `human_authorized`), subject to the same
correlation and freshness checks. If no permitted tier is available (for
example UI automation is not enabled), the surface instructs the user to
respond in Antigravity directly; Vela detects the resolution through progress
signals and never delivers a second response.

**`DENY` outcome:** where a deny/reject control exists, Vela delivers a refusal
through the delivery chain so the agent can continue or adapt, journals it, and
resumes the prior state. A repeated `DENY` of the same fingerprint beyond the
threshold, or a denied operation the ticket requires, raises `NEEDS_HUMAN` (kind
`POLICY_DENIED`). If no permitted tier can deliver the refusal, the worker is
`NEEDS_HUMAN` (kind `APPROVAL_UNDELIVERABLE`) and the underlying operation
simply stays blocked.

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

# Posture, Evidence, Consent, and Session Conditions

## 14. Evidence binding (ADR-009)

An approval prompt is bound to a normalized operation only when:

-   facts come from the highest-trust channel available: structured
    provider/CLI/SDK event, then the accessibility subtree of the identified
    approval control, then verified visual recognition;
-   only text inside the approval control's own subtree, in a window
    verified to belong to a Vela-created session, counts as evidence;
    transcript, chat, tool output, or other agent-authored content never
    does;
-   truncated, elided, scrolled, multi-operation, or non-normalizable text
    yields `ASK`;
-   visual-tier evidence yields `ALLOW` only for operation classes the
    versioned layout fingerprint declares fully legible;
-   where a post-execution observation exists, Vela compares it with the
    approved signature, and a mismatch is a policy violation (worker paused,
    `NEEDS_HUMAN` kind `POLICY_VIOLATION`, journaled);
-   the control identity must match a versioned adapter fingerprint; an
    unrecognized surface fails closed.

## 15. Native permission posture (ADR-009)

The Broker operates under the Native Permission Posture: operations outside the
native envelope must surface to a Vela-observable channel, and the native
envelope must be bounded to the worktree and contain no `ASK`/`DENY` class.
Vela never configures or depends on unconditional auto-execution. Preflight
reports `MEETS`, `DOES_NOT_MEET`, or `UNKNOWN`; the exact Antigravity mechanism
is verified by Prompt 4 and confirmed by Prompt 15.

## 16. Consent (ADR-010)

Tiers 2 and 3, including reading the accessibility tree and screen capture,
require the persisted onboarding consent. Without it the Broker detects only from
non-UI sources (provider events, process/session state, watchdog), classifies,
may use native delivery, and surfaces remaining prompts as interventions.

## 17. Interactive-session conditions (ADR-012)

UI-automation delivery requires an interactive, unlocked desktop. A locked,
disconnected, or secure desktop is never bypassed. Affected workers pause with
reason `INTERACTIVE_SESSION_UNAVAILABLE` (error `APPROVAL_SURFACE_UNAVAILABLE`),
unaffected workers continue, and on return of the interactive session Vela
re-detects the prompt, re-verifies correlation and freshness, and re-evaluates
policy before delivering. Display-off alone is not a failure.

## 18. Visual-tier delivery safeguards

Before a visual click Vela re-verifies window identity, layout fingerprint, and
prompt freshness immediately prior to the click, acquires a global visual-
delivery mutex (one visual delivery at a time across workers), and aborts if
user input is detected within a short guard interval before the click.
