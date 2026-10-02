# Product Acceptance Tests

## AT-001 Unattended single ticket

Given a prepared repository and one ready ticket, when Build is pressed,
Vela completes implementation, validation, checkpoint, review,
push/merge policy, and marks the ticket done without ordinary command
approvals.

## AT-002 Safe parallel pair

Two independent tickets run in separate worktrees concurrently and merge
serially without lost changes.

## AT-003 Unsafe pair

Two tickets modifying a shared schema are not run concurrently; UI
explains why.

## AT-004 Review fix loop

A seeded review defect produces finding → fix → tests → checkpoint →
re-review → pass.

## AT-005 Crash recovery

Terminate Vela after checkpoint but before recorded worker completion.
Relaunch detects the commit and resumes without duplicate
implementation.

## AT-006 Stop All

During multiple workers, Stop All prevents new work, cancels safely,
preserves worktrees/commits, and allows later resume.

## AT-007 Capacity interruption

A worker receives a rate-limit/cooldown condition. Other safe work
continues; blocked worker preserves state and resumes/migrates only
through supported profile semantics.

## AT-008 Visual identity

Home and graph views match the design principles: black spatial canvas,
reactive dots, violet/blue/cyan ambient light, restrained glass, no
conventional dashboard dominance.

## AT-009 Efficiency

Minimizing the application suspends rendering while orchestration remains correct. Pass criteria
are the numeric budgets in `PERFORMANCE_BUDGET.md` (zero animation-frame callbacks and zero GPU
frame submissions after the suspension interval, and bounded UI-process CPU while minimized),
measured on the recorded reference hardware, with orchestration events continuing to be journaled.

## AT-010 Human ambiguity

An intentionally contradictory ticket stops in `NEEDS_HUMAN` with a
precise decision request and no speculative product change.

# Approval Acceptance Tests

## AT-011 Persistent Antigravity prompt

Given a `TRUSTED` repository, an effective posture of `MEETS`, guarded UI automation enabled by the
user (ADR-010), and Antigravity still showing a routine approval prompt for a command in the
confirmed command profile, when Vela has classified the underlying project-scoped action as
allowed, Vela delivers approval through the best supported adapter and the worker continues without
user presence (while an interactive desktop is available; see AT-017).

## AT-012 Unsafe approval

Given a destructive or credential-sensitive request, Vela does not
auto-approve it even if an approval button is detectable.

## AT-013 Approval loop

Given Antigravity repeatedly asks for the same approval without
progress, Vela stops automatic delivery after the configured threshold
and surfaces `APPROVAL_STALLED`.

## AT-014 Wrong window protection

Given another application exposes an "Approve" control, Vela never
activates it because process/window/session correlation fails.

# Antigravity-First Release Gate

V1 acceptance must include at least one complete representative workflow through the real/supported Antigravity integration path, including execution, validation/review flow, observable state, and recovery/approval behavior applicable to the environment.

A fake adapter or alternate development agent cannot substitute for this release gate.

# Acceptance Tests Added by the Prompt 3 Specification Fixes

## AT-015 Policy sovereignty

Given the installed environment's native posture is `DOES_NOT_MEET` or `UNKNOWN`, Autonomous mode is
blocked in preflight and Supervised mode requires the recorded acknowledgement and banner. Given a
posture of `MEETS`, a `DENY`-class operation is classified by Vela's policy before execution and is
not executed.

## AT-016 Opt-in UI automation

On a fresh install guarded UI automation is disabled; enabling it at onboarding persists across
restarts and runs without re-requesting consent; revoking it stops UIA/visual delivery for new
deliveries; each change is journaled.

## AT-017 Locked or disconnected session

When the interactive desktop becomes unavailable while a worker waits on an approval, that worker
pauses with reason `INTERACTIVE_SESSION_UNAVAILABLE`, unaffected workers continue, Vela does not try
to bypass the lock, and when the session returns Vela re-detects the prompt, re-verifies correlation,
re-evaluates policy, and never replays a stale click. Display-off alone causes no failure.

