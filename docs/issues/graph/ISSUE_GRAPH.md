# Issue Graph (Prompt 6 draft; GitHub publication deferred)

**Status:** the complete proposed implementation issue graph for Vela v1 is generated and approved as the Prompt 6 draft (2026-10-02). **No GitHub issues have been created.** Publication is intentionally deferred pending the independent **Prompt 7 issue/DAG audit** (`docs/project/CURRENT_STATE.md`).

This is a planning artifact. The repository specifications remain authoritative. The files here are the durable source from which Prompt 7 audits the graph and from which the later publication step creates issues.

## Contents

-   `docs/issues/graph/tickets/<KEY>.md`: 130 full ticket bodies (all 19 required items). Each begins with its planned labels, DAG depth, and published blockers.
-   `docs/issues/graph/ISSUE_GRAPH.json`: machine-readable graph (tickets, labels, requirements, reduced and unreduced blockers, depth, levels, critical path, frontier).
-   This file: counts, validation, DAG table, critical path, initial frontier, parallel waves, spike gates, traceability, and the publication plan.

Symbolic keys (for example `F02`, `S10`) are used instead of issue numbers because nothing is published. At publication each key is mapped to a real issue number and the blockers become native GitHub blocked-by links; that mapping will be recorded in this directory.

## Counts and validation

| Measure | Value |
|---|---|
| Tickets | **130** (127 in the first draft plus the three audit additions O01, TK1, R08) |
| Direct edges as authored | 283 |
| Published edges (transitively reduced) | **238** |
| Cycles | none |
| Unknown, self, or duplicate blockers | none |
| FR-001..FR-050 without a ticket | none |
| AT-001..AT-026 without a ticket | none |
| Critical path | **17 tickets** |
| Initial ready frontier | **14 tickets** |
| Widest parallel wave | 17 tickets |

## Critical path (17 tickets, counted by tickets, not effort)

F01 → F02 → F04 → F05 → F08 → S01 → S03 → S04 → S08 → S09 → S10 → S11 → S12 → S14 → Z03 → Z05 → Z06

## Initial ready frontier (14)

F01, SP01, SP02, SP03, SP04, SP06, SP07, SP08, SP09, SP10, SP11, H01, H02, H03

All but F01 are research spikes or human actions that need no prior ticket. Spikes SP04, SP08, SP09, and SP11 and human items H01-H03 need user assistance, so human attention is the real early bottleneck.

## Parallel waves (tickets that may run together once their blockers are done)

-   **Wave 0** (14): F01, SP01, SP02, SP03, SP04, SP06, SP07, SP08, SP09, SP10, SP11, H01, H02, H03
-   **Wave 1** (3): F02, K04, SP05
-   **Wave 2** (12): F03, F04, F06, F07, D01, D02, D05, D06, P03, T01, SP12, K05
-   **Wave 3** (10): A02, F05, L04, Z01, P01, D03, D04, D07, A04, SP13
-   **Wave 4** (9): TK1, F08, X02, U01, U02, P02, G01, T02, A01
-   **Wave 5** (10): A05, S01, S02, X03, O01, G02, G03, G05, T03, A03
-   **Wave 6** (17): S05, U04, S03, L01, L03, K06, U03, U15, S06, S15, S17, R01, G04, R05, A08, T04, A06
-   **Wave 7** (10): S04, PV1, K08, S07, U05, K01, U10, A09, A15, X04
-   **Wave 8** (12): S08, K10, U06, U07, U11, U12, A07, K02, K09, A10, A11, X01
-   **Wave 9** (6): S09, U14, U17, U08, U13, U16
-   **Wave 10** (3): S10, PF1, U18
-   **Wave 11** (6): S11, S16, R02, R04, A12, U19
-   **Wave 12** (9): S12, A13, R06, R07, A14, K07, L02, U09, Z02
-   **Wave 13** (5): S13, S14, R03, A16, R08
-   **Wave 14** (2): Z04, Z03
-   **Wave 15** (1): Z05
-   **Wave 16** (1): Z06

## Added by the dependency audit

