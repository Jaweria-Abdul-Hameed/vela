# Component Specifications (Frozen, Prompt 5)

Companion to `IMPLEMENTATION_ARCHITECTURE.md`. Each component lists **INPUTS, OUTPUTS, STATE OWNED, DEPENDENCIES, FAILURE
MODES, TEST STRATEGY**. Tags **[V]/[P]/[U]** follow the verification register (`IMPLEMENTATION_ARCHITECTURE.md` section 15).
"Journal" means the append-only `event_journal`; "state" means the materialized rows.

## C01 Orchestrator (scheduler, run and worker state machines) — `vela-core`

-   **INPUTS:** UI commands (`run.*`, `graph.approve`, `intervention.resolve`, `approval.decide`); worker progress messages;
    merge-lane results; broker decisions; watchdog and timer ticks; reconciliation results.
-   **OUTPUTS:** state transitions and journal events (one transaction each); worker start/cancel requests; merge-lane
    requests; notifications; UI snapshots.
-   **STATE OWNED:** run, ticket, and worker state and `integration_health`; the persisted `resume_state`; the lane lock record.
    It is the **only writer** of orchestration state.
-   **DEPENDENCIES:** `vela-domain` (transition tables, graph, policy), `vela-persistence`, ports (`AgentAdapter`, `GitAdapter`,
    `Notifier`, `PowerManager`, `Clock`).
-   **FAILURE MODES:** invalid transition (rejected and journaled); persistence failure (run `NEEDS_HUMAN`/`FAILED` per
    `ORCHESTRATION_ENGINE.md`); message-queue overflow (bounded channels apply backpressure, never drop); panic in a worker
    task (isolated; worker `FAILED` with evidence).
-   **TEST STRATEGY:** exhaustive transition-table tests; property tests (no illegal transition reachable, frontier never
    contains a blocked ticket); scripted-agent e2e (AT-001..AT-010); fault-point crash tests at every transition.

## C02 Graph and Parallelization Engine — `vela-domain`

-   **INPUTS:** normalized tickets, explicit blockers, validated analyst output, declared resource keys, command profile,
    recent integration changes.
-   **OUTPUTS:** DAG with cycle report, ready frontier, pairwise/group safety verdicts with evidence labels, deterministic layout.
-   **STATE OWNED:** none (pure); results are stored in the graph snapshot by C01.
-   **DEPENDENCIES:** `petgraph`.
-   **FAILURE MODES:** cycle (graph invalid, `GRAPH_INVALID`); malformed metadata (flagged, sequential); analyst absent/invalid
    (pairs "Unknown — sequential"); runtime overlap surprise (re-evaluation message to C01).
-   **TEST STRATEGY:** unit and property tests (conservative: when uncertain, serialize; deterministic hard blockers always win);
    fixture issue sets for the schema-overlap and contract cases; layout determinism test.

## C03 Policy Engine — `vela-domain`

-   **INPUTS:** normalized operation, context (worktree root, repo trust state, command profile, run mode, branch/remote),
    ordered rule set.
-   **OUTPUTS:** `ALLOW | ASK | DENY` with rule id, reason, and scope; generated native allow-rule entries and the hook rule table.
-   **STATE OWNED:** none (pure). Rules are persisted by C11.
-   **DEPENDENCIES:** none. Normalizer grammar: one simple command; anything with pipes, redirection, chaining, subexpressions,
    encoded commands, or unresolved variables is `ASK`; unknown executables are `ASK`; destructive classes are `DENY`.
-   **FAILURE MODES:** unparseable input (`ASK`); conflicting rules (first match by order; conflicts reported at rule creation);
    normalizer ambiguity between PowerShell and `cmd` (`ASK`).
-   **TEST STRATEGY:** table-driven tests per operation class (force push, credential access, outside-root writes, trusted vs
    untrusted repo), PowerShell and `cmd` injection corpus, property test "no input yields ALLOW outside confirmed profile".

## C04 Approval Broker and Watchdog — `vela-core::broker`

-   **INPUTS:** `ApprovalSource` events (hook spool records **[V input fields]**; Desktop card observations when consented);
    worker progress heartbeats from the journal; human decisions.
