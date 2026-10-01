# Domain Model

## Aggregate: Project

Fields include identity, repository path, remote metadata, default
branch, integration policy, command profile, documentation manifest,
tracker config, UI preferences.

## Aggregate: BuildRun

A single attempt to implement a selected scope/spec. Contains immutable
run ID, project snapshot reference, graph snapshot, integration base,
current state, timestamps, policy snapshot.

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

## Ticket states

`PENDING → READY → RUNNING → TESTING → REVIEWING ↔ FIXING → MERGING → DONE`

Exceptional: `BLOCKED`, `RATE_LIMITED`, `PAUSED`, `FAILED`,
`REVIEW_STALLED`, `NEEDS_HUMAN`, `CANCELLED`.

State transitions require explicit causes; UI must never derive
authoritative state from animation.

# Approval Domain Objects

Add: - `ApprovalRequest`: provider/session/worker correlation,
normalized operation signature, requested capability, observed source. -
`ApprovalDecision`: `ALLOW | ASK | DENY`, policy rule, reason,
expiry/scope. - `ApprovalDeliveryAttempt`: adapter, target identity,
result, timestamp. - `ApprovalFingerprint`: stable-enough signature for
loop detection without persisting secrets. - worker exceptional state
`APPROVAL_STALLED`.

# Runtime Provider Semantics

Where domain objects contain provider/runtime identity, `antigravity` is the expected production runtime value for v1. The model may remain extensible, but code must not imply that every modeled provider value is currently supported.
