# Issue Graph (Prompt 6 draft, audited by Prompt 7; GitHub publication deferred)

**Status:** the complete proposed implementation issue graph for Vela v1 was generated and approved as the Prompt 6 draft (2026-10-02) and then **independently audited and corrected by Prompt 7** (2026-10-02). **No GitHub issues have been created.** Publication is intentionally deferred until the user confirms it (`docs/project/CURRENT_STATE.md`).

This is a planning artifact. The repository specifications remain authoritative. The files here are the durable source from which the later publication step creates issues.

## Contents

-   `docs/issues/graph/tickets/<KEY>.md`: 135 full ticket bodies (all 19 required items). Each begins with its planned labels, DAG depth, and published blockers.
-   `docs/issues/graph/ISSUE_GRAPH.json`: machine-readable graph (tickets, labels, requirements, reduced and unreduced blockers, depth, levels, critical path, frontier).
-   `docs/issues/graph/SHARED_SURFACE_PROTOCOL.md`: **normative** rules for the shared files, migrations, command registry, composition root, resource keys, and shared documents that parallel tickets rely on (Prompt 7).
-   `docs/issues/graph/AUDIT_PROMPT7.md`: the audit report: findings, corrections, dependency changes, parallel and serialized groups, coverage matrix, and remaining human decisions.
-   This file: counts, validation, DAG table, critical path, initial frontier, waves, spike gates, traceability, and the publication plan.

Symbolic keys (for example `F02`, `S10`) are used instead of issue numbers because nothing is published. At publication each key is mapped to a real issue number and the blockers become native GitHub blocked-by links; that mapping will be recorded in this directory.

## Counts and validation

| Measure | Value |
|---|---|
| Tickets | **135** (130 in the Prompt 6 draft plus 6 added by the Prompt 7 audit: F09, W01, X05, H04, H05, H06, minus H02 and H03, which were resolved before publication, plus Z07 added by the H05 follow-up) |
| Direct edges as authored | 347 |
| Published edges (transitively reduced) | **274** |
| Cycles | none (independently recomputed from the ticket files) |
| Unknown, self, or duplicate blockers | none |
| Ticket header, body section 7, and JSON blockers agree | yes (all 135) |
| FR-001..FR-050 without a ticket | none |
| AT-001..AT-026 without a ticket | none |
| Tickets with no dependent (release gate leaves) | none other than Z06 (18 tickets had none in the Prompt 6 draft; 10 gained real dependents and 8 now block Z05: A13, A14, K09, L01, R06, S07, U09, U13) |
| Critical path | **17 tickets** (14 distinct longest chains tie at this length) |
| Initial ready frontier | **14 tickets** |
| Widest parallel wave | 16 tickets (a topological layer, not a safe-to-run-together set; see AUDIT_PROMPT7.md) |

## Critical path (17 tickets, counted by tickets, not effort)

F01 → F02 → F04 → F05 → F08 → S01 → S03 → S04 → S08 → S09 → S10 → S11 → S12 → S14 → Z03 → Z05 → Z06

The Prompt 7 corrections add edges but leave this chain and its length unchanged. Several other chains tie at the same length because Z03 waits on nine tickets at depth 13 (S14, S13, R03, R08, L02, and others), so schedule slack is thin everywhere on the S, R, and L chains.

## Initial ready frontier (14)

F01, H01, SP01, SP02, SP03, SP04, SP06, SP07, SP08, SP09, SP10, SP11, H04, H05

F01 is the only code ticket. The rest are research spikes and human actions. H01, SP04, SP06, SP08, SP09, SP11, H04, H05 need the user's help, and spikes that run the real `agy` (SP04, SP06, SP07, SP08, SP10, SP11) must run one at a time (`RK-AGY-REAL-ACCOUNT`, SHARED_SURFACE_PROTOCOL.md section 8). Human attention is the real early bottleneck, mainly through the spikes and H01. H04 and H05 are in the frontier only because they have no blockers: H04 (production release signing) gates Z06 and H05 (the clean-machine VM, provisioned when Z07 is about to start) gates Z07. H06 is not in the frontier (it depends on SP09).

