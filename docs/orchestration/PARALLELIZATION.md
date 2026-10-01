# Parallelization and Conflict Forecasting

## Goal

Maximize useful concurrency without allowing two individually-correct
agents to create an incoherent combined codebase.

## Hard blockers

Tickets cannot run concurrently if any of these are known: - explicit
blocking edge, - same migration/schema object with incompatible or
unordered changes, - same generated artifact/source-of-truth file, - one
changes an API/type/contract consumed by the other before that contract
is merged, - one renames/moves files the other expects, - one changes
shared build/configuration semantics the other relies upon, - both
require mutually exclusive infrastructure/local resources, - repository
policy marks the area serialized.

## Evidence inputs

1.  Explicit tracker blockers.
2.  Ticket text and acceptance criteria.
3.  Predicted write sets.
4.  Imports/dependency graph.
5.  ownership/module boundaries.
6.  schema/migration impact.
7.  API/event/type contract impact.
8.  package/config/lockfile impact.
9.  recent integration changes.
10. semantic agent analysis.

## Decision rule

Concurrency requires:
`dependency-safe AND write-safe AND contract-safe AND integration-safe`.

Semantic confidence alone never overrides a deterministic hard conflict.

When uncertain, schedule sequentially.

## Predicted write set

Before execution, analysis should classify likely changes: - exact files
if predictable, - directory/module, - shared contracts, - database
schema, - migrations, - lockfiles, - CI/config, - tests/fixtures.

Predictions are hints, not permissions. Runtime file-change observation
can trigger re-evaluation.

## Dynamic conflict detection

If two workers unexpectedly begin touching overlapping high-risk
surfaces, Vela may: - allow if changes are clearly independent and
policy permits, - pause one worker, - let both finish but
serialize/rebase with enhanced validation, - escalate.

## UI representation

Do not present an AI-generated numeric probability as objective truth.
UI may display labels such as: - Safe by explicit separation - Safe with
caution - Serialized by dependency - Serialized by shared contract -
Unknown --- sequential

The user may inspect the evidence behind the decision.

## Tracer bullets

Prefer tickets that cut vertically through required layers while
remaining independently verifiable. This reduces long-lived cross-ticket
assumptions and improves fresh-session execution.