-   **OUTPUTS:** classification (via C03), delivery requests to the right `ApprovalDeliveryAdapter`, journal events
    (`ApprovalDetected/Classified/DeliveryAttempted/DeliveryVerified/Stalled`), interventions (`APPROVAL_ASK`, `APPROVAL_STALLED`,
    `POLICY_DENIED`, `APPROVAL_UNDELIVERABLE`).
-   **STATE OWNED:** approval requests, decisions, delivery attempts, fingerprints and counters (persisted), watchdog timers.
-   **DEPENDENCIES:** C03, C11, ports (`ApprovalSource`, `ApprovalDeliveryAdapter`, `SessionProbe`, `Clock`).
-   **FAILURE MODES:** weak or truncated evidence (`ASK`); source unavailable (watchdog reports suspected block); delivery
    unverified (one retry then `APPROVAL_UNDELIVERABLE`); repeated fingerprint without progress (`APPROVAL_STALLED`, default 3);
    non-interactive session (worker paused `INTERACTIVE_SESSION_UNAVAILABLE`).
-   **TEST STRATEGY:** fake source/delivery harness; fake approval window; wrong-window, ambiguous-surface, vanished-prompt,
    restart-with-pending-prompt, persistent-option-refusal, and loop-guard tests (AT-011..AT-014, AT-017).

## C05 AntigravityAdapter and `vela-hook` — `vela-adapters::antigravity`, `vela-hook`

-   **INPUTS:** worker/reviewer task envelope (prompt, worktree, profile, rule table, timeouts, conversation id).
-   **OUTPUTS:** a normalized event stream (`SessionStarted`, `StepObserved`, `ToolDenied`, `ApprovalBlocked`, `TurnFinished`,
    `CapacityError`, `AuthRequired`) and final turn result with the parsed `conversation_id`; spool records from the hook.
-   **STATE OWNED:** per-worker profile directory (settings, hook config, rule table, spool, transcript pointers); the child process.
-   **DEPENDENCIES:** `vela-process` (spawn, Job Object), `vela-domain` (rule table types, events), the installed `agy` binary.
-   **DESIGN (ADR-018):** one `agy -p ... --output-format stream-json --print-timeout N` per turn **[V]**; isolated profile via
    `USERPROFILE`/`HOME` **[V]**; allow rules generated from the confirmed profile **[V]**; fail-closed deny hook **[V]**
    (combination with allow **[U]**); ASK via spool + one-time rule + resume **[U]**; capability detection of version, flags
    (`agy --help`), and posture before use **[V]**; reviewer in a separate profile.
-   **FAILURE MODES:** `agy` missing or not authenticated (`CAPABILITY_MISSING`/`AUTH_REQUIRED`); soft-denial (blocked tool
    surfaced, never treated as success); hook failure (tool blocked, worker informed); quota text (`CAPACITY`); background
    self-update changes version (divergence); timeout (explicit, never the default 0s); malformed events (turn failed with evidence).
-   **TEST STRATEGY:** scripted fake `agy` replaying recorded Section S transcripts (soft-denial, deny, crash, resume);
    profile-isolation test (no write to the real `~/.gemini`); credential-access audit test (AT-025); the gated real suite
    reproduces probes P1-P8 and the spikes.

## C06 UIA approval adapter and guarded visual module — `vela-uia` (Windows only)

-   **INPUTS:** Vela-created session identities (process id, window title), the correlated request, delivery decision.
-   **OUTPUTS:** `ApprovalSource` observations (title, assembled command text, working directory, status text, option set) and
    delivery results (selected option, submit invoked, card cleared, progress observed).
-   **STATE OWNED:** warm-up and retry state per window; no persistent state.
-   **DEPENDENCIES:** `windows` crate UIA **[U]** S-UIA-BINDINGS; `SessionProbe`.
-   **RULES (verified, `APPROVAL_BROKER.md` section 19):** warm up and retry while the tree populates; require a visible normal
    window; match by role and label; correlate before acting; select only the single-use allow or the refusal, never persistent
    options; use patterns, never coordinates; fail closed on ambiguity. One delivery verified **[V]**; broader reliability **[P/U]**.
    The visual module is feature-gated, consent-gated, and **[U]**.