## Waves (topological layers: tickets whose blockers are all in earlier layers)

A layer is an upper bound on what could run together. Which members are actually safe together is decided in AUDIT_PROMPT7.md using write surfaces, contracts, migrations, lockfiles, generated artifacts, fixtures, resource keys, and semantic coupling.

-   **Wave 0** (14): F01, SP01, SP02, SP03, SP04, SP06, SP07, SP08, SP09, SP10, SP11, H01, H04, H05
-   **Wave 1** (5): F02, K04, SP05, X05, H06
-   **Wave 2** (12): F03, F04, F06, F07, D01, D02, D05, D06, P03, T01, SP12, K05
-   **Wave 3** (10): A02, F05, L04, P01, D03, D04, D07, A04, SP13, F09
-   **Wave 4** (9): Z01, TK1, F08, X02, U01, P02, G01, T02, A01
-   **Wave 5** (11): U02, A05, S01, S02, X03, O01, G02, G03, G05, T03, A03
-   **Wave 6** (16): S05, U04, S03, L03, K06, U03, U15, S06, S15, S17, R01, G04, R05, A08, T04, A06
-   **Wave 7** (10): L01, S04, PV1, K08, S07, U05, K01, U10, A09, A15
-   **Wave 8** (11): X04, S08, U06, U07, U12, A07, K02, K09, A10, A11, X01
-   **Wave 9** (6): U11, S09, U14, U08, U13, U16
-   **Wave 10** (3): K10, U17, S10
-   **Wave 11** (5): PF1, U18, S11, R02, W01
-   **Wave 12** (9): S16, R04, A12, U19, S12, R06, R07, A14, K07
-   **Wave 13** (9): A13, L02, U09, Z02, S13, S14, R03, A16, R08
-   **Wave 14** (3): Z04, Z03, Z07
-   **Wave 15** (1): Z05
-   **Wave 16** (1): Z06

## Added or changed by earlier audits

-   **Prompt 6 dependency audit:** O01 Orchestrator skeleton, TK1 fake `agy`, R08 Stop All safe points; 47 preference edges swapped for 47 real blockers.
-   **Prompt 7 independent audit:** added F09, W01, X05, H04, H05, H06; 55 edges added, 21 edges removed or made implied by transitive reduction, 17 depth changes, and corrections to roughly 90 ticket bodies (write surfaces, acceptance criteria, unit and integration test requirements, parallelization notes, ADR references). Details in AUDIT_PROMPT7.md.

-   **H01 follow-up (2026-10-03):** provider-policy-block handling was added by extending FR-024 and AT-007 (no new requirement or acceptance test, no new ticket). One edge was added, S15 to S16, because S16 now creates an intervention through the S15 model; depth, critical path, frontier, and write surfaces are unchanged. H01 stays a release gate only (Z06). Details in AUDIT_PROMPT7.md and docs/research/H01_google_position.md.

-   **H04 follow-up (2026-10-03):** production signing is a release prerequisite, not a development prerequisite. The edge H04 to Z02 was removed (Z02 verifies the updater with a disposable test keypair) and the edge H04 to Z06 was added (Z06 owns the production release-signing gate, so a release cannot complete while H04 is open): one edge removed, one added, no depth, critical-path, or frontier change. Tauri updater signing and Windows Authenticode are separate mechanisms in every ticket. Details in AUDIT_PROMPT7.md and docs/research/H04_signing_research_2026-10-03.md.

-   **H05 follow-up (2026-10-03):** the clean-machine route is decided (a local Windows 11 virtual machine under VMware Workstation Pro, Windows 11 Enterprise 90-day Evaluation guest) but not provisioned. H05 no longer blocks Z01: Z01 builds and development-signs the installer without a VM. The environment-dependent evidence moved to the new ticket Z07 (blocked by H05, Z02, and the lifecycle features it exercises), which feeds Z05 and therefore Z06. Edges and counts in AUDIT_PROMPT7.md.

