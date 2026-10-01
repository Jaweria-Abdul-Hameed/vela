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
require mutually exclusive infrastructure/local resources (declared as
resource keys in the project profile; two tickets or test commands claiming
the same key are serialized), - repository policy marks the area serialized.

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
serialize/merge with enhanced validation (never rebase a pushed branch; ADR-011), - escalate.

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

## Dependency analyst (semantic analysis)

The semantic analysis in the evidence inputs is performed by a **dependency analyst task**:

-   it runs once during run state `ANALYZING`, through `AgentAdapter` as a read-only analysis
    session (no writes, no repository scripts, no commands beyond read operations), and counts
    against Antigravity capacity like any other session;
-   its output follows the analyst contract in `PROMPT_CONTRACTS.md` and is validated: ticket IDs
    exist, predicted paths are well-formed, and deterministic checks (explicit blockers, lockfile,
    schema/migration path rules, declared resource keys) are recomputed independently;
-   deterministic rules and hard blockers always apply regardless of analyst output; analyst
    output may only add hazards or evidence, never remove a deterministic hard blocker;
-   the validated result, the analyst/prompt version, and its inputs are stored in the graph
    snapshot, which is the reproducible input to scheduling; the analyst is not re-run mid-run
    (runtime re-evaluation uses observed file changes, not new analysis);
-   if the analyst fails or returns invalid output after one correction retry, affected pairs are
    labelled "Unknown — sequential" and scheduled sequentially.
