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
