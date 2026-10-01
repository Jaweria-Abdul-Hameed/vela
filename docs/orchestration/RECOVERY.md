# Recovery and Reconciliation

## Failure classes

-   Vela UI crash
-   core process crash
-   agent process crash
-   command timeout
-   laptop sleep/restart
-   network loss
-   GitHub unavailable
-   push failure
-   partial merge
-   merge conflict
-   test process orphan
-   review session interruption
-   execution profile capacity exhaustion
-   corrupted/missing worktree
-   repository modified externally

## Startup reconciliation

For every nonterminal run: 1. inspect persisted state, 2. inspect
repository/worktrees/branches, 3. inspect child process/session status
where possible, 4. compare recorded commit SHAs with actual refs, 5.
verify integration branch, 6. verify remote state if network available,
7. classify discrepancies, 8. resume only idempotent/safe operations.

## Never guess after divergence

Examples requiring human action or explicit recovery policy: -
integration ref moved unexpectedly, - branch history rewritten
externally, - worktree contains unrecorded modifications from unknown
actor, - merge appears partially completed, - credentials changed, -
spec/issues materially changed during run.

## Checkpoints

Durable checkpoints are created: - after ticket implementation before
authoritative review, - after review fixes, - before integration
merge, - after integration merge, - before finalization.

## Laptop sleep

Sleep is not failure. On wake: - detect elapsed gap, - reconcile
processes, - do not assume commands remained alive, - re-check
locks/network/auth, - continue only after state confirmation.

# Approval Recovery

Persist enough approval state to avoid blindly clicking after restart.
On reconciliation: - determine whether the original request still
exists, - confirm worker/session identity, - re-evaluate time-sensitive
policy decisions, - never replay a stale UI click, - preserve loop
counters/fingerprints for the run, - escalate if the UI/provider state
cannot be correlated confidently.

# Antigravity Recovery Requirement

V1 recovery is incomplete unless it can reconcile the Antigravity execution state Vela actually uses. Generic persisted scheduler recovery and fake-adapter tests are necessary but insufficient; real Antigravity process/session divergence must have a defined reconciliation path or a safe `NEEDS_HUMAN` outcome.
