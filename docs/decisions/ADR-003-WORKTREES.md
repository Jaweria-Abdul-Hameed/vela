# ADR-003: Worktree Isolation for Parallel Tickets

## Status

Accepted product invariant.

## Decision

Concurrent implementation tickets use distinct Git branches and
worktrees.

## Why

Multiple agent sessions sharing one working directory/index/HEAD can
interfere even when editing different files. Worktrees isolate working
directories and branch heads.

## Caveat

Repository-wide resources such as stash refs remain shared; workers must
not use stash as a concurrency primitive.