-   **O01** Orchestrator actor skeleton (message bus, transactional transitions, run creation, snapshot assembly): previously assumed by S04, S06, S08, R01, and S15 but unowned.
-   **TK1** Scripted fake `agy` executable replaying recorded Antigravity behavior: needed by the tickets accepted against a fake agy (A03, A05, A06, A07).
-   **R08** Stop All safe points for the merge lane and push: split from R04 so the basic Stop All, run bar, and tray do not wait for the merge lane.

The audit also removed 47 preference edges and added 47 real blockers (including S08 ← S03/S04, S12 ← G05, K10 ← P03, Z03 ← L02/R07/R08/S15, U04/U05 ← U01, X04 ← S03), then relaxed U16 to depend on U07 only. No existing ticket's earliest start became later.

## Spike gates (an unverified element is claimed ready only after its spike closes with evidence)

| Spike | Gates |
|---|---|
| SP01 S-TRAY | L02 |
| SP02 S-UIA-BINDINGS | K05, K06, K10, X04 |
| SP03 S-PROC-JOB | P02 and everything that cancels process trees |
| SP04 S-NATIVE-POSTURE | A06, A07, K10, A16, Z04 |
| SP05 S-ASK-RESUME | A07 |
| SP06 S-HOOK-GLOBAL | A06 |
| SP07 S-SCHEMA-OUTPUT | A10 |
| SP08 S-DESKTOP-VISIBILITY | A05, A06, A07, A12, A15, A16, K10, X04, Z04, Z05, Z06 |
| SP10 S-SKILLS-ANTIGRAVITY | A08, A09, A10, A16 |
| SP12 S-UIA-RELIABILITY | SP13, K08 hardening |
| SP13 S-VISUAL | K08 |

Tickets downstream of SP08 (the isolated-profile and policy-enforcement path): A05, A06, A07, A12, A15, A16, K10, X04, Z04, Z05, Z06. Backend orchestration, trust, recovery, broker core, UI, and packaging do not wait on it.

## Area counts

foundation: 3 · domain: 6 · ui: 19 · orchestration: 16 · security: 7 · spike: 13 · human: 3 · broker: 10 · process: 3 · git: 6 · tracker: 5 · lifecycle: 6 · recovery: 8 · antigravity: 17 · perf: 2 · release: 6

## The DAG (published edges, transitively reduced)

