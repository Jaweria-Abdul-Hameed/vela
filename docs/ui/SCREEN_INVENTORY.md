# Screen and State Inventory

## Home

Empty, recent projects, invalid dropped folder, loading project.

## Project preflight

Running checks, all pass, warnings, blocking failure, skills missing,
auth required.

## Graph analysis

Analyzing docs, building DAG, cycle detected, conflict analysis, graph
ready.

## Build-ready

Summary + inspectable graph + Build.

## Active run

One worker, multiple workers, worker testing, worker reviewing, merge
lane busy, capacity cooldown.

## Ticket focus

Implementation, test evidence, review findings, Git history, logs.

## Intervention

Ambiguous requirement, merge conflict, destructive action, auth
required, external repository divergence, stalled review.

## Paused

Manual pause, Stop All completed, interrupted/recovered.

## Completion

Success, partial success, failed run.

## Settings

General, appearance, motion/performance, autonomy/policy, Git, agent
adapters, tracker, notifications, advanced diagnostics.

Every screen/state needs loading, empty, error, keyboard,
reduced-motion, and high-DPI behavior considered.

## Onboarding (first run)

Welcome, then the explicit choices of ADR-010 and ADR-012: guarded approval UI automation consent
(default off), background operation (default off), login auto-start (default off), recovery
continuation (`ask` or `auto_safe`, default `ask`). States: first run, revisit from Settings, consent
text version changed. Each choice is skippable and conservative by default.

## Repository trust sheet

Shown when a repository is `UNTRUSTED` and the user asks to Build (ADR-013). Lists what becomes
executable (scripts, hooks, provisioning, command profile), the confirmed command profile, and
Trust / Not now. States: untrusted (analysis only), trust confirmation, trust revoked.

## Tray menu (when background operation is enabled)

Reopen window, run status, Stop All, quit (with safe-pause behavior). Reduced-motion, keyboard, and
high-DPI considerations apply.

## Approval indicator and intervention variants

A compact approval indicator in the run bar (active, waiting on human, delivery degraded) and
intervention variants for `APPROVAL_ASK`, `APPROVAL_STALLED`, `APPROVAL_UNDELIVERABLE`,
`POLICY_DENIED`, `POLICY_VIOLATION`, `MERGE_CONFLICT`, `INTEGRATION_REGRESSION`, and interactive-
session-unavailable pauses, plus the scoped-rule editor (scope shown before saving) and the posture
and unattended-readiness summary in the preflight panel.

## Conversation link (ADR-020)

The ticket inspector and timeline show each worker's, reviewer's, and analyst's Antigravity conversation id with an "Open in Antigravity"
action. Until S-DESKTOP-VISIBILITY verifies a mechanism, the action shows the id and the verified way to locate the conversation and states
plainly when it cannot open it directly. States: available, not openable directly, conversation missing.
