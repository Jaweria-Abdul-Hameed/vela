# Crash-injection recovery matrix (R03)

`tests/e2e/tests/crash_matrix.rs` arms every declared `FaultPoint` in a child process running a real scenario over the real Git and process adapters, kills the child at the point (abort, no unwinding), restarts it against the state it left, and verifies the recovered state. A standing completeness test fails when any declared point has no entry, so a point added later cannot be forgotten. The matrix owns every entry; tickets that declare points do not write into it.

## What the restart verifies
- the state store opens and passes its integrity check (no journal gap, no foreign or dangling run pointers);
- reconciliation completes (no failure), and what it allows is resumed through the same path the application uses (`Recovery::reconcile`, `StopService::resume`, `RunDriver::drive`);
- no `index.lock` or `HEAD.lock` and no half merge is left behind, and every `vela/*` branch is preserved;
- no ticket is merged twice (no duplicated destructive or non-idempotent action).

## Scenarios
| Scenario | Points |
|---|---|
| flow (two dependent tickets, built, merged, finalized, pushed to a bare remote) | persistence before/after commit, git checkpoint before/after commit, git push before/after, build before branch / after worktree / before running, worker after provisioning / implementing / before checkpoint / after checkpoint, lane before merge / after merge / after validation, final before push / after push / after promotion / during cleanup |
| discard (an integration gate that fails on the merge result) | lane after discard |
| approval (one allowed request through the broker) | approval decision persisted, approval delivery attempted |
| stop (busy lane) | stop bounded wait expired |
| stop (unfinished merge on record) | stop discard decision |

`SelfTest` belongs to the harness itself and is not a product transition.