## Resolved before publication

-   **H02** (add the Stitch reference image): the user supplied the asset, verified at `docs/ui/reference/vela-stitch-reference.png`, and it is recorded in `DOCUMENTATION_INDEX.md`, `docs/ui/REFERENCE_BRIEF.md`, and `CURRENT_STATE.md`. A resolved human action is not published as an issue, so H02 is removed from the graph; U19 (fidelity gate) and the U01-U04 tickets now cite the asset path directly.
-   **H03** (record the performance reference hardware): the user's development laptop is the canonical reference machine. Its identity was verified with read-only queries and is recorded as profile `REF-HW-1` in the "Reference Hardware Profile" section of `docs/ui/PERFORMANCE_BUDGET.md` (budgets unchanged), and in `CURRENT_STATE.md`. A resolved human action is not published as an issue, so H03 is removed from the graph and its only edge, H03 to PF1, is dropped. PF1 now requires the profile ID and a per-run environment capture with every result.

## Spike gates (an unverified element is claimed ready only after its spike closes with evidence)

| Spike | Direct dependents | Tickets gated transitively (excluding Z03-Z06 and PF1) |
|---|---|---|
| SP01 S-TRAY | L02 | 1 |
| SP02 S-UIA-BINDINGS | K05, SP12 | 8 |
| SP03 S-PROC-JOB | P02 | 20 |
| SP04 S-NATIVE-POSTURE | A06, SP05 | 7 |
| SP05 S-ASK-RESUME | A07 | 2 |
| SP06 S-HOOK-GLOBAL | A06 | 6 |
| SP07 S-SCHEMA-OUTPUT | A10 | 3 |
| SP08 S-DESKTOP-VISIBILITY | A05, A12 | 9 |
| SP09 S-INTERACTIVE-TRUST | H06 | 2 |
| SP10 S-SKILLS-ANTIGRAVITY | A08 | 8 |
| SP11 S-CAP-REMAINING | A14, S16 | 3 |
| SP12 S-UIA-RELIABILITY | SP13 | 2 |
| SP13 S-VISUAL | K08 | 1 |

Tickets downstream of SP08 (the isolated-profile and policy-enforcement path, ADR-020): A05, A06, A15, A07, X04, K10, A12, A14, A16, Z04, Z05, Z06.

## Area counts