| Key | Title | Phase | Blocked by |
|---|---|---|---|
| F01 | Workspace bootstrap: Cargo and npm workspaces, toolchain pins, CI checks | 0 | none |
| SP01 | Spike S-TRAY: does prevent_exit with a tray keep the Tauri process alive after the last window closes? | 0 | none |
| SP02 | Spike S-UIA-BINDINGS: does the windows crate expose the UI Automation interfaces Vela needs? | 0 | none |
| SP03 | Spike S-PROC-JOB: does a Windows Job Object with kill-on-close terminate a whole agy process tree? | 0 | none |
| SP04 | Spike S-NATIVE-POSTURE: do generated allow rules plus a fail-closed hook enforce ALLOW, DENY, and unknown? | 0 | none |
| SP06 | Spike S-HOOK-GLOBAL: are global-path hooks in an isolated profile loaded, and are policy files protected from agent writes? | 0 | none |
| SP07 | Spike S-SCHEMA-OUTPUT: does --json-schema enforce the reviewer contract in print mode? | 0 | none |
| SP08 | Spike S-DESKTOP-VISIBILITY: are headless CLI conversations visible and openable in Antigravity Desktop? | 0 | none |
| SP09 | Spike S-INTERACTIVE-TRUST: how do the CLI and Desktop treat a new workspace interactively? | 0 | none |
| SP10 | Spike S-SKILLS-ANTIGRAVITY: do the Matt Pocock implement and code-review skills run correctly inside agy? | 0 | none |
| SP11 | Spike S-CAP-REMAINING: WAITING status, concurrency ceiling, background self-update control, and interactive TUI behavior | 0 | none |
| H01 | Confirm Google position on external orchestration of the headless agy with the user account | 7 | none |
| H02 | Add the Stitch reference image to docs/ui/reference and record it in the index | 5 | none |
| H03 | Record the performance reference hardware profile | 6 | none |
| F02 | Domain primitives, ports, and TypeScript contract generation | 0 | F01 |
| K04 | Fake approval window harness reproducing the verified card roles | 2 | F01 |
| SP05 | Spike S-ASK-RESUME: after a hook-blocked ASK and a human allow, does a resumed conversation run the command? | 0 | SP04 |
| F03 | Test kit foundation: fixture repositories, fakes, and fault points | 0 | F02 |
| F04 | Tauri shell walking skeleton with strict CSP and a typed command round trip | 0 | F02 |
| F06 | Persistence foundation: single-writer SQLite, migration ladder, journal, and integrity checks | 0 | F02 |
| F07 | Structured logging, correlation ids, and the central redaction layer | 0 | F02 |
| D01 | State machines: run, worker, and ticket projection with exhaustive transition tests | 1 | F02 |
| D02 | Dependency graph engine: DAG, cycle detection, frontier, topological layers | 1 | F02 |
| D05 | Policy engine core: operation model, PowerShell and cmd normalizer, ordered rules, ALLOW/ASK/DENY | 1 | F02 |
| D06 | Review domain: finding identity, exit policy, loop state, and oscillation detection | 1 | F02 |
| P03 | Keep-awake thread and interactive-session probe | 1 | F02 |
| T01 | Local Markdown tracker: ticket parsing, blockers, and malformed-metadata handling | 1 | F02 |
| SP12 | Spike S-UIA-RELIABILITY: repeat and vary UI Automation approval delivery | 2 | SP02, K04 |
| K05 | vela-uia discovery and read: windows, warm-up, correlation, and card observation | 2 | SP02, K04, F02 |
| A02 | stream-json parser and result interpreter against recorded transcripts | 1 | F03 |
| F05 | Renderer skeleton: React, store, IPC wrappers, and front-end test tooling | 0 | F04 |
| L04 | Window state persistence and single-instance behavior | 5 | F04 |
| Z01 | Installer: signed NSIS per-user package with WebView2 handling and clean install and uninstall | 7 | F04 |
| P01 | ProcessRunner: spawn, allowlisted environment, timeouts, streamed redacted output, cancel token | 1 | F07 |
| D03 | Parallel-safety engine: predicted write sets, hard blockers, evidence labels, resource keys | 1 | D02 |
| D04 | Deterministic layered graph layout stored with the snapshot | 1 | D02 |
| D07 | Approval domain: requests, decisions, fingerprints, progress definition, loop-guard counters | 2 | D05, D01 |
| A04 | vela-hook binary, rule-table format, and spool records (fail-closed) | 2 | D05 |
| SP13 | Spike S-VISUAL: is the guarded visual tier ever needed on the primary path? | 2 | SP12 |
| TK1 (added) | Scripted fake agy executable replaying recorded Antigravity behavior | 1 | A02 |
| F08 | IPC gateway: command registry, snapshot plus sequenced event stream, resync | 0 | F05, F06 |
| X02 | Renderer hardening verification: CSP, inert rendering, capabilities, injection fuzz | 6 | F05 |
| U01 | Design system: tokens, typography, restrained glass, icons, and the base shell | 5 | F05 |
| U02 | Canvas foundation: reactive dot lattice with demand rendering and suspension | 5 | F05 |
| P02 | Process tree kill and cancellation | 1 | P01, SP03 |
| G01 | GitAdapter: hardened invocation, read operations, and hostile-config tests | 1 | P01, F03 |
| T02 | GitHub tracker via gh: read issues, native blocking links, labels, and evidence comments | 4 | P01 |
| A01 | Antigravity capability discovery: installation, versions, flags, and authentication state | 1 | P01, F03 |
| A05 | Per-worker profile manager and generated settings and allow rules | 2 | D05, SP08, TK1 |
| S01 | Project import: choose a repository, persist it, and list recent projects | 1 | F08 |
| S02 | Settings layers, onboarding choices, and the immutable run snapshot | 1 | F08 |
| X03 | Diagnostics export with redaction verification and log retention | 6 | F07, F08 |
| O01 (added) | Orchestrator actor skeleton: message bus, transactional transitions, run creation, snapshot assembly | 1 | D01, F08 |
| G02 | Worktree manager: root policy, create, remove, lock, repair, slug and case safety, cleanup | 1 | G01 |
| G03 | Checkpoint commits, per-worktree operation lock, and hook-aware commit handling | 1 | G01 |
| G05 | Remote operations: fetch, non-force push, remote ref verification, idempotent push | 1 | G01 |
| T03 | Remote update queue with idempotent retries and outage behavior | 4 | T02, F06 |
| A03 | Per-turn session driver: spawn, conversation id, explicit timeout, cancel, resume | 1 | A01, P02, TK1 |
| S05 | Context discovery and documentation manifest | 1 | S01, G01 |
| U04 | Home scene: drop a project folder, recent project anchors, and the import flow | 5 | S01, U02, U01 |
| S03 | Repository trust: untrusted by default, trust sheet, data-only analysis, Build refusal | 1 | S01, G01, D05, S02 |
| L01 | Onboarding experience: consent choices, background, auto-start, recovery continuation | 5 | S02, U01 |
| L03 | Login auto-start integration | 5 | S02 |
| K06 | vela-uia delivery: single-use allow or refusal through patterns, never persistent options | 2 | K05, S02 |
| U03 | Ambient violet, blue, and cyan field with Full, Balanced, and Efficiency graphics modes | 5 | U02, S02 |
| U15 | Command palette and settings sheet | 5 | S02, U01 |
| S06 | Issue ingestion to an approved-ready graph snapshot with cycle reporting | 1 | O01, T01, D03, D04, S01, S02 |
| S15 | Human intervention model: records, kinds, resume state, decisions | 1 | O01 |
| S17 | Desktop notifications for human-required, failure, and completion events | 1 | O01 |
| R01 | Operation records and idempotency framework | 3 | O01 |
| G04 | Merge operations: integration merge into branch, no-ff merge, discard unpublished merge, conflict detection | 1 | G02, G03 |
| R05 | State-store failure: backup, failed migration, and Git-derived inventory rebuild | 3 | F06, G02, G03 |
| A08 | Skills detection and bootstrap for worker worktrees | 2 | A01, G03, SP10 |
| T04 | PR workflow and promotion modes: draft PR, ready, local-ready, no silent merge | 4 | T02, G05 |
| A06 | Native policy enforcement and the posture probe (MEETS, DOES_NOT_MEET, UNKNOWN) | 2 | A05, A04, SP04, SP06, A03 |
| S04 | Preflight engine end to end: repository, toolchain, trust, and Windows checks with severity table | 1 | S03, O01 |
| PV1 | Worktree provisioning contract: confirmed commands, file allowlist, caches, resource keys | 1 | G02, S03 |
| K08 | Guarded visual fallback module (only if the spike says it is needed) | 6 | K06, SP13 |
| S07 | GitHub issue ingestion into the same graph pipeline | 4 | S06, T02 |
| U05 | Project universe: dependency constellation rendering, camera, and fit view | 5 | U02, S06, U01 |
| K01 | Approval Broker core: classification pipeline, evidence tiers, decisions, rules, interventions | 2 | D07, S15 |
| U10 | Intervention sheet for every intervention kind | 5 | S15, U01 |
| A09 | Worker task envelope and prompts: implement invocation, recovery envelope, untrusted-text handling | 2 | A03, A08, S05 |
| A15 | Real-environment compatibility suite gated by VELA_REAL_ANTIGRAVITY | 2 | A06 |
| X04 | Security regression suite: normalizer corpus, hostile repositories, credential audit, force-push denial | 6 | A06, K06, S03 |
| S08 | Graph approval and Build start: integration branch, integration worktree, base SHA | 1 | S06, G02, S04 |
| K10 | Effective-approval preflight and unattended-readiness report | 3 | A06, K06, S04, P03 |
| U06 | Node states and event-driven effects: ripples, edge flow, settling | 5 | U05 |
| U07 | Issue focus transition and inspector with progressive disclosure | 5 | U05 |
| U11 | Build-ready view and preflight panel with parallelization reasoning | 5 | S04, U05 |
| U12 | Conflict forecasting visualization | 5 | U05 |
| A07 | Headless ASK path: spool, APPROVAL_ASK, one-time allow, and resume | 2 | A06, SP05, K01 |
| K02 | Approval Watchdog and loop guard: progress heartbeats, thresholds, APPROVAL_STALLED | 2 | K01 |
| K09 | Scoped rule creation from an intervention with the scope shown first | 3 | K01, U10 |
| A10 | Authoritative reviewer session: fresh conversation, code-review at the fixed point, structured output | 2 | A09, D06, SP07 |
| A11 | Dependency analyst session: read-only, validated, stored in the snapshot | 2 | A09, S06 |
| X01 | Prompt-injection and untrusted-input boundaries verified end to end | 6 | S03, A09 |
| S09 | Scheduler tick: frontier, safety, capacity, and worker records with a fake agent | 1 | S08 |
| U14 | Reduced motion everywhere and the WebGL failure fallback | 5 | U06 |
| U17 | Large graphs: 500-node performance and adaptive quality suggestions | 6 | U06, U03 |
| U08 | Timeline and evidence panels: commands, tests, review findings | 5 | U07 |
| U13 | Accessibility: graph tree and list, keyboard navigation, focus, and screen-reader labels | 5 | U07 |
| U16 | Completion state and execution summary | 5 | U07 |
| S10 | Worker lifecycle with a fake agent: provision, implement, focused validation, timeline | 1 | S09, PV1 |
| PF1 | Performance harness and budget measurement on the reference hardware | 6 | U17, H03 |
| U18 | Visual regression suite and fixtures for the key states | 5 | U08, U10, U12, U16 |
| S11 | Checkpoint and review loop with a fake reviewer: fixed point, findings, fix loop, cap | 1 | S10, G03, D06 |
| S16 | Execution profiles and capacity model with simulated capacity events | 2 | S10 |
| R02 | Startup reconciliation and recovery-resume | 3 | R01, S10 |
| R04 | Stop All, pause, and resume for running workers | 3 | S10, P02, R01 |
| A12 | Conversation ids and Desktop visibility: record, show, and open per the verified mechanism | 5 | SP08, A03, U07, S10 |
| U19 | UI fidelity review gate against the Stitch reference | 5 | H02, U18 |
| S12 | Merge lane and integration validation: two parallel tickets merge, gate, frontier advances | 2 | S11, G04, G05 |
| A13 | Capacity detection from Antigravity output and pause/cooldown mapping | 2 | A03, S16 |
| R06 | Sleep, wake, network loss, and GitHub outage handling | 3 | R02, T03, P03 |
| R07 | Reboot continuation and the recovery_continuation setting | 3 | R02 |
| A14 | Antigravity version drift and background self-update divergence | 3 | A01, R02 |
| K07 | Interactive-session degradation and reconciliation for UI-automation work | 3 | K06, P03, R02, K01 |
| L02 | Tray, background operation, hide-on-close, and safe pause on close | 5 | SP01, R04 |
| U09 | Floating run bar: worker count, status, mode, pause, and an always reachable Stop All | 5 | R04, U01 |
| Z02 | Updater: signed updates, deferral during runs, pre-update backup, rollback | 7 | Z01, R05, R04 |
| S13 | Conflict resolution attempt and MERGE_CONFLICT escalation | 2 | S12 |
| S14 | Finalization: final review, push by Vela, promotion, cleanup, and completion | 4 | S12, T04 |
| R03 | Crash-injection recovery matrix across every worker transition | 3 | R02, S12 |
| A16 | Antigravity-backed single-ticket run end to end (AT-001 with the real agent) | 2 | A10, S12, A06 |
| R08 (added) | Stop All safe points for the merge lane and push; integration health UNKNOWN handling | 3 | R04, S12 |
| Z04 | Antigravity release gate: real-environment acceptance for AT-001, 011, 013, 015, 017, 025, 026 | 7 | A16, K10, A12, A15, K07 |
| Z03 | End-to-end fixture suite with fakes: AT-001 to AT-010 and AT-018 to AT-022 | 7 | S14, S13, R03, R08, R05, S16, L02, R07, S15 |
| Z05 | Requirements traceability report: every FR and AT with implementation and evidence | 7 | Z03, Z04, PF1, X04, Z02, U19, X01, X02, X03 |
| Z06 | Release candidate validation against the release checklist | 7 | Z05, H01, L03, L04, K08 |

