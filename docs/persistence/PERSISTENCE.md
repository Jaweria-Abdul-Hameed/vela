# Persistence Model

Use SQLite with explicit schema migrations.

Core tables/entities: - projects - build_runs - graph_snapshots -
tickets - dependency_edges - workers - execution_profiles - operations -
commands - test_runs - review_cycles - review_findings -
git_checkpoints - event_journal - human_interventions - settings

## Requirements

-   transactional state transitions,
-   foreign keys,
-   durable operation IDs,
-   schema versioning,
-   timestamps in UTC,
-   no secret values,
-   bounded/log-rotation strategy for large stdout/stderr,
-   exportable run summary.

## Crash consistency

Before starting a non-idempotent external action, persist intent. After
completion, persist observed result. Startup reconciliation handles the
gap.