-   **FAILURE MODES:** empty tree, minimized window, locked/secure desktop, elevated target, duplicate or missing controls, changed
    labels, card vanishes before delivery, concurrent cards — each fails closed with a typed error (`APPROVAL_SURFACE_NOT_FOUND`,
    `APPROVAL_TARGET_AMBIGUOUS`, `APPROVAL_WINDOW_MISMATCH`, `APPROVAL_SURFACE_UNAVAILABLE`).
-   **TEST STRATEGY:** `tools/fake-approval-window` reproducing roles, labels, option semantics, session-unique ids, delayed tree
    population; DPI/move/resize cases; unrelated-window "Allow" button; real-environment smoke on the user's Desktop (manual).

## C07 GitAdapter and Worktree Manager — `vela-git`

-   **INPUTS:** repo path, refs/SHAs, branch/worktree names, operation id, trust state.
-   **OUTPUTS:** status, refs, diffs, commits, worktrees, fetch/push results, conflict reports.
-   **STATE OWNED:** the per-worktree operation lock; Vela-managed worktree registry (mirrored in C11).
-   **DEPENDENCIES:** `vela-process`; the `git` binary invoked with the verified hardened set (hooks path, fsmonitor, pager, external
    diff, filter and textconv drivers neutralized; `--no-ext-diff --no-textconv`) for untrusted repos and for read operations.
-   **FAILURE MODES:** Git missing or too old; path too long; file lock on cleanup (bounded retry); case collision; submodules (preflight
    warning; multi-checkout of a superproject unsupported); dirty primary checkout (never touched); index lock contention.
-   **TEST STRATEGY:** fixture repos including the hostile-config repo (assert no marker executes); worktree add/remove/lock/repair on
    Windows paths; slug truncation and case-collision tests; stash-sharing caveat test.

## C08 Merge Lane — `vela-core::merge_lane`

-   **INPUTS:** READY_TO_MERGE worker, integration worktree, recorded `integration_before`.
-   **OUTPUTS:** merged-and-validated integration, discarded merge, or conflict report; journal events and checkpoints.
-   **STATE OWNED:** the persisted lane lock and operation record; `integration_health` transitions via C01.
-   **DEPENDENCIES:** C07, validation command runner (C10), C11.
-   **FAILURE MODES:** textual conflict (to C09 resolution attempt or human); validation failure (discard unpublished merge, back to `FIXING`);
    crash mid-lane (health `UNKNOWN`, reconciliation decides); push failure (queued, retried idempotently).
-   **TEST STRATEGY:** clean merge, conflict, validation-failure discard, crash-at-each-step reconciliation, and no-force-push assertion (AT-018).

## C09 Checkpoint and Review Service — `vela-core::review`

-   **INPUTS:** worker worktree state, `fixed_point_sha`, review policy, reviewer results.
-   **OUTPUTS:** checkpoint commits (trailers), review cycles, findings with stable ids, blocking decisions (via policy), conflict-resolution attempts.
-   **STATE OWNED:** review cycles and findings; the advancing `fixed_point_sha`.
-   **DEPENDENCIES:** C07, C05 (fresh reviewer session), C03, C11.
-   **DESIGN:** verifies clean worktree and non-empty diff; creates the checkpoint only when the session is quiescent (ADR-015); fixed point
    advances to the merged integration tip (`IMPLEMENTATION_ARCHITECTURE.md` section 13); reviewer structure via Vela's prompt and, if
    spike S-SCHEMA-OUTPUT passes, `--json-schema` **[U]**; otherwise Vela classifies the two-section skill output itself.
-   **FAILURE MODES:** empty diff; hook-rejected commit (back to `FIXING`); unparseable review (one retry then `NEEDS_HUMAN`); oscillation or cap
    (`REVIEW_STALLED`).
-   **TEST STRATEGY:** seeded-defect loop (AT-004), oscillation, cap, hook rejection, index-lock wait, fixed-point advancement test.

## C10 ProcessRunner, power, session probe — `vela-process`

