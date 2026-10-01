# Git Workflow

## Branch model

Example: - base: `main` - integration: `vela/run-<short-id>` - worker:
`vela/<run-id>/issue-<id>-<slug>`

Branch slugs are deterministically truncated with a hash suffix and lower-cased so that ref and
path names stay bounded and unique on case-insensitive Windows filesystems.

## Base selection

The integration branch is cut from an explicit SHA recorded at graph approval as `run_base_sha`.
When a remote exists and policy permits, Vela fetches first and shows the user the base as
"default branch @ SHA". If the local default branch differs from the remote default branch (ahead,
behind, or diverged), preflight warns and the user must acknowledge the recorded base; Vela never
silently chooses one.

## Worktrees

Every concurrent worker has a dedicated worktree. Each run also has a
dedicated **integration worktree** on the integration branch (ADR-011). Worktrees live under a
Vela-managed root outside the primary checkout. The default root is under the per-user local
application data directory, never inside the repository and never inside a cloud-synced folder
(preflight blocks a worktree root under a known sync provider).

Before start: - fetch if policy permits, - verify base SHA, - verify
clean worktree, - create branch/worktree, - record path and SHA.

### Provisioning contract

A fresh worktree contains only tracked files. Provisioning (requires a `TRUSTED` repository,
ADR-013) runs the project's user-confirmed provisioning commands (for example dependency install)
in the new worktree, classified by policy as repository-controlled executables. Rules:

-   provisioning commands are discovered by reading manifests as data and are confirmed by the
    user at project configuration time, never invented at run time;
-   Vela never copies secrets or untracked files (for example `.env`) from the primary checkout
    automatically. A project may declare an explicit allowlist of files to copy, confirmed by the
    user; values are never logged;
-   shared caches and build output directories must be declared per project as safe-to-share or
    per-worktree, so parallel workers do not contend on one build directory;
-   exclusive resources (network ports, local databases, other singletons) are declared as resource
    keys in the project profile; tickets and test commands that claim the same key are
    serialized (`PARALLELIZATION.md`);
-   a provisioning failure is a `VALIDATION_FAILURE` before the worker starts, not a ticket
    failure.

### Cleanup

Worktrees of `DONE` tickets are retained until the run reaches a terminal state or the user
approves cleanup, because the merge lane and recovery may need them. Worktrees of failed,
`NEEDS_HUMAN`, or paused workers are always retained. Cleanup removes only worktrees under the
Vela-managed root, only when clean, with bounded retry/backoff for file locks; the process holding
a lock is reported rather than force-killed outside Vela-owned process trees.

## Windows specifics

-   Long paths: Vela computes the worst-case path length for the worktree layout before starting
    and reports it in preflight.
-   Case-insensitivity: branch and slug generation lowercases and checks for case-only collisions.
-   Line endings: Vela does not change the user's `core.autocrlf`; it reports it so checkpoints do
    not create whole-file churn.
-   File locks (antivirus, editors, agents): cleanup retries with backoff and reports holders.

## Merge process

Merging is deterministic Vela code in the dedicated integration worktree (ADR-011); no agent
performs integration merges. The worker branch is brought up to date by **merging** the current
integration tip into it; pushed branches are never rebased. The worker enters integration with a
non-fast-forward merge commit with ticket trailers. A merge that Git completes cleanly proceeds
to integration validation. A textual conflict or any uncertainty is never resolved automatically
by the lane: it escalates to a reasoning-agent conflict-resolution attempt (reviewed and validated)
or to a human (ADR-011 section 5). Semantic conflicts escalate.

## Checkpoints and commits

Vela owns the checkpoint invariant (ADR-015): before authoritative review the worktree must be
clean and `fixed_point_sha..HEAD` non-empty; Vela creates the checkpoint commit itself if the
worker left uncommitted changes, with run and ticket trailers, only while the worker session is
quiescent and under the per-worktree Git operation lock. Repository Git hooks run normally in
trusted repositories and are not bypassed by default; a hook failure is a `VALIDATION_FAILURE`.

## Push

For Vela-supervised workers, **Vela pushes** the worker branch after the worker's review passes
(Supervised mode asks first); the worker does not push (ADR-015). Integration is pushed only after
integration validation succeeds. Integration push cadence is configurable but should happen
frequently enough for disaster recovery. Build-time agents working on Vela itself follow
`AGENTS.md`.

## Protected actions

Require explicit policy/human approval: - **any** force push, - deleting
unmerged remote branches, - rewriting integration history, - hard reset with
uncommitted unknown work, - cleaning files outside Vela-managed worktrees.

The single permitted discard is the lane's reset of an unpublished merge in the clean Vela-managed
integration worktree to a recorded `integration_before` SHA (ADR-011), which is journaled.

## Dirty primary checkout

Vela must not destroy or absorb unrelated user work. Preflight warns
and can still use separate worktrees. No Vela operation modifies the primary checkout's working
tree, index, or branch; integration work happens only in the integration worktree.

## Commit messages

Use coherent issue-referenced commits. The exact convention is
repository-configurable.
