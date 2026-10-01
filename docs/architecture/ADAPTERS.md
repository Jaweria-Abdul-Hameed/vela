# Integration Adapter Contracts

## AgentAdapter

Must expose capability discovery and lifecycle operations. Core
orchestration must not know whether execution occurs through Antigravity
CLI, SDK, daemon, or another provider.

## GitAdapter

Responsibilities: - inspect repository, - status/cleanliness, -
create/delete worktrees safely, - create branches, - resolve refs, -
commit, - fetch/push, - merge, - detect conflicts, - calculate
diffs/merge-base.

Destructive operations require policy authorization.

## IssueTrackerAdapter

Initial implementations: 1. Local Markdown tracker. 2. GitHub Issues.

Operations: - list/read ticket, - read blockers, - update
status/labels, - comment evidence, - close/resolve according to
policy, - create/link PR where supported.

## ProcessAdapter

Runs configured commands with: - working directory, - environment
allowlist, - timeout, - cancellation, - streamed stdout/stderr, -
redaction, - exit metadata.

## NotificationAdapter

Desktop notifications only in initial release. Future adapters may
include email/chat but must not be core dependencies.

# ApprovalDeliveryAdapter

Implementations may include: 1. native provider permission/approval
interface; 2. Windows accessibility/UI Automation control adapter; 3.
guarded visual/pixel compatibility adapter only when robust control
discovery is unavailable.

Required operations: - capability detection, - locate expected approval
surface, - extract enough non-secret metadata for correlation/policy, -
deliver allow/deny when supported, - verify resulting state
transition, - cancel, - report confidence/failure.

The adapter must never approve a control merely because a button labeled
"Approve" exists somewhere on screen.

# Required v1 Agent Adapter

`AgentAdapter` is the stable core boundary. For v1, `AntigravityAdapter` is the required production implementation.

Fake/test adapters remain required for deterministic tests. Additional production provider adapters are not required for v1 unless explicitly scoped.

Antigravity-specific capability differences should be represented through capability discovery/typed adapter behavior rather than leaking arbitrary provider checks throughout the scheduler.
