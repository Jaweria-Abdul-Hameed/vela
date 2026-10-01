# Orchestration Engine

## 1. Run state machine

``` text
CREATED
  ↓
PREFLIGHT
  ↓
ANALYZING
  ↓
GRAPH_READY
  ↓
AWAITING_START
  ↓
RUNNING
  ├─→ PAUSED
  ├─→ NEEDS_HUMAN
  ├─→ FAILED
  ↓
FINAL_REVIEW
  ↓
FINAL_VALIDATION
  ↓
COMPLETED
```

## 2. Scheduling cycle

On every scheduling tick caused by a material event:

1.  Reconcile external state.
2.  Confirm integration branch health.
3.  Mark newly satisfied explicit blockers.
4.  Recompute ready frontier.
5.  Apply capacity constraints.
6.  Apply pairwise/group parallel-safety constraints.
7.  Select maximal safe set up to concurrency limit.
8.  Create worker records transactionally.
9.  Provision worktrees.
10. Start fresh sessions.
11. Observe workers asynchronously.
12. Serialize successful merges.
13. Run integration gate.
14. Repeat.

Do not poll aggressively when event-driven observation is available.

## 3. Worker state machine

``` text
PROVISIONING
→ CONTEXT_LOADING
→ IMPLEMENTING
→ FOCUSED_VALIDATION
→ CHECKPOINTING
→ REVIEWING
→ [FIXING → VALIDATING → CHECKPOINTING → REVIEWING]*
→ READY_TO_MERGE
→ MERGING
→ INTEGRATION_VALIDATION
→ COMPLETE
```

Any state can transition to `PAUSED`, `FAILED`, `CANCELLED`, or
`NEEDS_HUMAN` under defined conditions.

## 4. Review loop

Review requires a durable diff visible from a fixed point. Vela
records: - `fixed_point_sha`, - `review_head_sha`.

If implementation tooling performs review before commit and its review
mechanism only sees committed `HEAD`, Vela must compensate by creating
a checkpoint before authoritative review. The orchestration layer---not
wishful assumptions about a skill---owns this invariant.

Default maximum review iterations: configurable; recommended 3. On
exhaustion, do not declare success.

## 5. Merge lane

Implementation can be parallel; integration merge is serialized: 1. lock
integration merge lane, 2. update worker branch with current integration
tip, 3. rerun conflict-sensitive gate if branch changed, 4. merge, 5.
run integration validation, 6. persist result, 7. release lane.

A failed integration gate blocks new scheduling until
repaired/reverted/decided.

## 6. Stop All

Stop All: - marks run `STOPPING`, - prevents new workers, - sends
graceful cancellation, - waits bounded time, - escalates process
termination if necessary, - records final known process/session
states, - preserves branches/worktrees, - does not reset source, - marks
run `PAUSED` or `NEEDS_HUMAN`.

## 7. Idempotency

Every external operation gets an operation ID and reconciliation logic.
After restart Vela checks whether the intended effect already occurred
before repeating it.

# Approval Sub-State

During any agent-driven state, the worker may enter `WAITING_APPROVAL`.
The Approval Watchdog can infer this from a provider event or from a
bounded lack-of-progress check combined with approval-surface detection.

Flow:

``` text
worker active
→ approval detected
→ normalize/fingerprint
→ policy classify
   ├─ ALLOW → deliver → verify progress → resume prior state
   ├─ ASK   → NEEDS_HUMAN
   └─ DENY  → reject/hold → policy outcome
```

If the same effective request reappears beyond the configured threshold
without meaningful progress, transition to `APPROVAL_STALLED`.

Approval handling must not advance ticket state by itself; it only
unblocks the underlying operation.

# Antigravity-First Execution

For v1 production runs, scheduler worker execution resolves through the Antigravity adapter. Scheduler state remains provider-neutral so future adapters can be introduced without rewriting orchestration.

A generic scheduler test passing against a fake adapter does not by itself prove v1 runtime readiness; Antigravity end-to-end acceptance evidence is required.
