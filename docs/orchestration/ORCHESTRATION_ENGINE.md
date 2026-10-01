# Orchestration Engine

This document is the normative source for run and worker state machines. `DOMAIN_MODEL.md`
defines the ticket-state projection and the UI node mapping derived from it.

## 1. Run state machine

```text
CREATED → PREFLIGHT → ANALYZING → GRAPH_READY → AWAITING_START → STARTING → RUNNING
RUNNING → FINAL_REVIEW → FINAL_VALIDATION → COMPLETED
FINAL_REVIEW ⇄ FINAL_FIXING ⇄ FINAL_VALIDATION
(any non-terminal) → STOPPING → PAUSED | NEEDS_HUMAN
RUNNING ⇄ PAUSED        RUNNING ⇄ NEEDS_HUMAN
Terminal: COMPLETED, FAILED, CANCELLED
```

### 1.1 Transition table

| From | To | Cause |
|---|---|---|
| `CREATED` | `PREFLIGHT` | Run requested. |
| `PREFLIGHT` | `PREFLIGHT` | Re-run after remediation (a result containing `BLOCK` cannot advance). |
| `PREFLIGHT` | `ANALYZING` | No `BLOCK` result remains; required `WARN`s acknowledged. |
| `ANALYZING` | `GRAPH_READY` | Graph and parallelization snapshot built and valid. |
| `ANALYZING` | `NEEDS_HUMAN` | Invalid graph (cycle, malformed issues); kind `GRAPH_INVALID`. Returns to `ANALYZING` after correction. |
| `GRAPH_READY` | `AWAITING_START` | User approves the graph (approval recorded in the snapshot). |
| `AWAITING_START` | `STARTING` | User presses Build; repository must be `TRUSTED` (ADR-013). |
| `STARTING` | `RUNNING` | Integration branch and integration worktree created; `run_base_sha` recorded. |
| `STARTING` | `NEEDS_HUMAN` / `FAILED` | Creation failed and cannot be retried safely. |
| `RUNNING` | `PAUSED` | Manual pause; recovery completed awaiting Resume; all remaining work blocked by capacity or by loss of an interactive session. |
| `RUNNING` | `NEEDS_HUMAN` | Run-level human need: integration health `UNKNOWN`/`UNHEALTHY`, policy violation, external divergence, or an unresolved run-level intervention. |
| `PAUSED` / `NEEDS_HUMAN` | `RUNNING` | Resume or recorded decision, after successful reconciliation. The run returns to the state recorded in `resume_state` (including `FINAL_*` states). |
| `RUNNING` | `FINAL_REVIEW` | Every in-scope ticket is `DONE` (or explicitly excluded by the user) and integration health is `HEALTHY`. |
| `FINAL_REVIEW` | `FINAL_FIXING` | Final review findings fail the exit policy. |
| `FINAL_FIXING` | `FINAL_REVIEW` | Fixes validated and checkpointed (bounded by the review iteration cap). |
| `FINAL_REVIEW` | `FINAL_VALIDATION` | Exit policy passes. |
| `FINAL_VALIDATION` | `FINAL_FIXING` | Failures attributable to the integrated work. |
| `FINAL_VALIDATION` | `COMPLETED` | Validation passes and promotion step completes per ADR-014. |
| any non-terminal | `STOPPING` | Stop All (section 6). |
| `STOPPING` | `PAUSED` / `NEEDS_HUMAN` | Quiescent (all processes stopped or reconciled) / reconciliation needs a decision. |
| `PAUSED` / `NEEDS_HUMAN` | `CANCELLED` | User cancels the run. Branches, worktrees and commits are preserved. |
| any non-terminal | `FAILED` | Unrecoverable internal error, recorded as the `FAILED` reason (for example `STATE_STORE_UNRECOVERABLE`). Artifacts are preserved. |

`PAUSED` carries a `reason` (`MANUAL`, `STOP_ALL`, `RECOVERED_AWAITING_RESUME`, `CAPACITY`,
`INTERACTIVE_SESSION_UNAVAILABLE`, `AWAITING_RECOVERY_CONFIRMATION`). `NEEDS_HUMAN` carries a
`kind` (see section 3.2). Both carry `resume_state`.

### 1.2 Integration health (orthogonal to run state)

`integration_health` is `HEALTHY`, `UNKNOWN`, or `UNHEALTHY`.

- `UNKNOWN`: after a merge until validation completes, after an interrupted lane operation, or
  after an unexplained ref change.
