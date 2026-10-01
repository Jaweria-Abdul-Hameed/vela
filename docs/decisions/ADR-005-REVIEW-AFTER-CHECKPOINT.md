# ADR-005: Authoritative Review Requires a Durable Checkpoint

## Status

Accepted.

## Decision

Vela records a fixed point and creates a commit checkpoint before
authoritative fixed-point review.

## Rationale

A review mechanism based on `git diff <fixed-point>...HEAD` cannot see
uncommitted/staged changes. The orchestration layer must ensure the
review target actually exists in Git history.

## Consequence

Review fixes create subsequent checkpoint commits before re-review.
