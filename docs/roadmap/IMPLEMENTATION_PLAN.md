# Implementation Plan

## Phase 0 --- prove risky assumptions

Before polishing UI, prototype: 1. Antigravity/agent adapter can
start/cancel/observe a worker reliably. 2. Git worktree lifecycle is
reliable on Windows paths. 3. review fixed-point flow produces a real
non-empty diff. 4. Tauri background orchestration survives renderer
reload. 5. Three.js/R3F dot field can suspend cleanly.

Throw prototypes away or explicitly graduate them; do not let
exploratory code become accidental architecture.

## Phase 1 --- walking skeleton

One local Markdown ticket, one fake/real worker, one worktree, one test
command, one commit, one review result, one merge, persisted timeline.
UI can be minimal but must be end-to-end.

## Phase 2 --- graph scheduler

DAG, blockers, safe frontier, two parallel fixture workers, serialized
merge.

## Phase 3 --- recovery

Kill processes at every state transition and prove reconciliation.

## Phase 4 --- GitHub

Issues, status/comments, pushes, integration PR.

## Phase 5 --- premium UI

Reactive field, ambient gradients, spatial graph, focus transitions,
evidence panels.

## Phase 6 --- security/performance

Policy engine, untrusted input boundaries, adaptive rendering, large
graphs.

## Phase 7 --- packaging

Installer, update strategy, diagnostics, release checklist.

The UI design system begins early, but heavy visual polish should not
mask an unreliable scheduler.

# V1 Runtime Priority

The implementation roadmap must optimize for an excellent Antigravity-on-Windows path before provider breadth.

Early risky-assumption work should prove the Antigravity lifecycle Vela actually needs. Later phases may preserve adapter seams, but no phase should add peer runtime providers merely for architectural symmetry.

V1 completion requires Antigravity end-to-end validation; future-provider adapter work belongs after v1 unless separately approved.
