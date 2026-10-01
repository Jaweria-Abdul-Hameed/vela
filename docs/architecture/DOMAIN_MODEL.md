# Domain Model

## Aggregate: Project

Fields include identity, repository path, remote metadata, default
branch, integration policy, command profile, documentation manifest,
tracker config, UI preferences, **trust state** (`UNTRUSTED` default,
`TRUSTED`; see ADR-013), and **promotion mode** (ADR-014).

## Aggregate: BuildRun

A single attempt to implement a selected scope/spec. Contains immutable
run ID, project snapshot reference, graph snapshot, integration base
(`run_base_sha`), current state (see `ORCHESTRATION_ENGINE.md`),
`integration_health`, timestamps, policy snapshot.

## Entity: Ticket

Contains external/local identity, requirement links, blockers,
acceptance criteria, predicted impact, risk, current orchestration
status.

## Entity: Worker

Binds exactly one ticket execution attempt to one agent profile,
session, branch, worktree, base commit, and lifecycle.

## Entity: ExecutionProfile

A supported authenticated execution identity/capability set. Vela
stores identifiers and availability metadata, never raw passwords.

## Entity: ReviewCycle

Fixed point, reviewed head, findings, dispositions, iteration, exit
status.

## Entity: TestRun

Command, scope, exit code, duration, output summary, artifact pointers.

## Value objects

-   CommitSha
-   BranchName
-   WorktreePath
-   TicketId
-   RunId
-   WorkerId
-   CommandPolicyDecision
-   PredictedWriteSet
-   DependencyEdge
-   RiskClass
-   ReviewSeverity
-   GateResult

## State models

The worker state machine and run state machine are normative in
`ORCHESTRATION_ENGINE.md`. Ticket and UI node states are projections of
worker/run state and are never set independently.

### Ticket state projection

`PENDING → READY → RUNNING → TESTING → REVIEWING ↔ FIXING → MERGING → DONE`

Exceptional: `BLOCKED`, `RATE_LIMITED`, `PAUSED`, `FAILED`,
`REVIEW_STALLED`, `NEEDS_HUMAN`, `CANCELLED`.

| Ticket state | Derived from |
|---|---|
| `PENDING` | Not every explicit blocker is `DONE`; no worker. |
| `BLOCKED` | At least one unmet blocker is itself `FAILED`, `NEEDS_HUMAN`, or `CANCELLED`. |
| `READY` | All blockers `DONE`, integration healthy, eligible for the frontier, no worker yet. |
| `RUNNING` | Worker in `PROVISIONING`, `CONTEXT_LOADING`, `IMPLEMENTING`. |
| `TESTING` | Worker in `FOCUSED_VALIDATION` or `VALIDATING`. |
| `REVIEWING` | Worker in `CHECKPOINTING` or `REVIEWING`. |
| `FIXING` | Worker in `FIXING` or `CONFLICT_RESOLUTION`. |
| `MERGING` | Worker in `READY_TO_MERGE`, `MERGING`, or `INTEGRATION_VALIDATION`. |
| `DONE` | Worker `COMPLETE` (merge validated). |
| `PAUSED` | Worker `PAUSED` (any reason except `CAPACITY`). |
| `RATE_LIMITED` | Worker `PAUSED` with reason `CAPACITY`. |
| `NEEDS_HUMAN` | Worker `NEEDS_HUMAN` (any kind except `REVIEW_STALLED`). |
| `REVIEW_STALLED` | Worker `NEEDS_HUMAN` with kind `REVIEW_STALLED`. |
| `FAILED` / `CANCELLED` | Worker `FAILED` / `CANCELLED`. |

`WAITING_APPROVAL` is not a ticket state; it is a sub-indicator on the
ticket's current state. `APPROVAL_STALLED` is a `NEEDS_HUMAN` kind and
projects to `NEEDS_HUMAN`.

### UI node mapping

UI node states in `UI_UX_SPEC.md` use the ticket state names above (pending,
ready, running, testing, reviewing, fixing, merging, done, blocked, paused,
rate-limited, failed, needs human, cancelled). Sub-indicators (waiting approval,
review stalled, approval stalled) are secondary marks on the node. Cancelled and
paused nodes use the settled/quiet treatment defined in the design system.

State transitions require explicit causes; UI must never derive
authoritative state from animation.

# Approval Domain Objects

Add: - `ApprovalRequest`: provider/session/worker correlation,
normalized operation signature, requested capability, observed source,
**evidence tier** (ADR-009). - `ApprovalDecision`: `ALLOW | ASK | DENY`,
policy rule, reason, expiry/scope. - `ApprovalDeliveryAttempt`: adapter,
target identity, result, timestamp. - `ApprovalFingerprint`:
stable-enough signature for loop detection without persisting secrets. -
worker intervention kind `APPROVAL_STALLED` (see `ORCHESTRATION_ENGINE.md`
section 3.2). - `PolicyRule`: ordered, scoped, user-created or default
rule with scope, expiry, and creating action. - `ProjectTrust`: project,
repository identity, state, timestamp, consent text version. -
`ApprovalAutomationConsent`: consent record for UI automation (ADR-010).

# Runtime Provider Semantics

Where domain objects contain provider/runtime identity, `antigravity` is the expected production runtime value for v1. The model may remain extensible, but code must not imply that every modeled provider value is currently supported.