- `UNHEALTHY`: validation failed and the merge could not be safely discarded, or published
  integration history is found broken.
- While not `HEALTHY`: no new worker starts and no worker may enter the merge lane. Workers
  already running continue in their isolated worktrees up to `READY_TO_MERGE` and wait there.
  If health cannot be restored by the lane's defined recovery (section 5), the run becomes
  `NEEDS_HUMAN` (kind `INTEGRATION_UNKNOWN` or `INTEGRATION_UNHEALTHY`).

## 2. Scheduling cycle

On every scheduling tick caused by a material event:

1.  Reconcile external state.
2.  Confirm integration branch health (`integration_health`).
3.  Mark newly satisfied explicit blockers.
4.  Recompute ready frontier.
5.  Apply capacity constraints (including Antigravity-declared concurrency; see `ADAPTERS.md`).
6.  Apply pairwise/group parallel-safety constraints.
7.  Select maximal safe set up to concurrency limit.
8.  Create worker records transactionally.
9.  Provision worktrees (provisioning contract, `GIT_WORKFLOW.md`).
10. Start fresh sessions.
11. Observe workers asynchronously.
12. Serialize successful merges.
13. Run integration gate.
14. Repeat.

Do not poll aggressively when event-driven observation is available.

## 3. Worker state machine

```text
PROVISIONING
→ CONTEXT_LOADING
→ IMPLEMENTING
→ FOCUSED_VALIDATION
→ CHECKPOINTING
→ REVIEWING
→ [FIXING → VALIDATING → CHECKPOINTING → REVIEWING]*
→ READY_TO_MERGE
→ MERGING                    (merge lane; may detour through CONFLICT_RESOLUTION)
→ INTEGRATION_VALIDATION
→ COMPLETE
```

### 3.1 Exceptional states

Any state can transition to `PAUSED`, `FAILED`, `CANCELLED`, or `NEEDS_HUMAN` under defined
conditions. `CONFLICT_RESOLUTION` is entered from `MERGING` on a textual conflict (ADR-011) and
returns through `CHECKPOINTING → REVIEWING → READY_TO_MERGE`.

`WAITING_APPROVAL` is an overlay sub-state of any agent-driven state, not a replacement for it:
the worker retains its `resume_state`.

### 3.2 Intervention kinds

`NEEDS_HUMAN` is the single human-gated worker state. Its `kind` is persisted and displayed:

| Kind | Entered when |
|---|---|
| `AMBIGUITY` | Ticket contradictory, impossible, or underspecified in an externally visible way. |
| `REVIEW_STALLED` | Review loop cap reached, or reviews oscillate (`REVIEW_PROTOCOL.md`). |
| `APPROVAL_ASK` | Policy returned `ASK` for a request. |
| `APPROVAL_STALLED` | Repeated equivalent approval with no progress past threshold. |
| `APPROVAL_UNDELIVERABLE` | A decision cannot be delivered by any permitted tier. |
| `POLICY_DENIED` | A `DENY` is repeated beyond threshold, or the denied operation is required by the ticket. |
| `POLICY_VIOLATION` | Detective control fired (ADR-009 section 4). |
| `MERGE_CONFLICT` | Conflict not resolvable under ADR-011 section 5. |
| `INTEGRATION_REGRESSION` | Integration validation failures exceed the fix cap. |
| `AUTH_REQUIRED` | Credentials or provider authentication needed. |
| `EXTERNAL_DIVERGENCE` | Refs, worktree, or issues changed outside Vela. |
| `OTHER` | Any other defined human-gated condition. |

`REVIEW_STALLED` and `APPROVAL_STALLED` keep their historical names as intervention kinds. Where
other documents say "transition to `NEEDS_HUMAN` or `REVIEW_STALLED`", the result is
`NEEDS_HUMAN` with kind `REVIEW_STALLED`. `RATE_LIMITED` is a worker `PAUSED` with reason
`CAPACITY`.

## 4. Review loop

Review requires a durable diff visible from a fixed point. Vela records:

- `fixed_point_sha`: the worker's base integration SHA, constant across all iterations of the
  ticket, so every review sees the cumulative diff `fixed_point_sha..review_head_sha`;
- `review_head_sha`: the checkpoint commit reviewed in this iteration.

Ownership of the checkpoint invariant is defined by ADR-015: Vela verifies a clean worktree and a
non-empty, resolvable `fixed_point_sha..HEAD`, creating the checkpoint commit itself if the worker
left uncommitted changes. The orchestration layer, not assumptions about a skill, owns this
invariant. Authoritative review runs in a fresh reviewer session (`REVIEW_PROTOCOL.md`).

