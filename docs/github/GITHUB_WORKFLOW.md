# GitHub Workflow

GitHub is an optional tracker/remote implementation, not a core-domain
dependency.

## Issue structure

Every issue should include: - objective, - user-visible outcome, -
requirements, - implementation constraints, - dependencies/blockers, -
acceptance criteria, - test/evidence requirements, - relevant docs, -
explicit non-goals.

## Labels

Suggested: `type:feature`, `type:infra`, `type:test`, `type:docs`,
`state:ready`, `state:blocked`, `state:needs-human`, `risk:low`,
`risk:medium`, `risk:high`, `area:ui`, `area:orchestrator`, `area:git`,
`area:agent`, `area:persistence`.

## PRs

For whole-run integration, create a draft PR after the integration
branch has content if repository policy prefers PRs. Worker PRs are
optional; direct controlled merges to integration may be faster for
local autonomous execution.

PR evidence should summarize: - scope, - issue closure links, - test
evidence, - review evidence, - merge risk, - screenshots/video for
meaningful UI changes.

## Remote failures

GitHub outage must not corrupt local progress. Queue remote metadata
updates and retry later.
