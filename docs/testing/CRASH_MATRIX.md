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

## Results (2026-10-15, local run; the same matrix runs in CI)
| Fault point | Scenario | After restart |
|---|---|---|
| persistence before/after commit | flow | the first transaction is the project import: no run exists yet; the store reopens intact |
| git checkpoint before commit / after commit | flow | the interrupted checkpoint is found by its trailer or redone; the run completes |
| git push before / after | flow | the push is repeated safely; the run completes |
| build before branch / after worktree / before running | flow | the interrupted start is completed from the recorded base; the run completes |
| worker after provisioning / implementing / before checkpoint / after checkpoint | flow | the worker continues from its recorded checkpoint without implementing again; the run completes |
| lane before merge / after merge / after validation / after discard | flow, discard | the integration branch is unverified, so the run is NEEDS_HUMAN (safe by design: a merge is never guessed at) |
| final before push / after push / after promotion / during cleanup | flow | finalization is repeated safely; the run completes |
| approval decision persisted / delivery attempted | approval | the decision is never replayed: at most one delivery is on record |
| stop bounded wait expired / discard decision | stop | the interrupted stop is finished (workers paused, run paused with STOP_ALL), then resumed and completed |

Every row also verified: the store passes its integrity check, no lock file or half merge remains, worker branches are preserved, and no ticket was merged twice.

## Defects the matrix found and this ticket fixed
- A worker that crashed after its checkpoint commit was failed on restart ("changed nothing"); the checkpoint is now adopted from the journal or found by its trailer (the checkpoint recipe had never been registered with a real lookup).
- A run that crashed while starting stayed in STARTING forever; the start is now completed idempotently from the recorded base.
- A run left in STOPPING stayed there; the stop is now finished on restart.
- Stopping the run driver left worker tasks running detached, changing state under the recovery that followed; worker tasks are now cancelled with the driver.
- The application never ran startup reconciliation by itself; it now does, applying the persisted continuation choice.