## Requirement traceability (functional requirements)

Z05 (the requirements traceability report) names the full ranges FR-001..FR-050 and AT-001..AT-026 as an audit umbrella and is not listed below.

| Requirement | Tickets |
|---|---|
| FR-001 | S01, U04, Z05 |
| FR-002 | S04, A01, U11 |
| FR-003 | S05 |
| FR-004 | SP10, A08 |
| FR-005 | T01, T02, S06, S07 |
| FR-006 | D02, S06 |
| FR-007 | D03, S06, A11 |
| FR-008 | D02, D03, S08, S09, U11, U12 |
| FR-009 | SP03, P02, S10, A03, A09, A16 |
| FR-010 | G02 |
| FR-011 | SP10, S10, A03, A09, A16 |
| FR-012 | P01, S10, U08 |
| FR-013 | SP07, SP10, D06, S11, A10, A16, U08 |
| FR-014 | D06, S11 |
| FR-015 | G03, S11 |
| FR-016 | G05, T03, S14 |
| FR-017 | G04, S12, S13 |
| FR-018 | S12 |
| FR-019 | F06, D01, R01, R02, R03, R06, A14, O01 |
| FR-020 | D01, S15, U10 |
| FR-021 | SP03, D01, P02, R04, L02, U09, R08 |
| FR-022 | D01, R01, R02, R04, R08 |
| FR-023 | F06, F08, T02, U07, U08, U16, O01 |
| FR-024 | SP11, S16, A13 |
| FR-025 | F05, D04, U01, U05, U06 |
| FR-026 | U01, U02, U03, U06 |
| FR-027 | H03, U02, U03, U14, U15, U17, PF1 |
| FR-028 | S17 |
| FR-029 | F02, SP07, A02, A09, A10 |
| FR-030 | D05, X04 |
| FR-031 | SP05, D05, A07, K01, K09 |
| FR-032 | K02 |
| FR-033 | SP02, SP12, SP13, K05, K06, K08 |
| FR-034 | D07, K02 |
| FR-035 | F07, D07, A04, K01, X03 |
| FR-036 | SP04, A06, K10 |
| FR-037 | SP11, A01, A02, A03, A14, A15, TK1 |
| FR-038 | SP04, SP06, D05, A04, A05, A06 |
| FR-039 | A04, K01, K05 |
| FR-040 | S02, L01, U15 |
| FR-041 | P03, K07 |
| FR-042 | SP01, S02, R07, L01, L02, L03 |
| FR-043 | G01, S03, X01 |
| FR-044 | SP09, G02, PV1 |
| FR-045 | T04, S08, S14, U16 |
| FR-046 | F06, R05 |
| FR-047 | F04, Z01, Z02 |
| FR-048 | F04, F08, X02 |
| FR-049 | A01 |
| FR-050 | SP08, A05, A12, Z05 |