foundation: 5 · domain: 6 · ui: 19 · orchestration: 16 · security: 8 · spike: 13 · human: 5 · broker: 10 · process: 3 · git: 6 · tracker: 5 · lifecycle: 6 · recovery: 8 · antigravity: 17 · perf: 2 · release: 6

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
| H04 (Prompt 7) | Decide and provision production release signing: Windows Authenticode route and production updater keypair | 7 | none |
| H05 (Prompt 7) | Provide a clean Windows test environment for install, upgrade, and uninstall evidence | 7 | none |
| F02 | Domain primitives, ports, and TypeScript contract generation | 0 | F01 |
| K04 | Fake approval window harness reproducing the verified card roles | 2 | F01 |
| SP05 | Spike S-ASK-RESUME: after a hook-blocked ASK and a human allow, does a resumed conversation run the command? | 0 | SP04 |
| X05 (Prompt 7) | CI supply-chain gates: secret scanning, dependency audit, license check, and locked builds | 0 | F01 |
| H06 (Prompt 7) | Decide whether v1 creates Desktop-hosted Antigravity sessions for approval delivery (UI Automation path scope) | 2 | SP09 |
| F03 | Test kit foundation: fixture repositories, fault-injection harness, and the synced-folder guard | 0 | F02 |
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
| P01 | ProcessRunner: spawn, allowlisted environment, timeouts, streamed redacted output, cancel token | 1 | F07 |
| D03 | Parallel-safety engine: predicted write sets, hard blockers, evidence labels, resource keys | 1 | D02 |
| D04 | Deterministic layered graph layout stored with the snapshot | 1 | D02 |
| D07 | Approval domain: requests, decisions, fingerprints, progress definition, loop-guard counters | 2 | D05, D01 |
| A04 | vela-hook binary, rule-table format, and spool records (fail-closed) | 2 | D05 |
| SP13 | Spike S-VISUAL: is the guarded visual tier ever needed on the primary path? | 2 | SP12 |
| F09 (Prompt 7) | Port fakes and the shared conformance suite | 0 | F03 |
| Z01 | Installer: NSIS per-user package with development signing, WebView2 handling, and uninstall behavior | 7 | F04, A04 |
| TK1 (added) | Scripted fake agy executable replaying recorded Antigravity behavior | 1 | A02 |
| F08 | IPC gateway: command registry, snapshot plus sequenced event stream, resync | 0 | F05, F06 |
| X02 | Renderer hardening verification: CSP, inert rendering, capabilities, injection fuzz | 6 | F05 |
| U01 | Design system: tokens, typography, restrained glass, icons, and the base shell | 5 | F05 |
| P02 | Process tree kill and cancellation | 1 | P01, SP03 |
| G01 | GitAdapter: hardened invocation, read operations, and hostile-config tests | 1 | P01, F03 |
| T02 | GitHub tracker via gh: read issues, native blocking links, labels, and evidence comments | 4 | P01, F09 |
| A01 | Antigravity capability discovery: installation, versions, flags, and authentication state | 1 | P01, F03 |
| U02 | Canvas foundation: reactive dot lattice with demand rendering and suspension | 5 | U01 |
| A05 | Per-worker profile manager and generated settings and allow rules | 2 | D05, SP08, TK1 |
| S01 | Project import: choose a repository, persist it, and list recent projects | 1 | F08 |
| S02 | Settings layers, onboarding choices, and the immutable run snapshot | 1 | F08 |
| X03 | Diagnostics export with redaction verification and log retention | 6 | F07, F08 |
| O01 (added) | Orchestrator actor skeleton: message bus, transactional transitions, run creation, snapshot assembly | 1 | D01, F08, F09, F07 |
| G02 | Worktree manager: root policy, create, remove, lock, repair, slug and case safety, cleanup | 1 | G01 |
| G03 | Checkpoint commits, per-worktree operation lock, and hook-aware commit handling | 1 | G01 |
| G05 | Remote operations: fetch, non-force push, remote ref verification, idempotent push | 1 | G01 |
| T03 | Remote update queue with idempotent retries and outage behavior | 4 | T02, F06 |
| A03 | Per-turn session driver: spawn, conversation id, explicit timeout, cancel, resume | 1 | A01, P02, TK1 |
| S05 | Context discovery and documentation manifest | 1 | S01, G01 |
| U04 | Home scene: drop a project folder, recent project anchors, and the import flow | 5 | S01, U02 |
| S03 | Repository trust: untrusted by default, trust sheet, data-only analysis, Build refusal | 1 | S01, G01, D05, S02, U01 |
| L03 | Login auto-start integration | 5 | S02 |
| K06 | vela-uia delivery: single-use allow or refusal through patterns, never persistent options | 2 | K05, S02 |
| U03 | Ambient violet, blue, and cyan field with Full, Balanced, and Efficiency graphics modes | 5 | U02, S02 |
| U15 | Command palette and settings sheet | 5 | S02, U01 |
| S06 | Issue ingestion to an approved-ready graph snapshot with cycle reporting | 1 | O01, T01, D03, D04, S01, S02, U01 |
| S15 | Human intervention model: records, kinds, resume state, decisions | 1 | O01 |
| S17 | Desktop notifications for human-required, failure, and completion events | 1 | O01 |
| R01 | Operation records and idempotency framework | 3 | O01 |
| G04 | Merge operations: integration merge into branch, no-ff merge, discard unpublished merge, conflict detection | 1 | G02, G03 |
| R05 | State-store failure: backup, failed migration, and Git-derived inventory rebuild | 3 | F06, G02, G03 |
| A08 | Skills detection and bootstrap for worker worktrees | 2 | A01, G03, SP10, G02 |
| T04 | PR workflow and promotion modes: draft PR, ready, local-ready, no silent merge | 4 | T02, G05 |
| A06 | Native policy enforcement and the posture probe (MEETS, DOES_NOT_MEET, UNKNOWN) | 2 | A05, A04, SP04, SP06, A03 |
| L01 | Onboarding experience: consent choices, background, auto-start, recovery continuation | 5 | U15 |
| S04 | Preflight engine end to end: repository, toolchain, trust, and Windows checks with severity table | 1 | S03, O01, G05, T02 |
| PV1 | Worktree provisioning contract: confirmed commands, file allowlist, caches, resource keys | 1 | G02, S03 |
| K08 | Guarded visual fallback module (only if the spike says it is needed) | 6 | K06, SP13 |
| S07 | GitHub issue ingestion into the same graph pipeline | 4 | S06, T02 |
| U05 | Project universe: dependency constellation rendering, camera, and fit view | 5 | U02, S06 |
| K01 | Approval Broker core: classification pipeline, evidence tiers, decisions, rules, interventions | 2 | D07, S15 |
| U10 | Intervention sheet for every intervention kind | 5 | S15, U01, X02 |
| A09 | Worker task envelope and prompts: implement invocation, recovery envelope, untrusted-text handling | 2 | A03, A08, S05 |
| A15 | Real-environment compatibility suite gated by VELA_REAL_ANTIGRAVITY | 2 | A06 |
| X04 | Security regression suite: normalizer corpus, hostile repositories, credential audit, force-push denial | 6 | A06, K06, S03, K01 |
| S08 | Graph approval and Build start: integration branch, integration worktree, base SHA | 1 | S06, G02, S04 |
| U06 | Node states and event-driven effects: ripples, edge flow, settling | 5 | U05 |
| U07 | Issue focus transition and inspector with progressive disclosure | 5 | U05, X02 |
| U12 | Conflict forecasting visualization | 5 | U05 |
| A07 | Headless ASK path: spool, APPROVAL_ASK, one-time allow, and resume | 2 | A06, SP05, K01 |
| K02 | Approval Watchdog and loop guard: progress heartbeats, thresholds, APPROVAL_STALLED | 2 | K01 |
| K09 | Scoped rule creation from an intervention with the scope shown first | 3 | K01, U10 |
| A10 | Authoritative reviewer session: fresh conversation, code-review at the fixed point, structured output | 2 | A09, D06, SP07 |
| A11 | Dependency analyst session: read-only, validated, stored in the snapshot | 2 | A09, S06 |
| X01 | Prompt-injection and untrusted-input boundaries verified end to end | 6 | S03, A09 |
| U11 | Build-ready view and preflight panel with parallelization reasoning | 5 | U05, S08 |
| S09 | Scheduler tick: frontier, safety, capacity, and worker records with a fake agent | 1 | S08 |
| U14 | Reduced motion everywhere and the WebGL failure fallback | 5 | U06 |
| U08 | Timeline and evidence panels: commands, tests, review findings | 5 | U07 |
| U13 | Accessibility: graph tree and list, keyboard navigation, focus, and screen-reader labels | 5 | U07 |
| U16 | Completion state and execution summary | 5 | U07 |
| K10 | Antigravity preflight providers and unattended readiness: capabilities, authentication, skills, posture, and UI-automation consent | 3 | A06, K06, P03, U11, A08 |
| U17 | Large graphs: 500-node performance and adaptive quality suggestions | 6 | U03, U07, U14 |
| S10 | Worker lifecycle with a fake agent: provision, implement, focused validation, timeline | 1 | S09, PV1, R01 |
| PF1 | Performance harness and budget measurement on the reference hardware | 6 | U17 |
| U18 | Visual regression suite and fixtures for the key states | 5 | U08, U10, U12, U16, U04, U17 |
| S11 | Checkpoint and review loop with a fake reviewer: fixed point, findings, fix loop, cap | 1 | S10, G03, D06 |
| R02 | Startup reconciliation and recovery-resume | 3 | S10 |
| W01 (Prompt 7) | Production composition root: real adapter wiring and a fixture run in the real window | 2 | S10, S17, P03, T04 |
| S16 | Execution profiles and capacity model with simulated capacity events | 2 | R02, SP11, S15 |
| R04 | Stop All, pause, and resume for running workers | 3 | P02, S11 |
| A12 | Conversation visibility in Antigravity Desktop: read model, Open in Antigravity action, and the AT-026 harness | 5 | SP08, U07, S11, A10, A11 |
| U19 | UI fidelity review gate against the Stitch reference | 5 | U18 |
| S12 | Merge lane and integration validation: two parallel tickets merge, gate, frontier advances | 2 | S11, G04 |
| R06 | Sleep, wake, network loss, and GitHub outage handling | 3 | R02, T03, P03 |
| R07 | Reboot continuation and the recovery_continuation setting | 3 | R02 |
| A14 | Antigravity recovery and version drift: orphan sessions, stale profile and spool state, and background self-update | 3 | R02, A03, A05, SP11 |
| K07 | Interactive-session degradation and reconciliation for UI-automation work | 3 | K06, P03, R02, H06, K02 |
| A13 | Capacity detection from Antigravity output and pause/cooldown mapping | 2 | A03, S16 |
| L02 | Tray, background operation, hide-on-close, and safe pause on close | 5 | SP01, R04 |
| U09 | Floating run bar: worker count, status, mode, pause, and an always reachable Stop All | 5 | R04 |
| Z02 | Updater: signed updates, deferral during runs, pre-update backup, rollback | 7 | Z01, R05, R04 |
| S13 | Conflict resolution attempt and MERGE_CONFLICT escalation | 2 | S12 |
| S14 | Finalization: final review, push by Vela, promotion, cleanup, and completion | 4 | S12, T04 |
| R03 | Crash-injection recovery matrix across every worker transition | 3 | R02, S12, K01 |
| A16 | Antigravity-backed single-ticket run end to end (AT-001 with the real agent) | 2 | A10, S12, W01, A07 |
| R08 (added) | Stop All safe points for the merge lane and push; integration health UNKNOWN handling | 3 | R04, S12 |
| Z04 | Antigravity release gate: real-environment acceptance for AT-001, 011, 013, 015, 017, 025, 026 | 7 | A16, K10, A12, A15, K07 |
| Z03 | End-to-end fixture suite with fakes: AT-001 to AT-007, AT-010, and AT-018 to AT-022 | 7 | S14, S13, R03, R08, R05, S16, L02, R07, W01 |
| Z05 | Requirements traceability report: every FR and AT with implementation and evidence | 7 | Z03, Z04, PF1, X04, U19, X01, X03, A13, A14, K09, R06, S07, U09, U13, Z07 |
| Z06 | Release candidate validation against the release checklist | 7 | Z05, H01, L04, K08, H04, X05 |
| Z07 | Clean-machine installer and lifecycle validation on the H05 environment | 7 | Z02, H05, W01, K10, L03, L01 |

