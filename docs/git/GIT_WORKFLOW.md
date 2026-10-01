# Git Workflow

## Branch model

Example: - base: `main` - integration: `vela/run-<short-id>` - worker:
`vela/<run-id>/issue-<id>-<slug>`

## Worktrees

Every concurrent worker has a dedicated worktree. Worktree paths live
under an Vela-managed directory outside the primary checkout where
practical.

Before start: - fetch if policy permits, - verify base SHA, - verify
clean worktree, - create branch/worktree, - record path and SHA.

## Merge process

A dedicated merger operation serializes merges into integration. Before
merge, worker branch incorporates the latest integration state if
required. Conflicts are resolved only when mechanical and
policy-approved; semantic conflicts escalate.

## Push

Push after successful worker review by default. Integration push cadence
is configurable but should happen frequently enough for disaster
recovery.

## Protected actions

Require explicit policy/human approval: - force push, - deleting
unmerged remote branches, - rewriting integration history, - hard reset
with uncommitted unknown work, - cleaning files outside Vela-managed
worktrees.

## Dirty primary checkout

Vela must not destroy or absorb unrelated user work. Preflight warns
and can still use separate worktrees when safe, but operations affecting
the primary checkout require care.

## Commit messages

Use coherent issue-referenced commits. The exact convention is
repository-configurable.
