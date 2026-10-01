# ADR-015: Ownership of Checkpoints, Push, and Authoritative Review

## Status

Accepted (2026-10-02). Clarifies ADR-005; does not change its decision.

## Context

Prompt 2 finding SA-06: `AGENTS.md` and the playbook have the worker checkpoint and push;
`PRODUCT_SPEC.md` has Vela push; `ORCHESTRATION_ENGINE.md` says orchestration owns the
checkpoint invariant. `/implement` already includes its own review and commit. Git hooks and
index-lock contention with an active agent were unaddressed.

## Decision

1. **Scope of `AGENTS.md`'s push step.** `AGENTS.md` governs agents that build Vela (manual or
   playbook-driven work) and agent conduct inside any worktree. For a worker supervised by the
   Vela runtime, the worker's final push step is **delegated to Vela**: the worker reports
   readiness, and Vela's `GitAdapter` pushes after validating the report. Workers never merge into
   integration (unless merge policy explicitly delegates) and never push other branches.
2. **Checkpoint invariant belongs to Vela.** Before an authoritative review, Vela verifies that the
   worker's worktree is clean and that `fixed_point_sha..HEAD` is non-empty and resolvable. If
   uncommitted changes remain after the worker's implementation turn, Vela creates the checkpoint
   commit itself, with run and ticket trailers. Commits the worker or `/implement` already made
   are normal history; they count as the checkpoint only after Vela verifies the invariant.
3. **Skill-internal review is advisory.** Any review performed inside `/implement` is
   non-authoritative. The authoritative review is the Vela-orchestrated `/code-review` against the
   recorded fixed point, run in a fresh reviewer session distinct from the implementing session.
4. **Mutual exclusion.** Vela holds a per-worktree Git operation lock. It creates checkpoint
   commits only when the worker's session is quiescent (turn finished or paused by Vela); if a
   Git index lock is present it waits with bounded retry and then pauses the worker.
5. **Hooks.** Repository Git hooks run as normal in trusted repositories (ADR-013); Vela does not
   bypass them by default. A hook failure on a checkpoint is a `VALIDATION_FAILURE` that returns
   the worker to `FIXING`.
6. **Supervised mode** asks before push, merge, and promotion; Autonomous mode performs push and
   merge within policy.

## Consequences

- `AGENTS.md`, `AGENT_PROTOCOL.md`, `ORCHESTRATION_ENGINE.md`, `GIT_WORKFLOW.md`,
  `REVIEW_PROTOCOL.md`, and `MATT_POCKOCK_SKILLS.md` are aligned to this ADR.
- A double review (skill-internal and authoritative) is accepted as the cost of an independent
  gate.