## Acceptance-test traceability

| Acceptance test | Tickets |
|---|---|
| AT-001 | F03, S01, S04, PV1, S10, A16, Z03, Z04, Z05 |
| AT-002 | S09, S12, Z03 |
| AT-003 | D03, S09, U11, U12, Z03 |
| AT-004 | S11, A10, Z03 |
| AT-005 | F03, R02, R03, A03, Z03 |
| AT-006 | U09, Z03, R08 |
| AT-007 | S16, A13, Z03 |
| AT-008 | H02, U01, U02, U04, U05, U06, U07, U18, U19 |
| AT-009 | U02, U03, PF1 |
| AT-010 | S15, U10, Z03 |
| AT-011 | SP12, A07, K04, K06, Z04 |
| AT-012 | K01, X04 |
| AT-013 | K02, Z04 |
| AT-014 | SP12, K04 |
| AT-015 | A06, A15, K10, Z04 |
| AT-016 | L01 |
| AT-017 | P03, K04, K07, Z04 |
| AT-018 | S12, S13, Z03 |
| AT-019 | R03, R07, L02, Z03 |
| AT-020 | S03, Z03 |
| AT-021 | T04, S14, Z03 |
| AT-022 | R05, Z03 |
| AT-023 | S17 |
| AT-024 | Z02 |
| AT-025 | A01, A15, X04, Z04 |
| AT-026 | SP08, A12, Z04, Z05 |

