# Implementation Plan

## Phase 0 --- prove risky assumptions

Before polishing UI, prototype: 1. Antigravity/agent adapter can
start/cancel/observe a worker reliably. 2. Git worktree lifecycle is
reliable on Windows paths. 3. review fixed-point flow produces a real
non-empty diff. 4. Tauri background orchestration survives renderer
reload. 5. Three.js/R3F dot field can suspend cleanly. 6. The installed
Antigravity satisfies (or fails, with a concrete reason) each capability in the
Antigravity Required Capability Contract (`ADAPTERS.md`). 7. Native Permission
Posture: which native modes meet NPP-1..3 (ADR-009). 8. Approval delivery
feasibility on the installed version: detection, correlation to a Vela-created
session, UIA control identification, and behavior under lock, display-off, and
elevation (ADR-010, ADR-012). 9. Keep-awake, tray, and background lifecycle in
Tauri (ADR-012).

Throw prototypes away or explicitly graduate them; do not let
exploratory code become accidental architecture.

Phase 0 also executes the architecture spikes listed in `IMPLEMENTATION_ARCHITECTURE.md` section 14 (S-NATIVE-POSTURE,
S-ASK-RESUME, S-HOOK-GLOBAL, S-SCHEMA-OUTPUT, S-INTERACTIVE-TRUST, S-TRAY, S-UIA-BINDINGS, S-UIA-RELIABILITY, S-VISUAL, S-PROC-JOB,
S-CAP-REMAINING). Results are recorded in `docs/research/`; an element tagged unverified stays gated until its spike closes with
evidence.

## Phase 1 --- walking skeleton

One local Markdown ticket, one fake/real worker, one worktree, one test
command, one commit, one review result, one merge, persisted timeline.
UI can be minimal but must be end-to-end.

## Phase 2 --- graph scheduler

DAG, blockers, safe frontier, two parallel fixture workers, serialized
merge.

## Phase 2b --- policy and approval broker

Policy engine core and rule model, repository trust, Approval Broker domain model, native delivery,
posture check, then the UIA tier and loop protection, all against the fake approval-window harness
first and the real environment as soon as Phase 0 item 8 permits. The visual tier follows and stays
isolated. This phase precedes any unattended-run claim and must not be deferred behind UI polish.

## Phase 3 --- recovery

Kill processes at every state transition and prove reconciliation.

## Phase 4 --- GitHub

Issues, status/comments, pushes, integration PR.

## Phase 5 --- premium UI

Reactive field, ambient gradients, spatial graph, focus transitions,
evidence panels.

## Phase 6 --- security/performance

Policy engine hardening and extended rule coverage (the policy engine core
arrives in Phase 2b), untrusted input boundaries, renderer hardening, adaptive
rendering, large graphs.

## Phase 7 --- packaging

Installer, signed update strategy (never applied during a run), web-view
runtime handling, diagnostics, release checklist.

The UI design system begins early, but heavy visual polish should not
mask an unreliable scheduler.

# V1 Runtime Priority

The implementation roadmap must optimize for an excellent Antigravity-on-Windows path before provider breadth.

Early risky-assumption work should prove the Antigravity lifecycle Vela actually needs. Later phases may preserve adapter seams, but no phase should add peer runtime providers merely for architectural symmetry.

V1 completion requires Antigravity end-to-end validation; future-provider adapter work belongs after v1 unless separately approved.