## Requirement traceability (functional requirements)

Z05 (the requirements traceability report) names the full ranges FR-001..FR-050 and AT-001..AT-026 as an audit umbrella and is not listed below.

| Requirement | Tickets |
|---|---|
| FR-001 | S01, U04 |
| FR-002 | A01, S04, U11, K10 |
| FR-003 | S05 |
| FR-004 | SP10, A08, K10 |
| FR-005 | T01, T02, S06, S07 |
| FR-006 | D02, S06 |
| FR-007 | D03, S06, A11 |
| FR-008 | D02, D03, S08, U12, S09, U11 |
| FR-009 | SP03, P02, A03, A09, S10, A16 |
| FR-010 | G02 |
| FR-011 | SP10, A03, A09, S10, A16 |
| FR-012 | P01, U08, S10, W01 |
| FR-013 | SP07, SP10, D06, A10, U08, S11, A16 |
| FR-014 | D06, S11 |
| FR-015 | G03, S11 |
| FR-016 | G05, T03, S14 |
| FR-017 | G04, S12, S13 |
| FR-018 | S12 |
| FR-019 | D01, F06, O01, R01, R02, A14, R06, R03 |
| FR-020 | D01, S15, U10 |
| FR-021 | SP03, D01, P02, R04, L02, R08, U09 |
| FR-022 | D01, R01, R02, A14, R04, R08 |
| FR-023 | F06, F08, T02, O01, U07, U08, U16 |
| FR-024 | SP11, S16, A13 |
| FR-025 | D04, F05, U01, U05, U06 |
| FR-026 | U01, U02, U03, U06 |
| FR-027 | U02, U03, U15, U14, U17, PF1 |
| FR-028 | S17 |
| FR-029 | SP07, F02, A02, A09, A10 |
| FR-030 | D05, X04 |
| FR-031 | SP05, D05, K01, A07, K09 |
| FR-032 | K02 |
| FR-033 | SP02, H06, K05, SP12, SP13, K06, K08 |
| FR-034 | D07, K02 |
| FR-035 | X05, F07, A04, D07, X03, K01 |
| FR-036 | SP04, A06, K10 |
| FR-037 | SP11, A02, A01, TK1, A03, A15, K10, A14 |
| FR-038 | SP04, SP06, D05, A04, A05, A06 |
| FR-039 | K05, A04, K01 |
| FR-040 | H06, S02, U15, L01 |
| FR-041 | H06, P03, K07 |
| FR-042 | SP01, S02, L03, L01, R07, L02 |
| FR-043 | G01, S03, X01 |
| FR-044 | SP09, G02, PV1 |
| FR-045 | T04, S08, U16, S14 |
| FR-046 | F06, R05 |
| FR-047 | H04, H05, F04, Z01, Z02, Z07 |
| FR-048 | F04, F08, X02 |
| FR-049 | A01, K10 |
| FR-050 | SP08, A05, A12 |

