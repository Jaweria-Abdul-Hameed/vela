# Test Strategy

## Pyramid

### Unit

Pure graph algorithms, scheduler decisions, state transitions, policy
evaluation, parsers, reducers, UI state derivation.

### Integration

SQLite persistence, Git repositories/worktrees, process execution,
adapter contracts, restart reconciliation.

### End-to-end

Temporary fixture repository: project import → issue graph → two safe
parallel workers → merge → integration gate → completion.

### UI

Interaction tests for graph selection, keyboard navigation, stop/pause,
intervention states, settings.

### Visual regression

Key visual states: home, active graph, worker detail, conflict warning,
needs-human, completed run, reduced motion.

## Mandatory orchestration scenarios

-   explicit dependency serializes tickets,
-   safe independent tickets run together,
-   overlapping schema tickets serialize,
-   worker fails tests,
-   review requires fixes,
-   review stalls,
-   merge conflict,
-   integration test regression,
-   app restart mid-worker,
-   restart after commit before persisted completion,
-   network disappears during push,
-   profile enters cooldown,
-   Stop All during test,
-   dirty external repository modification,
-   cyclic issue dependencies,
-   missing skills,
-   malformed issue metadata.

## UI performance tests

Measure: - startup, - graph load with 50/100/500 nodes, - frame
stability during pan/zoom, - CPU/GPU use idle, - minimized/unfocused
resource use, - event-stream burst handling.

Tests must avoid brittle dependence on animation timing.

# Approval Automation Test Matrix

Add tests for: - native permission path succeeds with no UI fallback; -
native setting exists but prompt still appears; - accessibility adapter
finds correct Antigravity control; - unrelated application's "Approve"
button is never clicked; - ambiguous multiple approval controls fail
closed; - policy-allowed command is auto-approved; - force push remains
blocked; - credential prompt remains human-only; - repeated identical
prompt triggers `APPROVAL_STALLED`; - prompt disappears before click; -
Antigravity window moves/resizes/DPI changes; - UI label/layout changes
and adapter fails safely; - Vela restarts while prompt is visible; -
worker progresses after delivery and watchdog clears.

# Antigravity Runtime Test Obligation

Maintain fast fake-adapter tests for the core, but include a smaller Antigravity-specific compatibility/end-to-end suite for release confidence. Passing only provider-neutral unit tests is insufficient for v1.

Other production-provider suites are not required until those providers enter supported runtime scope.

# Additional Mandatory Scenarios (Prompt 3 fixes)

-   analyst returns invalid output: one correction retry, then affected pairs are "Unknown —
    sequential" (FR-007),
-   structured worker/review output fails schema validation: one correction retry, then escalate
    (FR-029),
-   merge conflict escalates to resolution attempt or human, never auto-resolved (ADR-011),
-   integration validation failure discards the unpublished merge and returns to `FIXING`,
-   checkpoint commit blocked by a Git hook returns the worker to `FIXING` (ADR-015),
-   checkpoint with a Git index lock present waits with bounded retry (ADR-015),
-   untrusted repository: no repository-controlled execution, Build refused (ADR-013),
-   native posture `UNKNOWN` blocks Autonomous mode (ADR-009),
-   operation evidence truncated or from non-control content yields `ASK` (ADR-009),
-   locked/disconnected desktop with a pending approval pauses safely and reconciles (ADR-012),
-   window close with and without background operation enabled; reboot with each
    `recovery_continuation` value (ADR-012),
-   corrupt state store, failed migration, and update during a run (`RECOVERY.md`),
-   provisioning failure before a worker starts; exclusive resource-key serialization,
-   cloud-synced worktree root blocked in preflight,
-   notification delivery for FR-028 events.