## Non-functional requirement references

| NFR | Tickets |
|---|---|
| NFR Maintainability | F01 |
| NFR Testability | F01, F02, F03, F05, U18, TK1 |
| NFR Provider independence | F02 |
| NFR Security | F04, F07, P01, G01, X01, Z06 |
| NFR Reliability | F06, T03, S02, R06, Z06, O01 |
| NFR Observability | F07, X03 |
| NFR Performance | F08, H03, D04, U17, PF1 |
| ADR-012 | SP01 |
| ADR-007 | SP13 |
| ADR-016 | H01 |
| NFR Portability | L04 |
| NFR Accessibility | U09, U13, U14 |

## Product-area coverage (from the Prompt 6 brief)

foundation F01-F08; Tauri shell F04, L02-L04; React frontend F05, U01-U19; Rust core O01, S01-S17; typed IPC F08; SQLite F06; event journal F06, O01; project import S01; preflight S04, K10; context discovery S05; local issues T01; GitHub issues T02, S07; DAG D02, S06; cycle detection D02, S06; parallelization D03; predicted write sets D03, A11; scheduler S09; worktrees G02, PV1; process execution P01-P03; agent sessions A03, A09; Matt Pocock integration SP10, A08, A10; testing F03, TK1, Z03, A15; checkpointing G03, S11; review A10, D06, S11; review/fix loop S11; merge lane S12, G04, S13; integration validation S12; Git push G05, S14; GitHub PR T04; recovery R01-R08; reconciliation R02; Stop All R04, R08; pause/resume R04; human intervention S15, U10; execution profiles and capacity S16, A13; Approval Broker K01; Watchdog K02; native approval path A04-A07; Windows UI Automation K04-K07; guarded visual fallback SP13, K08; approval-loop detection D07, K02; security and policy D05, X01-X04; prompt injection X01; observability F07; diagnostics X03; home UI U04; reactive dots U02; ambient gradients U03; dependency constellation U05; node states U06; issue inspector U07; timeline U08; conflict visualization U12; run controls U09; accessibility U13; reduced motion U14; graphics quality U03; performance PF1, U17; large graphs U17; installer Z01; updates Z02; release validation Z05, Z06.