## Acceptance-test traceability

| Acceptance test | Tickets |
|---|---|
| AT-001 | S01, PV1, S04, S10, A16, Z03, Z04 |
| AT-002 | S09, S12, Z03 |
| AT-003 | D03, U12, S09, U11, Z03 |
| AT-004 | A10, S11, Z03 |
| AT-005 | F03, A03, R02, R03, Z03 |
| AT-006 | R08, U09, Z03 |
| AT-007 | S16, A13, Z03 |
| AT-008 | U01, U02, U04, U05, U06, U07, U18, U19 |
| AT-009 | U02, U03, PF1 |
| AT-010 | S15, U10, Z03 |
| AT-011 | K04, SP12, K06, A07, Z04 |
| AT-012 | K01, X04 |
| AT-013 | K02, Z04 |
| AT-014 | K04, SP12, K05 |
| AT-015 | A06, A15, K10, Z04 |
| AT-016 | K06, L01 |
| AT-017 | K04, P03, K07, Z04 |
| AT-018 | S12, S13, Z03 |
| AT-019 | R07, L02, R03, Z03 |
| AT-020 | S03, Z03 |
| AT-021 | T04, S14, Z03 |
| AT-022 | R05, Z03 |
| AT-023 | S17 |
| AT-024 | Z02 |
| AT-025 | A01, A15, X04, Z04 |
| AT-026 | SP08, A12, Z04 |

