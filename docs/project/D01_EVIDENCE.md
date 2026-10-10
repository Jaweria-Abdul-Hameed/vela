# D01 Evidence (Prompt 10, execution log)

Ticket: `docs/issues/graph/tickets/D01.md` (State machines: run, worker, and ticket projection with exhaustive transition tests).
Base: `49dffb3db3e8127e86e9d580fc30d8fe20ec8c42` (`main` equal to `origin/main`, clean; F01, F02, F03 integrated; D01's only blocker F02 done).
Worker: branch `d01/state-machines`, worktree `%LOCALAPPDATA%\Vela\worktrees\d01`.

## What D01 delivered (`crates/vela-domain/src/state/**`, pure, no new dependency, no `lib.rs` or manifest change)

| File | Content |
|---|---|
| `common.rs` | `PausedReason` (6), `InterventionKind` (13 of section 3.2 plus run-level `GRAPH_INVALID`, `INTEGRATION_UNKNOWN`, `INTEGRATION_UNHEALTHY`), `Cause` (`System`, `UserDecision`). |
| `error.rs` | `TransitionError`: `Illegal`, `Terminal`, `ResumeStateMismatch`, `KindNotAllowed`, `ExplicitDecisionRequired`, `ProviderPolicyBlockHeld`, `HumanGateHeld`, `ReasonNotAllowed`, `IntegrationNotHealthy` and the overlay errors. |
| `run.rs` | `RunPhase`, `RunState` (`Active`, `Stopping` with the kept gate, `Paused`, `NeedsHuman` with `resume_state`; terminal states), `transition(to, cause, health)` implementing every row of section 1.1, `IntegrationHealth` and its gates (section 1.2). |
| `worker.rs` | `WorkerPhase`, `WorkerState`, `transition(to, cause, health)` for sections 3, 3.1 and 5, and `WorkerStatus` with the validated `WAITING_APPROVAL` overlay and `provision(health)`. |
| `projection.rs` | `TicketState` projection of worker state (every phase, pause reason, intervention kind), `project_unstarted` (`PENDING`/`BLOCKED`/`READY`), `UiNodeState` (the 14 node states) and `UiNode` with sub-indicators (waiting approval, review stalled, approval stalled). |

No ts-rs DTO is exported, so no contract regeneration was needed.

## Acceptance criteria

1. Every section 1.1 and worker-machine transition is implemented and every other is rejected: `run::tests::every_transition_pair_matches_the_specification_table` and `worker::tests::every_transition_pair_matches_the_specification_table` compare all (from, to) pairs over the full enumerated state space (403 run states, 243 worker states) with an independently written oracle of the specification; `every_specification_row_is_accepted` names each table row.
2. Projection covers every worker state and intervention kind: `projection::tests` (every phase, every pause reason, all 16 kinds, UI mapping, every ticket state reachable).
3. `PROVIDER_POLICY_BLOCK` (worker and run): `provider_policy_block_exits_only_by_explicit_resolve_or_cancel` enumerates every target and both causes. Nothing resumes work automatically: the exits are `Active(resume_state)` by user decision, `CANCELLED` by user decision, `FAILED` (terminal, never resumes work) and, for a run, a `STOPPING` that keeps the gate (DR-D01-3). It also projects to `NEEDS_HUMAN`, never `RATE_LIMITED`.
4. Property tests (proptest): random walks over legal successors only, for the run machine, the worker machine (with the overlay) and the worker projection, never leave the enumerated state space, never leave a gate except through the explicit exits, and always project to a ticket and UI node state.

## Decisions

Fourteen interpretations are recorded as non-blocking decision requests in `D01_DECISION_REQUESTS.md` (DR-D01-1 to DR-D01-14), with follow-ups for the specification owners. The notable ones: the section 1 diagram draws `FINAL_FIXING` to `FINAL_VALIDATION` but table 1.1 does not (the table wins); the three run-level kinds are missing from section 3.2; leaving a human gate needs an explicit user decision; a held block keeps its gate through Stop All.

## Review

- Iteration 1 (head `717c16a`, base `49dffb3`): 10 findings. Fixed: failed integration validation returns to `FIXING` (spec section 5 step 7); a pause can escalate to a gate; held block can still fail and keep its hold through Stop All (instead of the over-strict rejection); resume out of a gate needs an explicit user decision; overlay cannot be built inconsistently (private fields, validated deserialization); `STOP_ALL` pause reason only through `STOPPING`; health gates enforced by the public API; dead `is_run_level` removed; serde wording corrected; decisions extended. The weak property walks were replaced by walks over legal successors.
- Iteration 2 (head `9f908bd`): 4 mediums and 6 lows. Fixed: cancelling a gate needs a user decision (run and worker); Stop All keeps any human gate (`Stopping.gate`), one variant, typed `HumanGateHeld`; a gate can escalate to `PROVIDER_POLICY_BLOCK`; integration health is a required argument of the public `transition` (and of `READY_TO_MERGE` to `MERGING`, and worker provisioning). Dispositioned as documented non-blocking: transitive `BLOCKED` (DR-D01-6), unvalidated state deserialization (DR-D01-10), `ANALYZING` gate rule only on the direct edge (DR-D01-1, DR-D01-2), small duplication.
- Iteration 3 (head `b5aedc1`, the last permitted): 9 findings, no severities given by the reviewer. Classified by the Engineering exit policy: none is an acceptance-criterion miss, a defect against explicit specification text, a security or data issue, or a build/test failure, so none is classified blocking. Dispositions (all documentation only, no code change after this review): user-only forward edges (DR-D01-11); a gated `STOPPING` cannot record a different need (DR-D01-12); `VALIDATING` failure, merge-lane re-gate and health on resume are edges the specification does not draw (DR-D01-13); worker `STOP_ALL` and the `ANALYZING` route asymmetry (DR-D01-14); transitive `BLOCKED` (DR-D01-6); an automatic-reason pause can be cancelled by the system (DR-D01-9, DR-D01-11); duplicated helpers (style, nonblocking). The merged tree equals the reviewed head except for this evidence and the decision file. The findings closest to the line (the health gate not applying to a resume into `MERGING`, forward edges accepting a system cause, a gated `STOPPING` not recording a new kind) are listed so the owners can overrule the classification.

## Validation (CI only; local Rust is blocked by Smart App Control)

- Checkpoint head `cdc996c`, run `38070892545`: success. After review 1, head `9f908bd`, run `38073859347`: success. After review 2, head `2c51fc8`, run `38075028783` (documentation-only head `b5aedc1`, run `38075269846`, reviewed in iteration 3): Rust (format, clippy `-D warnings`, build, test, structure) success, TypeScript success; `vela-domain` unit tests 134 passed, including 38 state tests (3 proptest properties).
- Three pushes failed only `cargo fmt --check` (formatter output applied) and one failed a clippy-visible compile error in a `prop_assert!` (fixed).