-   **INPUTS:** command, working directory, allowlisted environment, timeout, cancel token.
-   **OUTPUTS:** streamed redacted stdout/stderr, exit metadata, process-tree handle (Job Object).
-   **STATE OWNED:** live process trees; the keep-awake thread.
-   **DEPENDENCIES:** `tokio::process`, `windows` crate.
-   **FAILURE MODES:** timeout (tree kill); orphan after crash (found by reconciliation); redaction miss (central layer plus tests); power API failure (reported, run continues with a warning).
-   **TEST STRATEGY:** kill-tree and no-orphan tests (**[V]** via `taskkill /T`; Job Object implementation **[U]** S-PROC-JOB); redaction corpus; output-volume tests.

## C11 Persistence and Journal — `vela-persistence`

-   **INPUTS:** write commands from C01/C04/C09/C13; read queries.
-   **OUTPUTS:** committed state, journal entries with `seq`, post-commit notifications, backups.
-   **STATE OWNED:** the SQLite database, migration ladder, backups.
-   **DEPENDENCIES:** `rusqlite`.
-   **FAILURE MODES:** corruption/loss/failed migration (`RECOVERY.md`); disk full (run paused); state/journal mismatch (integrity failure, never silently repaired).
-   **TEST STRATEGY:** migration tests (forward, failure restore), atomicity tests (state and journal together), kill-during-write tests, integrity-check tests.

## C12 Preflight Service — `vela-core::preflight`

-   **INPUTS:** project path, settings, adapters' capability probes.
-   **OUTPUTS:** PASS/WARN/BLOCK report per `PREFLIGHT.md` (severity table, trust, Windows checks, Antigravity capability and posture, UIA consent state,
    unattended-readiness, submodule and Git-version checks).
-   **STATE OWNED:** report history; capability snapshots.
-   **DEPENDENCIES:** C07, C05 (capability and posture probes), C10, C16.
-   **FAILURE MODES:** probe timeout (`WARN`/`BLOCK` per table); partial setup (recorded, never half-applied).
-   **TEST STRATEGY:** fixture projects for every severity row; posture-probe tests against the fake `agy` and the gated real suite.

## C13 Recovery and Reconciliation — `vela-core::recovery`

-   **INPUTS:** persisted runs, operations, refs, worktrees, process/session inventory, remote state.
-   **OUTPUTS:** discrepancy classification, safe-resume actions, interventions.
-   **STATE OWNED:** reconciliation reports.
-   **DEPENDENCIES:** C07, C05, C10, C11.
-   **DESIGN:** per-operation recipes (merge: ancestry check; push: remote ref check; checkpoint: commit by trailer; approval: never replay a click);
    recovery-resume creates a new session at the last checkpoint; honors `recovery_continuation`.
-   **FAILURE MODES:** divergence (human); missing worktree; background `agy` update; state-store loss (inventory rebuild from Git, never auto-resume).
-   **TEST STRATEGY:** kill at every transition (Prompt 16), push-succeeded-not-recorded, merge-succeeded-not-recorded, approval-before-crash.

## C14 Tracker adapters — `vela-adapters::tracker_*`

-   **INPUTS/OUTPUTS:** list/read tickets and blockers, status/labels/comments, PR create/link; local Markdown parsing.
-   **STATE OWNED:** queued remote updates.
-   **DEPENDENCIES:** `gh` CLI (GitHub), filesystem (local).
-   **FAILURE MODES:** `gh` missing or unauthenticated (local-only; `AUTH_REQUIRED`); outage (queued, idempotent retry by lookup); malformed metadata (flagged).
-   **TEST STRATEGY:** fake `gh` executable, malformed-issue corpus, outage and duplicate-comment idempotency tests.

## C15 Context Index and Dependency Analyst — `vela-core::context`

-   **INPUTS:** repo data (read as data), manifests, tickets; analyst session output via C05 in read-only mode.
-   **OUTPUTS:** documentation manifest, project model, validated analyst result stored in the graph snapshot.
-   **STATE OWNED:** the manifest and snapshot inputs.
-   **DEPENDENCIES:** C05, C07, C02.
-   **FAILURE MODES:** analyst failure/invalid output (sequential labels); untrusted repo (data-only; hardened Git).
-   **TEST STRATEGY:** manifest discovery fixtures; analyst schema validation and fallback tests.

## C16 Settings, Trust, and Policy Store — `vela-core::settings` (storage in C11)