## Non-functional requirement and ADR references

| Reference | Tickets |
|---|---|
| ADR-007 | SP13 |
| ADR-012 | SP01 |
| ADR-016 | H01 |
| NFR Accessibility | U13, U14, U09 |
| NFR Maintainability | F01 |
| NFR Observability | F07, X03 |
| NFR Performance | D04, F08, U17, PF1 |
| NFR Portability | L04 |
| NFR Provider independence | F02, F09 |
| NFR Reliability | F06, O01, S02, T03, W01, R06, Z06 |
| NFR Security | X05, F04, F07, P01, G01, X01, W01, Z06 |
| NFR Testability | F01, F02, F03, F05, F09, TK1, U18, W01 |

## Product-area coverage (from the Prompt 6 brief)

foundation F01-F09; Tauri shell F04, L02-L04; production composition root W01; React frontend F05, U01-U19; Rust core O01, S01-S17; typed IPC F08; SQLite F06; event journal F06, O01; project import S01; preflight S04, K10; context discovery S05; local issues T01; GitHub issues T02, S07; DAG D02, S06; cycle detection D02, S06; parallelization D03; predicted write sets D03, A11; scheduler S09; worktrees G02, PV1; process execution P01-P03; agent sessions A03, A09; Matt Pocock integration SP10, A08, A10; testing F03, F09, TK1, Z03, A15; checkpointing G03, S11; review A10, D06, S11; review/fix loop S11; merge lane S12, G04, S13; integration validation S12; Git push G05, S14; GitHub PR T04; recovery R01-R08, A14; reconciliation R02; Stop All R04, R08; pause/resume R04; human intervention S15, U10; execution profiles and capacity S16, A13; Approval Broker K01; Watchdog K02; native approval path A04-A07; Windows UI Automation K04-K07; guarded visual fallback SP13, K08; approval-loop detection D07, K02; security and policy D05, X01-X05; prompt injection X01; observability F07; diagnostics X03; home UI U04; reactive dots U02; ambient gradients U03; dependency constellation U05; node states U06; issue inspector U07; timeline U08; conflict visualization U12; run controls U09; accessibility U13; reduced motion U14; graphics quality U03; performance PF1, U17; large graphs U17; installer Z01; signing H04; clean-machine test environment H05; clean-machine validation Z07; updates Z02; supply-chain gates X05; Desktop-hosted-session decision H06; release validation Z05, Z06.

