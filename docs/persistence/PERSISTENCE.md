# Persistence Model

Use SQLite with explicit schema migrations (ADR-002, Accepted).

Core tables/entities: - projects - project_trust - build_runs - graph_snapshots -
tickets - dependency_edges - workers - worktrees - execution_profiles - operations -
commands - test_runs - review_cycles - review_findings -
git_checkpoints - event_journal - human_interventions - settings -
approval_requests - approval_decisions - approval_delivery_attempts -
approval_fingerprints - policy_rules - approval_automation_consent -
skill_installations - adapter_capability_snapshots - notifications -
state_store_backups

## Requirements

-   transactional state transitions,
-   foreign keys,
-   durable operation IDs,
-   schema versioning,
-   timestamps in UTC,
-   no secret values,
-   bounded/log-rotation strategy for large stdout/stderr,
-   exportable run summary.

## Authority between state and journal

-   The Vela core is the single writer of the store (a dedicated writer thread, `ADR-017`); the UI reads through the core; journal
    events are published to subscribers only after the transaction commits, so the UI is never ahead of durable state.
-   Every state transition is applied as **one transaction** that updates the materialized state
    and appends its journal event(s) together. Journal events carry a monotonically increasing
    sequence number; each materialized row records the `last_event_seq` that produced it.
-   **Materialized state is authoritative for current state; the journal is authoritative for
    history.** If startup detects a mismatch (a state row ahead of or behind the journal tip), that
    is a store-integrity failure handled by `RECOVERY.md` (State-Store Failure), never silently
    repaired by guessing which side is right.
-   The journal vocabulary is closed and listed in `SYSTEM_ARCHITECTURE.md` section 4.

## Crash consistency

Before starting a non-idempotent external action, persist intent. After
completion, persist observed result. Startup reconciliation handles the
gap.

## Integrity, backup, and migration

-   Run an integrity check at startup.
-   Create a backup before any migration; on migration failure restore it and refuse to run newer
    logic against the old store. Rotate backups on clean shutdown (latest three retained).
-   Corruption or loss follows `RECOVERY.md` (State-Store Failure): restore from backup, else
    rebuild a read-only inventory from Git, and never auto-resume from reconstructed state.
-   The store holds no secrets, so backups and diagnostic exports do not need secret handling
    beyond the redaction rules in `OBSERVABILITY.md`.

## Conversation identifiers (ADR-020)

`workers.conversation_id`, `review_cycles.reviewer_conversation_id`, and the id of any in-scope substantive analyst conversation (ADR-020 decision 1a) are persisted and journaled; they are
used by recovery-resume and shown in the inspector. They are identifiers only; no Antigravity credential or transcript content is stored.