## Publication plan (deferred)

Publication is **not** performed in this phase. After the independent Prompt 7 audit corrects the graph, publish as follows (hard to undo in bulk, so confirm with the user first):

1.  Verify `gh auth status` and the target repository (`Jaweria-Abdul-Hameed/vela`); the repository has no issues yet.
2.  Create the labels used in `ISSUE_GRAPH.json` (type, area, risk, phase, `ready-for-agent`, `ready-for-human`, `needs-user-assist`).
3.  Create issues in topological order (blockers first) from `tickets/<KEY>.md`, replacing symbolic blocker keys with real issue numbers and throttling to respect GitHub rate limits.
4.  Create native blocked-by links for every published edge (verify the dependency API on the first pairs; fall back to text references if unavailable).
5.  Record the key-to-issue-number map here, rebuild the DAG from the real issue ids, and update `docs/project/CURRENT_STATE.md`.
6.  Do not close or modify any parent issue.

## Notes on the process

-   The `to-tickets` skill (installed under `~/.agents/skills`) is user-invoked only. Its documented process (vertical slices, blocking edges, user approval, publish in dependency order with native links and the `ready-for-agent` label) was followed manually and not invoked. The skill advises omitting file paths; the brief explicitly requires expected write surfaces, which are given as crate and package paths.
-   The `/setup-matt-pocock-skills` tracker configuration has not been run; it edits `CLAUDE.md` or `AGENTS.md` and writes `docs/agents/*.md`, and must be handled on a separate branch under the preflight bootstrap rule.