## Publication plan (deferred)

Publication is **not** performed in this phase. After the user confirms, publish as follows (hard to undo in bulk, so confirm with the user first):

1.  Verify `gh auth status` and the target repository (`Jaweria-Abdul-Hameed/vela`); the repository has no issues yet.
2.  Commit `SHARED_SURFACE_PROTOCOL.md` and the graph files (already in the repository) so every issue body can link to them.
3.  Create the labels used in `ISSUE_GRAPH.json` (type, area, risk, phase, `ready-for-agent`, `ready-for-human`, `needs-user-assist`).
4.  Create issues in topological order (blockers first) from `tickets/<KEY>.md`, replacing symbolic blocker keys with real issue numbers and throttling to respect GitHub rate limits.
5.  Create native blocked-by links for every published edge (verify the dependency API on the first pairs; fall back to text references if unavailable).
6.  Record the key-to-issue-number map here, rebuild the DAG from the real issue ids, and update `docs/project/CURRENT_STATE.md`.
7.  Do not close or modify any parent issue.

## Notes on the process

-   The `to-tickets` skill (installed under `~/.agents/skills`) is user-invoked only. Its documented process (vertical slices, blocking edges, user approval, publish in dependency order with native links and the `ready-for-agent` label) was followed manually and not invoked. The skill advises omitting file paths; the brief explicitly requires expected write surfaces, which are given as crate and package paths.
-   The `/setup-matt-pocock-skills` tracker configuration has not been run; it edits `CLAUDE.md` or `AGENTS.md` and writes `docs/agents/*.md`, and must be handled on a separate branch under the preflight bootstrap rule.
-   The Prompt 7 audit rebuilt the DAG from the ticket files with its own parser and found it identical to the declared graph before any correction; all differences listed in AUDIT_PROMPT7.md are deliberate corrections.
