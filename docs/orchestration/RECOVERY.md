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
-   Vela state store corrupt, missing, or failing migration
-   interactive session unavailable (locked, disconnected, secure desktop)
-   upgrade or update attempted during an active run

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

# Resume Semantics ("documented recovery operation")

Fresh context per ticket (`AGENT_PROTOCOL.md`) is preserved by this single documented recovery
operation, **recovery-resume**:

-   it starts a **new session** on the existing worker branch and worktree at the last verified
    checkpoint SHA, with a recovery envelope (ticket, `fixed_point_sha`, checkpoint SHA, and a
    journal summary of completed steps). Implementation is never repeated from scratch when a
    checkpoint exists;
-   if the adapter provides CAP-10 (session resume) and the session identity is verified, the same
    session may be resumed instead; otherwise a new session is used;
-   fix-loop iterations continue in the ticket's worker session; authoritative review always runs
    in a separate fresh reviewer session (`REVIEW_PROTOCOL.md`);
-   migration to another execution profile is permitted only at a checkpoint boundary using
    recovery-resume (never mid-turn), and only through supported profile semantics.

# Reboot and Restart Continuation

After a reboot or Vela restart, reconciliation always runs first. Continuation follows the
persisted `recovery_continuation` setting (ADR-012): `ask` (default) leaves the run `PAUSED`
(reason `AWAITING_RECOVERY_CONFIRMATION`) until the user resumes; `auto_safe` resumes
automatically only after reconciliation reports no unresolved discrepancy and only for safe,
idempotent steps. UI-automation-dependent workers additionally require an interactive session
(ADR-012); otherwise they stay `PAUSED` with reason `INTERACTIVE_SESSION_UNAVAILABLE`.

# Interactive Session Loss

A locked, disconnected, or secure desktop is not bypassed. Affected workers pause; unaffected
workers continue. On return, reconcile: re-detect the prompt, re-verify correlation and freshness,
and re-evaluate policy before any delivery (see Approval Recovery).

# State-Store Failure

-   At startup Vela runs an integrity check on its SQLite store.
-   Before applying any migration Vela creates a backup of the store; on migration failure it
    restores the backup, refuses to start newer logic on the old store, and reports
    `MigrationFailed`. Backups are also rotated on clean shutdown (latest three retained).
-   On corruption or loss, Vela first attempts restore from the latest backup. If that fails, it
    rebuilds a read-only **inventory** from Git: `vela/*` branches, Vela-managed worktrees, and
    checkpoint/merge trailers (run and ticket identifiers). A reconstructed inventory never
    auto-resumes a run: affected runs are `NEEDS_HUMAN`, or `FAILED` (reason
    `STATE_STORE_UNRECOVERABLE`) if the user chooses to abandon them. Branches and worktrees are
    preserved.

# Updates During a Run

An application update is never applied while a run is active. Updates are deferred until the run
is `PAUSED` (quiescent) or terminal and require an explicit user action; the pre-update state
backup above is mandatory.
