# D01 Evidence (Prompt 10, execution log)

Ticket: `docs/issues/graph/tickets/D01.md` (State machines: run, worker, and ticket projection with exhaustive transition tests).
Base: `49dffb3db3e8127e86e9d580fc30d8fe20ec8c42` (`main` equal to `origin/main`, clean; F01, F02, F03 integrated; D01's only blocker F02 done).
Worker: branch `d01/state-machines`, worktree `%LOCALAPPDATA%\Vela\worktrees\d01`.

## What D01 delivered (`crates/vela-domain/src/state/**`, pure, no new dependency, no `lib.rs` or manifest change)

| File | Content |
|---|---|
| `common.rs` | `PausedReason` (6), `InterventionKind` (13 of section 3.2 plus run-level `GRAPH_INVALID`, `INTEGRATION_UNKNOWN`, `INTEGRATION_UNHEALTHY`), `Cause` (`System`, `UserDecision`). |
| `error.rs` | `TransitionError`: `Illegal`, `Terminal`, `ResumeStateMismatch`, `KindNotAllowed`, `ExplicitDecisionRequired`, `ProviderPolicyBlockHeld`, and the overlay errors. |
| `run.rs` | `RunPhase`, `RunState` (`Active`, `Stopping`, `Paused`, `NeedsHuman` with `resume_state`; terminal states), `transition` implementing every row of section 1.1, `IntegrationHealth` and its gates (section 1.2). |
| `worker.rs` | `WorkerPhase`, `WorkerState`, `transition` for section 3 and 3.1 (including `CONFLICT_RESOLUTION` via `MERGING`, returning through `CHECKPOINTING`), and `WorkerStatus` with the `WAITING_APPROVAL` overlay. |
| `projection.rs` | `TicketState` projection of worker state (every phase, pause reason, intervention kind), `project_unstarted` (`PENDING`/`BLOCKED`/`READY`), `UiNodeState` (the 14 node states) and `UiNode` with sub-indicators (waiting approval, review stalled, approval stalled). |

No ts-rs DTO is exported, so no contract regeneration was needed.

## Acceptance criteria

1. Every section 1.1 and worker-machine transition is implemented and every other is rejected: `run::tests::every_transition_pair_matches_the_specification_table` and `worker::tests::every_transition_pair_matches_the_specification_table` compare all (from, to) pairs over the full enumerated state space (243 run states, 255 worker states) with an independently written oracle of the specification; `every_specification_row_is_accepted` names each table row.
2. Projection covers every worker state and intervention kind: `projection::tests` (every phase, every pause reason, all 16 kinds, UI mapping, every ticket state reachable).
3. `PROVIDER_POLICY_BLOCK` (worker and run): `provider_policy_block_exits_only_by_explicit_resolve_or_cancel` enumerates all targets and both causes; the only exits are `Active(resume_state)` by `UserDecision` and `CANCELLED`. It also projects to `NEEDS_HUMAN`, never `RATE_LIMITED`.
4. Property tests (proptest): random legal walks of the run machine, of the worker machine (with the overlay), and of the worker projection never leave the enumerated state space, never leave a held block except through the explicit exits, and always project to a ticket and UI node state.

## Decisions

Eight interpretations are recorded as non-blocking decision requests in `D01_DECISION_REQUESTS.md` (DR-D01-1 to DR-D01-8). The notable ones: the section 1 diagram draws `FINAL_FIXING` to `FINAL_VALIDATION` but table 1.1 does not (the table wins); the three run-level kinds are missing from section 3.2; `PROVIDER_POLICY_BLOCK` rejects `FAILED` and `STOPPING` because the acceptance criterion says the only exits are resolve or cancel.

## Validation (CI only; local Rust is blocked by Smart App Control)

- Checkpoint: formatting-fixed head `cdc996cc9b8bc30cd91c331a00468c54dbba2e2a`, run `38070892545`: Rust (format, clippy `-D warnings`, build, test, structure) success, TypeScript success; `vela-domain` unit tests 123 passed (27 new state tests, including 3 proptest properties).
- The first push (`344868f`) failed only `cargo fmt --check`; the formatter output was applied.