Default maximum review iterations: configurable; recommended and default 3. On exhaustion, do not
declare success: the worker becomes `NEEDS_HUMAN` (kind `REVIEW_STALLED`).

## 5. Merge lane

Implementation can be parallel; integration is serialized, deterministic (Vela code, not an
agent), and runs in the dedicated integration worktree (ADR-011):

1.  lock the integration merge lane (exactly one holder; lock persisted with the operation ID),
2.  verify integration health is `HEALTHY` and record `integration_before`,
3.  update the worker branch by merging the current integration tip into it (never rebase a pushed
    branch; no force-push),
4.  rerun the conflict-sensitive gate if the worker branch changed,
5.  merge the worker branch into integration with a non-fast-forward merge commit and ticket
    trailers,
6.  run integration validation,
7.  on success: persist result and push integration (per policy); on failure: discard the
    unpublished merge by resetting the clean integration worktree to `integration_before`, journal
    it, and return the worker to `FIXING` with the failure as a blocking finding (counts against
    the iteration cap; exhaustion gives `NEEDS_HUMAN` kind `INTEGRATION_REGRESSION`),
8.  release the lane.

If the discard is impossible, ambiguous, or the process dies mid-operation, integration health
becomes `UNKNOWN` and startup reconciliation (`RECOVERY.md`) decides; otherwise the run becomes
`NEEDS_HUMAN`.

A textual conflict at step 3 or 5 sends the worker to `CONFLICT_RESOLUTION` (ADR-011 section 5);
the lane is released while the resolution attempt runs and re-entered afterwards.

## 6. Stop All

Stop All:

- marks run `STOPPING`,
- prevents new workers,
- sends graceful cancellation to sessions and processes that Vela created (identities recorded at
  creation); it never terminates or alters Antigravity processes or sessions Vela did not create,
- waits a bounded time (default 30 seconds, configurable),
- escalates to termination of Vela-owned process trees if necessary,
- lets an in-progress merge or push reach its next safe point within the bounded wait: a merge
  either completes validation or is discarded to `integration_before`; a push either completes or
  is abandoned (a Git push is atomic per ref). If neither can be established, integration health
  becomes `UNKNOWN` and reconciliation is required before resume,
- records final known process/session states,
- preserves branches/worktrees, does not reset source,
- marks run `PAUSED` (reason `STOP_ALL`) or `NEEDS_HUMAN`.

Stop All is reachable from the in-app run control, the tray menu (when background operation is
enabled), and an optional configurable global shortcut. Confirmation is inline and non-modal and
may be configured off; it never blocks keyboard access.

## 7. Idempotency

Every external operation gets an operation ID and reconciliation logic. After restart Vela checks
whether the intended effect already occurred before repeating it.

# Approval Sub-State

During any agent-driven state, the worker may enter `WAITING_APPROVAL`. The Approval Watchdog can
infer this from a provider event or from a bounded lack-of-progress check combined with
approval-surface detection.

Flow:

```text
worker active
→ approval detected
→ normalize/fingerprint (evidence rules, ADR-009)
→ policy classify
   ├─ ALLOW → deliver → verify progress → resume prior state
   ├─ ASK   → NEEDS_HUMAN (kind APPROVAL_ASK); decision delivered after the human decides
   └─ DENY  → deliver a refusal where a deny control exists, journal, resume prior state;
              repeated DENY, or a denied operation the ticket requires → NEEDS_HUMAN (POLICY_DENIED);
              if no permitted tier can deliver the refusal → NEEDS_HUMAN (APPROVAL_UNDELIVERABLE)
```

If the same effective request reappears beyond the configured threshold without meaningful
progress (defined in `APPROVAL_BROKER.md`), transition to `NEEDS_HUMAN` with kind
`APPROVAL_STALLED`.

If the interactive session needed for UI-automation delivery is unavailable (ADR-012), the worker
is `PAUSED` with reason `INTERACTIVE_SESSION_UNAVAILABLE`; unaffected workers continue.

Approval handling must not advance ticket state by itself; it only unblocks the underlying
operation.

# Antigravity-First Execution

For v1 production runs, scheduler worker execution resolves through the Antigravity adapter.
Scheduler state remains provider-neutral so future adapters can be introduced without rewriting
orchestration.

A generic scheduler test passing against a fake adapter does not by itself prove v1 runtime
readiness; Antigravity end-to-end acceptance evidence is required.