-   **INPUTS:** onboarding choices, settings edits, trust actions, scoped rule creation.
-   **OUTPUTS:** effective settings and run snapshots; journal events (`ProjectTrusted`, `ApprovalAutomationConsentChanged`, ...).
-   **STATE OWNED:** user-global and project settings, trust, consent, rules.
-   **DEPENDENCIES:** C11.
-   **FAILURE MODES:** invalid values (rejected); mid-run edits (applied to the next run unless explicitly live).
-   **TEST STRATEGY:** layering tests, snapshot immutability, trust revocation, rule-scope display tests.

## C17 Logging, Redaction, Diagnostics — `vela-process::redact`, `vela-core::diagnostics`

-   **INPUTS:** log events, process output, journal payloads.
-   **OUTPUTS:** rotated JSON logs, redacted payloads, diagnostic bundles (no repository source by default).
-   **STATE OWNED:** log files and retention.
-   **DEPENDENCIES:** `tracing`.
-   **FAILURE MODES:** a secret pattern missed (corpus tests; bundle export re-redacts); log disk growth (size-capped rotation).
-   **TEST STRATEGY:** redaction corpus, bundle-content tests, correlation-id propagation tests.

## C18 IPC and Event Gateway — `apps/desktop/src-tauri`

-   **INPUTS:** renderer commands; journal feed.
-   **OUTPUTS:** typed command results; coalesced `EventEnvelope` batches.
-   **STATE OWNED:** subscriptions only.
-   **DEPENDENCIES:** Tauri, C01, C11.
-   **FAILURE MODES:** renderer reload (resync by snapshot); malformed arguments (rejected in the core); unregistered command (blocked by capabilities).
-   **TEST STRATEGY:** contract test (registry versus TS wrappers), resync-after-gap test, argument-fuzz tests, event-burst test.

## C19 Lifecycle: tray, background, updater, notifications — `apps/desktop/src-tauri`

-   **INPUTS:** window events, onboarding settings, run state, update availability.
-   **OUTPUTS:** hide/exit decisions, tray state, notifications, deferred or applied updates (never during a run).
-   **STATE OWNED:** tray handle, window state.
-   **DEPENDENCIES:** Tauri plugins; C01.
-   **FAILURE MODES:** `prevent_exit` not honored (**[U]**: spike S-TRAY → amend ADR-017); notification denied (in-app indicator); WebView2 runtime update while running (prompt to restart UI).
-   **TEST STRATEGY:** lifecycle tests with a fake window host; manual and scripted checks on a clean machine (AT-019, AT-023, AT-024).

## C20 Frontend Store and Scenes — `apps/desktop/src`, `packages/ui/src/state`

-   **INPUTS:** snapshots and event batches; user input.
-   **OUTPUTS:** rendered DOM surfaces; canvas effect descriptors; commands.
-   **STATE OWNED:** UI-only state (scene, selection, graphics mode, reduced motion, layout offsets).
-   **DEPENDENCIES:** `packages/contracts`, C21.
-   **FAILURE MODES:** event gap (refetch); unknown event kind (ignored and logged); store divergence (snapshot overrides).
-   **TEST STRATEGY:** reducer property tests over recorded streams; keyboard and accessibility tests; intervention-state tests.

## C21 Canvas Layer — `packages/ui/src/canvas`

-   **INPUTS:** scene (nodes, edges, positions, states), effect descriptors, quality mode, visibility.
-   **OUTPUTS:** frames on demand; DOM-label projections; WebGL-lost notices.
-   **STATE OWNED:** GPU resources and per-frame animation state.
-   **DEPENDENCIES:** React Three Fiber/Three.js only here.
-   **FAILURE MODES:** context loss (fallback, ADR-019); software rendering; resource leak; low frame rate (suggest, never force, a lower mode).
-   **TEST STRATEGY:** mount/unmount leak test, frame-callback accounting while idle/minimized, 100/500-node performance runs, visual regression states, reduced-motion states.

## C22 Test Kit — `vela-testkit`, `tools/fake-approval-window`

-   **INPUTS/OUTPUTS:** fixture repo builder; scripted agent (replays transcripts, injects failures); fake `agy`; fault points; fake approval window.
-   **STATE OWNED:** temporary directories (outside synced folders).
-   **DEPENDENCIES:** ports only.
-   **FAILURE MODES:** fixture drift from real behavior (transcripts re-recorded from the gated real suite).
-   **TEST STRATEGY:** self-tests; transcripts versioned with the `agy` version that produced them.