## AT-018 Merge lane

Two safe parallel tickets merge through the deterministic lane into the dedicated integration
worktree with merge commits and no force-push; an injected textual conflict is not auto-resolved but
escalates to a conflict-resolution attempt or `NEEDS_HUMAN`; an integration validation failure
discards the unpublished merge and returns the ticket to `FIXING`.

## AT-019 Background, tray, and reboot

With background operation enabled, closing the UI during a run does not stop orchestration and the
tray offers reopen, status, and Stop All. With it disabled, closing offers a safe pause. After reboot
Vela reconciles first and resumes only per the persisted `recovery_continuation` setting.

## AT-020 Untrusted repository

A newly imported repository is `UNTRUSTED`: preflight and graph analysis work as data-only, no
repository-controlled executable (scripts, hooks, provisioning, installs) runs, and Build is
refused until the user explicitly trusts it. A script merely named `test` outside the confirmed
command profile is `ASK`.

## AT-021 Promotion

With a remote, a completed run leaves a validated integration branch and a PR ready for review, and
Vela does not merge the default branch. Without a remote, completion yields a validated local
integration branch marked ready for human promotion. Final review uses `run_base_sha`.

## AT-022 State-store failure

A simulated corrupt state store restores from backup or produces a read-only Git-derived inventory
with the affected runs `NEEDS_HUMAN`; a failed migration restores the backup; none of these auto-
resumes a run or loses branches and worktrees.

## AT-023 Notifications

A human-required event, a terminal failure, and run completion each produce a desktop notification
(also while the window is closed with background operation enabled).

## AT-024 Update during a run

An available update is deferred while a run is active; applying it requires an explicit action after
the run is quiescent, with a state backup first.

## Acceptance-test traceability

| AT | Requirements |
|---|---|
| AT-001 | FR-009..FR-018, FR-044 |
| AT-002 | FR-008, FR-010, FR-017 |
| AT-003 | FR-007, FR-008 |
| AT-004 | FR-013, FR-014, FR-015 |
| AT-005 | FR-015, FR-019, FR-022 |
| AT-006 | FR-021, FR-022 |
| AT-007 | FR-024 |
| AT-008 | FR-025, FR-026 |
| AT-009 | FR-027 |
| AT-010 | FR-020 |
| AT-011 | FR-031, FR-033, FR-035, FR-038..FR-040 |
| AT-012 | FR-030, FR-031, FR-039 |
| AT-013 | FR-034 |
| AT-014 | FR-033, FR-039 |
| AT-015 | FR-036, FR-038 |
| AT-016 | FR-040 |
| AT-017 | FR-033, FR-041 |
| AT-018 | FR-017, FR-018 |
| AT-019 | FR-021, FR-042 |
| AT-020 | FR-030, FR-043 |
| AT-021 | FR-016, FR-045 |
| AT-022 | FR-019, FR-046 |
| AT-023 | FR-028 |
| AT-024 | FR-047 |
| AT-025 | FR-049 |
| AT-026 | FR-050 |

## AT-025 Authentication ownership

With Antigravity unauthenticated, preflight and a run report `AUTH_REQUIRED` and ask the user to authenticate
through Antigravity's supported flow; Vela does not automate sign-in and never opens, reads, or copies
Antigravity credential-store entries or authentication material (verified by file and credential-store access
auditing in the test harness). An API key present in the environment does not silently change the runtime.
(Requirement: FR-049.)

## AT-026 Conversation visibility in Antigravity Desktop

For a run with at least two tickets, each in-scope conversation (each worker's, each authoritative reviewer's, and any substantive analyst's; not
incidental probes or deterministic computation, ADR-020 decision 1a) is fresh, its identifier is recorded and shown in
the inspector, and the user can find and open that conversation in Antigravity Desktop 2.x. The test records, for conversations created
through the primary headless surface, whether they appear in Desktop, under which project, and whether they open; if any cannot be
shown, the test fails the requirement rather than passing on Vela's own rendering. (Requirement: FR-050; capability CAP-12.)
