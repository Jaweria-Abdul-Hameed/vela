# Integration Adapter Contracts

## AgentAdapter

Must expose capability discovery and lifecycle operations. Core
orchestration must not know whether execution occurs through Antigravity
CLI, SDK, daemon, or another provider.

### Antigravity Required Capability Contract

This table is the **requirement** on the production `AntigravityAdapter`. Whether the installed
Antigravity actually provides each capability is an external fact to be verified by Prompt 4 and
confirmed by the real-environment suite; it is not assumed here. Vela feature-detects every
capability at runtime and never infers it from version strings alone.

| ID | Capability | Class | If absent |
|---|---|---|---|
| CAP-01 | Discover installation, version, and the capability set | Required | `BLOCK`: no run can start. |
| CAP-02 | Start a fresh isolated session bound to a given worktree/workspace | Required | `BLOCK`. |
| CAP-03 | Deliver the task and context pointers, and invoke installed skills/commands (`/implement`, `/code-review`) | Required | `BLOCK`. |
| CAP-04 | Observe session lifecycle: at minimum completion and error signals; structured command/test/approval events are preferred | Required (minimum signals) | `BLOCK` if no completion/error signal; if only minimal signals exist, evidence comes from Git/process observation and `ASK` is the default for approvals. |
| CAP-05 | Cancel a session Vela created | Required | `BLOCK` for Autonomous; Supervised requires user acknowledgement. |
| CAP-06 | Query session status | Required | Derived from CAP-04 where possible; otherwise `BLOCK`. |
| CAP-07 | Declare or discover the maximum concurrent sessions | Required | Vela assumes one session at a time and reports it. |
| CAP-08 | Expose a session/process/window identity usable for correlation | Required for approval automation | UI-automation delivery is unavailable; approvals needing it become interventions. |
| CAP-09 | Report rate-limit/capacity conditions | Required for FR-024 | Capacity handling degrades to pause-on-unclassified-failure; reported in preflight. |
| CAP-10 | Resume or recover a session | Optional | Recovery uses a new session at the last checkpoint (`RECOVERY.md`). |
| CAP-11 | Inspect (and where supported configure) the native permission posture | Required for Autonomous | Posture is `UNKNOWN` (ADR-009): Autonomous blocked. |

Rules:

- A missing Required capability yields a concrete preflight `BLOCK` that names the capability. Vela
  does not silently select another provider.
- This specification does **not** authorize substituting UI automation for session control or any
  other capability beyond the approval-delivery fallback (ADR-007, ADR-010). Doing so would
  require an approved specification change.
- If Prompt 4 shows a Required capability is unavailable in the real product, that is a
  specification-change decision (ADR), not an adapter workaround.
- Fake/test adapters implement the same contract so the scheduler is tested deterministically.

## GitAdapter

Responsibilities: - inspect repository, - status/cleanliness, -
create/delete worktrees safely, - create branches, - resolve refs, -
commit, - fetch/push, - merge, - detect conflicts, - calculate
diffs/merge-base.

Destructive operations require policy authorization.

Additional contracts:

- **Untrusted repositories:** every Git invocation on an `UNTRUSTED` repository neutralizes
  repository-controlled code execution (hooks path, fsmonitor, external diff/textconv/filter
  drivers, and similar configuration) (ADR-013).
- **Per-worktree operation lock:** Vela serializes its own Git operations per worktree and never
  commits while a worker session is mid-turn (ADR-015).
- **No force-push and no rebase of pushed branches** are provided as routine operations (ADR-011).
  Any force-push requires explicit human approval.
- **Integration worktree:** the adapter manages a dedicated integration worktree per run and
  exposes merge, discard-unpublished-merge (reset to a recorded SHA of a clean Vela-managed
  worktree), and conflict detection operations to the merge lane.

## IssueTrackerAdapter

Initial implementations: 1. Local Markdown tracker. 2. GitHub Issues.

Operations: - list/read ticket, - read blockers, - update
status/labels, - comment evidence, - close/resolve according to
policy, - create/link PR where supported.

## ProcessAdapter

Runs configured commands with: - working directory, - environment
allowlist, - timeout, - cancellation, - streamed stdout/stderr, -
redaction, - exit metadata.

Commands defined by an `UNTRUSTED` repository are never run (ADR-013). Process trees created by
Vela are tracked so that cancellation and Stop All terminate the whole tree and reconciliation
can find orphans.

## NotificationAdapter

Desktop notifications only in initial release. Future adapters may
include email/chat but must not be core dependencies.

# ApprovalDeliveryAdapter

Implementations may include: 1. native provider permission/approval
interface; 2. Windows accessibility/UI Automation control adapter; 3.
guarded visual/pixel compatibility adapter only when robust control
discovery is unavailable.

Tiers 2 and 3 are used only when the user has enabled guarded UI automation (ADR-010).

Required operations: - capability detection, - locate expected approval
surface, - extract enough non-secret metadata for correlation/policy
(under the evidence-binding rules of ADR-009), - deliver allow/deny when
supported, - verify resulting state transition, - cancel, - report
confidence/failure.

The adapter must never approve a control merely because a button labeled
"Approve" exists somewhere on screen.

# Required v1 Agent Adapter

`AgentAdapter` is the stable core boundary. For v1, `AntigravityAdapter` is the required production implementation.

Fake/test adapters remain required for deterministic tests. Additional production provider adapters are not required for v1 unless explicitly scoped.

Antigravity-specific capability differences should be represented through capability discovery/typed adapter behavior rather than leaking arbitrary provider checks throughout the scheduler.