---

# Requirements Coverage (FR-001..FR-049)

| FR | Implemented by | Verified by |
|---|---|---|
| 001 project management | C16, C18, C11 | e2e project import; unit |
| 002 preflight | C12 | fixtures per severity row |
| 003 documentation discovery | C15 | manifest fixtures |
| 004 skill bootstrap | C12, C05, C07 | missing-skills scenario; gated real suite |
| 005 issue ingestion | C14 | malformed-metadata corpus |
| 006 dependency graph | C02 | cycle tests |
| 007 semantic dependency analysis | C15, C02 | analyst validation/fallback tests |
| 008 safe parallelization | C02, C01 | AT-002, AT-003 |
| 009 fresh sessions | C05, C01 | AT-001; gated real suite |
| 010 worktree isolation | C07 | AT-002; worktree tests |
| 011 implementation invocation | C05 | AT-001 |
| 012 testing | C10, C01 | worker-fails-tests scenario |
| 013 review | C09, C05 | AT-004 |
| 014 review fix loop | C09, C01 | AT-004, oscillation/cap |
| 015 Git checkpoints | C09, C07 | AT-005 |
| 016 push | C07 (by Vela, ADR-015) | push scenarios; AT-021 |
| 017 merge | C08 | AT-002, AT-018 |
| 018 integration gate | C08, C01 | integration-regression scenario |
| 019 recovery | C13, C11 | AT-005, AT-022; Prompt 16 |
| 020 human intervention | C01, C20 | AT-010 |
| 021 kill switch | C01, C10, C19 | AT-006 |
| 022 resume | C13, C01 | AT-005, AT-006 |
| 023 execution history | C11, C20 | timeline tests |
| 024 capacity handling | C05, C01 | AT-007 (text detection **[V]**) |
| 025 spatial graph | C21, C20, C02 | AT-008; visual regression |
| 026 reactive environment | C21 | AT-008; performance tests |
| 027 efficiency | C21, C19 | AT-009 (budgets) |
| 028 notifications | C19 | AT-023 |
| 029 structured agent IO | C05, C09, C15 | schema-failure tests |
| 030 policy engine | C03 | AT-012, AT-020 |
| 031 Approval Broker | C04 | AT-011, AT-012 |
| 032 Approval Watchdog | C04 | watchdog tests (the 19-minute block observation) |
| 033 guarded UI approval fallback | C06 | AT-011, AT-014, AT-017 (one delivery **[V]**; reliability **[P/U]**) |
| 034 approval loop protection | C04 | AT-013 |
| 035 approval audit trail | C04, C11 | journal assertions |
| 036 compatibility health | C12, C05 | AT-015; spike S-NATIVE-POSTURE |
| 037 capability contract | C05, C12 | gated real suite |
| 038 policy sovereignty and posture | C03, C05, C12 | AT-015 |
| 039 evidence binding | C04, C06, C05 | EBR tests |
| 040 UI automation consent | C16, C06 | AT-016 |
| 041 unattended session management | C10, C04, C13 | AT-017 |
| 042 background and continuation | C19, C13, C16 | AT-019 |
| 043 repository trust | C16, C07, C03 | AT-020 |
| 044 worktree provisioning | C07, C16 | AT-001; provisioning tests |
| 045 completion and promotion | C01, C14, C07 | AT-021 |
| 046 state-store integrity | C11, C13 | AT-022 |
| 047 installation and updates | packaging, C19 | AT-024; clean-machine checklist |
| 048 renderer hardening | C18, `tauri.conf.json` | CSP, capabilities, injection tests |
| 049 authentication ownership | C05, C12 | AT-025 |

Non-functional coverage: reliability (C11, C13), safety (C03, C04, ADR-009/013), performance (C21, budgets), observability (C11, C17), portability
(Windows-gated crates, pure core), provider independence (ports, ADR-006/008), testability (ports, C22), accessibility (C20, ADR-019), security
(ADR-009/013, C18), maintainability (adapter isolation, contract test).
