# ADR-011: Deterministic Serialized Merge Lane with a Dedicated Integration Worktree

## Status

Accepted (2026-10-02).

## Context

The Prompt 2 audit (SA-07) found: merge versus rebase undefined; rebasing a pushed worker branch
implies force-push, which documents treated inconsistently; the merger sometimes described as an
agent and sometimes as an operation; "mechanical conflict" undefined; and no working tree for the
integration branch.

## Decision

1. **Deterministic lane.** The integration merge lane is Vela code (a merge-lane service using
   `GitAdapter`), not an agent. It is serialized: exactly one holder at a time.
2. **Dedicated integration worktree.** Each run has a Vela-managed integration worktree on the
   integration branch, outside the primary checkout. Merges and the integration gate run there.
   The primary checkout is never used for integration work.
3. **Merge, not rebase.** The worker branch is brought up to date by merging the current
   integration tip into the worker branch (an ordinary additive commit, pushable without force).
   The worker enters integration with a non-fast-forward merge commit that carries ticket
   trailers. Pushed branches are never rebased. History is preserved; any squash happens at PR
   promotion by human choice, not in the lane.
4. **No routine force-push.** Force-push is not part of any Vela workflow. If ever needed, it
   requires explicit human approval regardless of branch.
5. **Conflicts.** A merge that Git completes without conflicts proceeds to validation. A textual
   conflict, or any uncertainty, is never resolved automatically by the lane. It escalates to a
   conflict-resolution attempt: a reasoning agent session works in the worker's worktree to merge
   the integration tip into the worker branch; the result is checkpointed, fully re-reviewed and
   validated before the lane retries. Conflicts touching high-risk surfaces (see
   `PARALLELIZATION.md` hard blockers), uncertain or semantic conflicts, or failed attempts
   escalate to a human (`NEEDS_HUMAN`, kind `MERGE_CONFLICT`).
6. **Integration validation failure.** If validation fails after a merge, and the merge commit has
   not been published, the lane discards that unpublished merge by resetting the Vela-managed
   integration worktree to the recorded `integration_before` SHA (only when the worktree is clean
   and the SHA matches the record), journals the discard, and returns the ticket to `FIXING` with
   the failure as a blocking finding. If the discard is impossible or ambiguous, integration
   health becomes `UNKNOWN` or `UNHEALTHY` and the run requires human attention.
7. **Publish after validation.** The integration branch is pushed only after validation succeeds,
   so published integration history is always validated.

## Scope note

This ADR governs Vela's **runtime** merge lane. The playbook's "integration merger" prompt
(`VELA_MASTER_BUILD_PLAYBOOK.md`, Prompt 12) and its "update/rebase/merge according to repository
policy" wording describe the **build-time** workflow used while external agents build Vela before
Vela exists; they do not define Vela's runtime behavior.

## Consequences

- The merger prompt contract in `PROMPT_CONTRACTS.md` becomes the contract for the conflict-
  resolution attempt only.
- `GIT_WORKFLOW.md`, `ORCHESTRATION_ENGINE.md` and `PARALLELIZATION.md` are updated accordingly.
- Throughput is bounded by serialized validation; that is an accepted trade-off for correctness.
