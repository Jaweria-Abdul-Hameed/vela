# GitHub Workflow

GitHub is an optional tracker/remote implementation, not a core-domain
dependency.

## Issue structure

Every issue should include: - objective, - user-visible outcome, -
requirements, - implementation constraints, - dependencies/blockers, -
acceptance criteria, - test/evidence requirements, - relevant docs, -
explicit non-goals.

Issue and comment text is untrusted input to Vela (ADR-013).

## Labels

Suggested: `type:feature`, `type:infra`, `type:test`, `type:docs`,
`state:ready`, `state:blocked`, `state:needs-human`, `risk:low`,
`risk:medium`, `risk:high`, `area:ui`, `area:orchestrator`, `area:git`,
`area:agent`, `area:persistence`.

## PRs and promotion

Default promotion with a remote (ADR-014): validated integration branch -> pull request ->
**human-controlled merge** into the default branch. Vela opens a draft PR once the integration
branch has content, marks it ready for review only after final validation passes, and does not
merge it. Worker PRs are optional; direct controlled merges into integration happen in the
deterministic merge lane (ADR-011).

Promotion mode is a per-project policy: `pr` (default), `local_only`, or an explicitly opted-in mode
that merges the default branch automatically (requires per-project opt-in with a visible warning).
Without a remote, completion yields a validated local integration branch recorded as ready for human
promotion.

PR evidence should summarize: - scope, - issue closure links, - test
evidence, - review evidence, - merge risk, - screenshots/video for
meaningful UI changes. Closure links are included in the PR so issues close on the human merge, not
before.

## Remote failures

GitHub outage must not corrupt local progress. Queue remote metadata
updates and retry later. Retried updates are idempotent: before re-posting a comment, label, or PR,
Vela checks whether the intended effect already exists (operation ID plus remote lookup).
