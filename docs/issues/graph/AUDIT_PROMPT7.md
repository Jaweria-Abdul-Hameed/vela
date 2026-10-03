# Prompt 7 Independent Audit Record

Date: 2026-10-02. Scope: all 130 Prompt 6 tickets, audited against the specifications. Nothing was published to GitHub and no code was written.

## Result

- Method: the DAG was rebuilt from the ticket files with an independent parser and matched the declared graph exactly (238 edges, no cycles, no redundant edges). All differences below are deliberate corrections.
- Now: **135 tickets**, **274 published edges**, no cycles, critical path still **17 tickets** (F01 > F02 > F04 > F05 > F08 > S01 > S03 > S04 > S08 > S09 > S10 > S11 > S12 > S14 > Z03 > Z05 > Z06), initial frontier **14** (F01, H01, SP01, SP02, SP03, SP04, SP06, SP07, SP08, SP09, SP10, SP11, H04, H05).
- New tickets: F09 (port fakes; split from F03), W01 (production composition root), X05 (CI secret and supply-chain gates), H04 (signing credentials), H05 (clean Windows test machine; Windows Sandbox is unavailable on Home), H06 (decide Desktop-hosted sessions for approval delivery; independent of ADR-020).
- New normative doc: SHARED_SURFACE_PROTOCOL.md (module skeleton, lockfiles, reserved migration blocks and table ownership, command registry, composition root, fault points, resource keys, shared documents, orchestrator extension points).
- Edges added: 55; removed or made implied: 21 (counted against the Prompt 6 graph). Two further edges left the graph because their human actions were resolved before publication: H02 to U19 and H03 to PF1, and one edge was added after the H01 research, S15 to S16 (238 + 55 - 21 - 2 + 1 = 271 at that point; see the H04 and H05 follow-ups below for the current total). See ISSUE_GRAPH.md for the full table and ISSUE_GRAPH.json for raw and reduced edges.
- Declared write-surface overlaps between tickets with no dependency path: 31 found, 0 remain unprotected.
- Ticket quality: feature/infra tickets with a placeholder unit-test line fell from 65 to 23 (the rest are UI or manual-evidence tickets); single-bullet acceptance criteria fell from 86 to 2 (non-spike).

## Main findings

1. No ticket wired real adapters into the app (added W01; approval wiring owned by K07, Antigravity wiring by A16).
2. 17 tickets need tables but only 8 declared migrations, and serialization claims had no edges (reserved migration blocks plus table ownership).
3. A01 and A08 planned to persist from adapter crates, which the dependency rules forbid (tables moved to the preflight service, S04).
4. Missing blockers: R01 for S10; R02 and SP11 for S16; S11 for R04; K01 for R03 and X04; S11, A10, A11 for A12; G02 for A08; G05 and T02 for S04; U11 for K10; S08 for U11; X02 before U07, U08, U10; U15 before L01; U01 before S03, S06, U02; U07 and U14 before U17; U04, U14, U17 before U18; W01 for Z03 and A16; A07 for A16.
5. Eighteen tickets had no dependents, so release validation could finish without them (8 now block Z05).
6. SP09 and SP11 gated nothing (SP09 now informs H06, the approval-delivery decision; SP11 now gates S16 and A14).
7. Real-account spikes would confound each other (resource keys and a recommended order: SP08 first, SP11 last).
8. Packaging gaps: signing and updater keys (H04), clean machine (H05), vela-hook sidecar and uninstall behavior (Z01), N-1 to N upgrade test (Z02), CI secret scanning (X05).
9. F03 was oversized (split); F06 needed FaultPoint (mechanism moved to F02); ticket.inspect, timeline.get, evidence.get had no owner (S10, S11); Antigravity-specific recovery had no owner (A14); global Stop All shortcut had no owner (L02).
10. Stale or contradictory notes (A10, L03, R04, R01 acceptance vs scope, duplicate scope in L02/L04, X01/X04/D05).

## Remaining human decisions

1. H06: does v1 create Desktop-hosted Antigravity sessions for approval delivery (UI Automation path scope; affects K07 and the real-Desktop scope of AT-011 and AT-017 in Z04)? Independent of ADR-020: it does not decide FR-050, CAP-12, or AT-026 and does not authorize Desktop GUI automation to create conversations. Kept open until shortly before K07; provisional posture recorded in H06 and CURRENT_STATE.md.
2. H01 (OPEN, PARTIALLY-DOCUMENTED, **release gate only**: it gates Z06 and nothing earlier; development proceeds under documented uncertainty, which is not a conclusion that Google permits the pattern): an authoritative written Google answer is still wanted (see `docs/research/H01_google_position.md`).
3. ~~H02~~ resolved: see "H02 resolution" below.
4. ~~H03~~ resolved: see "H03 resolution" below. Still open and intentionally deferred: H04 (production release signing only; the production Authenticode route, spend, identity validation, and production updater key and custody are decided at the release-signing phase; it gates Z06 and does not block Z01 or Z02) and H05 (route decided: VMware Workstation Pro with a Windows 11 Enterprise Evaluation guest; not yet provisioned; provisioned when Z07 is about to start; it gates Z07).
5. Confirm publication to GitHub (not done).

## Coverage matrix: functional requirements

| FR | Tickets | Acceptance tests | Spike or human gates (transitive) |
|---|---|---|---|
| FR-001 | S01, U04 | n/a (verified by the listed tickets) | none |
| FR-002 | A01, S04, U11, K10 | n/a (verified by the listed tickets) | SP02, SP03, SP04, SP06, SP08, SP10 |
| FR-003 | S05 | n/a (verified by the listed tickets) | none |
| FR-004 | SP10, A08, K10 | n/a (verified by the listed tickets) | SP02, SP03, SP04, SP06, SP08, SP10 |
| FR-005 | T01, T02, S06, S07 | n/a (verified by the listed tickets) | none |
| FR-006 | D02, S06 | n/a (verified by the listed tickets) | none |
| FR-007 | D03, S06, A11 | AT-003 | SP03, SP10 |
| FR-008 | D02, D03, S08, U12, S09, U11 | AT-002, AT-003 | none |
| FR-009 | SP03, P02, A03, A09, S10, A16 | AT-001 | SP03, SP04, SP05, SP06, SP07, SP08, SP10 |
| FR-010 | G02 | AT-001, AT-002 | none |
| FR-011 | SP10, A03, A09, S10, A16 | AT-001 | SP03, SP04, SP05, SP06, SP07, SP08, SP10 |
| FR-012 | P01, U08, S10, W01 | AT-001 | none |
| FR-013 | SP07, SP10, D06, A10, U08, S11, A16 | AT-001, AT-004 | SP03, SP04, SP05, SP06, SP07, SP08, SP10 |
| FR-014 | D06, S11 | AT-001, AT-004 | none |
| FR-015 | G03, S11 | AT-001, AT-004, AT-005 | none |
| FR-016 | G05, T03, S14 | AT-001, AT-021 | none |
| FR-017 | G04, S12, S13 | AT-001, AT-002, AT-018 | none |
| FR-018 | S12 | AT-001, AT-018 | none |
| FR-019 | D01, F06, O01, R01, R02, A14, R06, R03 | AT-005, AT-022 | SP03, SP08, SP11 |
| FR-020 | D01, S15, U10 | AT-010 | none |
| FR-021 | SP03, D01, P02, R04, L02, R08, U09 | AT-006, AT-019 | SP01, SP03 |
| FR-022 | D01, R01, R02, A14, R04, R08 | AT-005, AT-006 | SP03, SP08, SP11 |
| FR-023 | F06, F08, T02, O01, U07, U08, U16 | n/a (verified by the listed tickets) | none |
| FR-024 | SP11, S16, A13 | AT-007 | SP03, SP11 |
| FR-025 | D04, F05, U01, U05, U06 | AT-008 | none |
| FR-026 | U01, U02, U03, U06 | AT-008 | none |
| FR-027 | U02, U03, U15, U14, U17, PF1 | AT-009 | none |
| FR-028 | S17 | AT-023 | none |
| FR-029 | SP07, F02, A02, A09, A10 | n/a (verified by the listed tickets) | SP03, SP07, SP10 |
| FR-030 | D05, X04 | AT-012, AT-020 | SP02, SP03, SP04, SP06, SP08 |
| FR-031 | SP05, D05, K01, A07, K09 | AT-011, AT-012 | SP03, SP04, SP05, SP06, SP08 |
| FR-032 | K02 | n/a (verified by the listed tickets) | none |
| FR-033 | SP02, H06, K05, SP12, SP13, K06, K08 | AT-011, AT-014, AT-017 | H06, SP02, SP09, SP12, SP13 |
| FR-034 | D07, K02 | AT-013 | none |
| FR-035 | X05, F07, A04, D07, X03, K01 | AT-011 | none |
| FR-036 | SP04, A06, K10 | AT-015 | SP02, SP03, SP04, SP06, SP08, SP10 |
| FR-037 | SP11, A02, A01, TK1, A03, A15, K10, A14 | n/a (verified by the listed tickets) | SP02, SP03, SP04, SP06, SP08, SP10, SP11 |
| FR-038 | SP04, SP06, D05, A04, A05, A06 | AT-011, AT-015 | SP03, SP04, SP06, SP08 |
| FR-039 | K05, A04, K01 | AT-011, AT-012, AT-014 | SP02 |
| FR-040 | H06, S02, U15, L01 | AT-011, AT-016 | H06, SP09 |
| FR-041 | H06, P03, K07 | AT-017 | H06, SP02, SP09 |
| FR-042 | SP01, S02, L03, L01, R07, L02 | AT-019 | SP01, SP03 |
| FR-043 | G01, S03, X01 | AT-020 | SP03, SP10 |
| FR-044 | SP09, G02, PV1 | AT-001 | SP09 |
| FR-045 | T04, S08, U16, S14 | AT-021 | none |
| FR-046 | F06, R05 | AT-022 | none |
| FR-047 | H04, H05, F04, Z01, Z02, Z07 | AT-024 | SP01, SP02, SP03, SP04, SP06, SP08, SP10, H04, H05 |
| FR-048 | F04, F08, X02 | n/a (verified by the listed tickets) | none |
| FR-049 | A01, K10 | AT-025 | SP02, SP03, SP04, SP06, SP08, SP10 |
| FR-050 | SP08, A05, A12 | AT-026 | SP03, SP07, SP08, SP10 |

## Coverage matrix: acceptance tests

| AT | Tickets | Evidence tickets | Requirements |
|---|---|---|---|
| AT-001 | S01, PV1, S04, S10, A16, Z03, Z04 | A16, Z03, Z04 | FR-009, FR-010, FR-011, FR-012, FR-013, FR-014, FR-015, FR-016, FR-017, FR-018, FR-044 |
| AT-002 | S09, S12, Z03 | Z03 | FR-008, FR-010, FR-017 |
| AT-003 | D03, U12, S09, U11, Z03 | Z03 | FR-007, FR-008 |
| AT-004 | A10, S11, Z03 | Z03 | FR-013, FR-014, FR-015 |
| AT-005 | F03, A03, R02, R03, Z03 | F03, R03, Z03 | FR-015, FR-019, FR-022 |
| AT-006 | R08, U09, Z03 | Z03 | FR-021, FR-022 |
| AT-007 | S16, A13, Z03 | Z03 | FR-024 |
| AT-008 | U01, U02, U04, U05, U06, U07, U18, U19 | U18, U19 | FR-025, FR-026 |
| AT-009 | U02, U03, PF1 | PF1 | FR-027 |
| AT-010 | S15, U10, Z03 | Z03 | FR-020 |
| AT-011 | K04, SP12, K06, A07, Z04 | K04, Z04 | FR-031, FR-033, FR-035, FR-038, FR-039, FR-040 |
| AT-012 | K01, X04 | X04 | FR-030, FR-031, FR-039 |
| AT-013 | K02, Z04 | Z04 | FR-034 |
| AT-014 | K04, SP12, K05 | K04, K05 | FR-033, FR-039 |
| AT-015 | A06, A15, K10, Z04 | A15, K10, Z04 | FR-036, FR-038 |
| AT-016 | K06, L01 | L01 | FR-040 |
| AT-017 | K04, P03, K07, Z04 | K04, Z04 | FR-033, FR-041 |
| AT-018 | S12, S13, Z03 | Z03 | FR-017, FR-018 |
| AT-019 | R07, L02, R03, Z03 | R03, Z03, Z06 (full-product clean-machine run) | FR-021, FR-042 |
| AT-020 | S03, Z03 | Z03 | FR-030, FR-043 |
| AT-021 | T04, S14, Z03 | Z03 | FR-016, FR-045 |
| AT-022 | R05, Z03 | Z03 | FR-019, FR-046 |
| AT-023 | S17 | S17, Z06 (clean-machine notification run) | FR-028 |
| AT-024 | Z02 | Z02, Z07 (clean-machine upgrade run) | FR-047 |
| AT-025 | A01, A15, X04, Z04 | A15, X04, Z04 | FR-049 |
| AT-026 | SP08, A12, Z04 | Z04 | FR-050 |

## Parallel groups by layer

Layers are topological (blockers all in earlier layers). Within a layer, tickets listed together are safe to run together **under SHARED_SURFACE_PROTOCOL.md** (module skeleton and pre-seeded dependencies from F01/F02, reserved migration blocks, per-group command files, per-concern wiring files, open-ended Orchestrator registries). Where a protocol item is not yet delivered, those tickets serialize. Checked for each group: file/module overlap, API/type contracts (ports frozen by F02), schema (reserved blocks, FK order by block), configuration (tauri.conf.json keys, capabilities per surface), lockfiles (pre-seeded; a new dependency is a dependency request), generated artifacts (per-type ts-rs files, wildcard export, regenerate on conflict), shared fixtures (per-ticket subdirectories), architectural coupling, and semantic dependency. A mechanical check of declared write surfaces across all pairs without a dependency path found no overlap outside the protocol-managed surfaces.

| Layer | Safe together | Serialized or conditional, and why |
|---|---|---|
| 0 | F01 with the human items H01, H04, and H05; SP03 with SP01 or SP02 | Spikes using the real account (SP04, SP06, SP07, SP08, SP09, SP10, SP11) run one at a time, order SP08, SP09, SP04, SP06, SP07, SP10, SP11 (SP08 observation is confounded by any other CLI conversation; SP11 provokes quota and self-update). SP01 and SP02 both move the foreground window: not together. User attention is a single resource. |
| 1 | F02, K04, X05 | SP05 after SP04 (real account). H06 waits for SP09 only (SP08 is a conversation-visibility spike under ADR-020 and is not a blocker). F02 owns the port signatures: no domain ticket starts before it merges. |
| 2 | F03, F04, F06, F07, D01, D02, D05, D06, P03, T01, K05 | Domain tickets share a crate but own disjoint modules and distinct generated files. SP12 and K05 real-Desktop parts share the foreground window and real account: sequential. |
| 3 | A02, A04, F05, L04, P01, D03, D04, D07, F09 | SP13 is analysis only. L04 may overlap Z01 (layer 4) only because they edit disjoint tauri.conf.json keys. |
| 4 | Z01, TK1, F08, X02, U01, P02, G01, T02, A01 | TK1 and T02 share vela-testkit but own separate directories. F08 owns the generated permission manifests, X02 only hand-written capability grants. |
| 5 | U02, A05, S01, S02, X03, O01, G02, G03, G05, T03, A03 | A05 and A03 may be coded together but their gated real runs hold the real-account key one at a time. G02, G03, G05 use the lock and separate impl files from G01. S01, S02, O01, T03 use disjoint migration blocks. |
| 6 | S05, U04, S03, L03, K06, U03, U15, S06, S15, S17, R01, G04, R05, A08, T04, A06 | Orchestrator modules plug in through O01 registries, not shared enums. K06 manual real delivery and A06 gated runs hold the real-account key. |
| 7 | L01, S04, PV1, K08, S07, U05, K01, U10, A09, A15 | A15 holds the real-account lock. K08 may close as dropped (SP13). |
| 8 | X04, S08, U06, U07, U12, A07, K02, K09, A10, A11, X01 | A07, A10, A11 gated real runs serialize on the real-account key. X01 and X04 use separate test subdirectories. U06 and U07 extend the canvas interface through separate directories. |
| 9 | U11, S09, U14, U08, U13, U16 | S09 continues the Orchestrator chain alone. U13 edits only its own directory and files findings against others. |
| 10 | K10, U17, S10 | None beyond the Orchestrator chain (S10). |
| 11 | PF1, U18, S11, R02, W01 | PF1 needs the machine quiet (RK-QUIET-MACHINE): run it alone. W01 smoke holds the foreground window. |
| 12 | S16, R04, A12, U19, S12, R06, R07, A14, K07 | U19, A12 manual evidence and R06 sleep and wake evidence share the one human. |
| 13 | A13, L02, U09, Z02, S13, S14, R03, A16, R08 | A16 holds the real account. S13, S14, R08 add crash-matrix entries for their own fault points. |
| 14 | Z03, Z04, Z07 | Disjoint paths; Z04 needs the user and the real account; Z07 holds the clean machine (the H05 VM, a quiet-machine workload on the 16 GB host: not run with PF1, heavy agent work, or large WSL workloads) and the foreground window. |
| 15-16 | Z05, Z06 | Sequential by definition. |

## Explicitly serialized by dependency edge (not parallel despite looking independent)

- Orchestrator chain S08, S09, S10, S11, S12, then S13 and S14: shared message types, transition tables, and one state machine.
- vela-git: G01 (lock and read) before G02, G03, G05; G04 after G02 and G03.
- Broker: K01 before K02, K09, A07; K06 after K05; K07 after K06, K02, H06.
- Canvas: U01, U02, then U03; U05 before U06, U07, U11, U12; U07 and U14 before U17; U17 and U04 and U14 before U18.
- Preflight panel: S04, then U11 (with S08), then K10.
- Settings UI: U15 before L01.
- Release: Z01 before Z02; X05 and H04 before Z06 (H04 no longer blocks Z02: Z02 uses a disposable test updater keypair).

## Residual risks and deliberate non-changes

- S12 is the largest ticket; a split point is documented in the ticket instead of splitting up front.
- Small tickets (L03, D06, D07, P02) were kept: each has distinct evidence and a single mental model.
- The first full vertical slice with real adapters is W01 (layer 11); the first fake-agent worker slice is S10 (layer 10). Integration risk lands late; Prompt 8 should look at this.
- K03 is only a numbering gap.
- Human attention is the early bottleneck mainly through the spikes and H01. H04 and H05 sit in the initial frontier only because they have no blockers: H04 is a release-signing decision (gates Z06) and H05 provisions the clean-machine VM when Z07 is about to start. H06 is not in the frontier (it depends on SP09) and is decided shortly before K07.

## Follow-up correction: H06 and ADR-020 (applied after review)

- H06 was retitled "Decide whether v1 creates Desktop-hosted Antigravity sessions for approval delivery (UI Automation path scope)" and now depends on SP09 only. The SP08 to H06 edge was removed because SP08 answers a conversation-visibility question (ADR-020), not an approval-delivery question.
- H06 states explicitly that it does not decide FR-050, CAP-12, or AT-026, does not reinterpret or reopen ADR-020 decision 1a, does not change the requirement that in-scope conversations be fresh where required and discoverable and openable in Antigravity Desktop, and does not authorize Desktop GUI automation to create or host conversations. Desktop-visible or openable does not mean Desktop-created or Desktop-hosted. ADR-020 decision 2 remains authoritative: GUI automation for conversation creation is a last resort needing a separate explicit decision, and only if SP08 shows that no supported route can satisfy ADR-020.
- Provisional posture until H06 is resolved: headless or programmatic agy is the primary orchestration path; the UIA approval adapter may be implemented and tested against the controlled fake approval window; the real-Desktop approval-delivery acceptance scope is unresolved; none of this weakens, defers, or substitutes for ADR-020, FR-050, CAP-12, or AT-026.
- K07 and the real-Desktop approval-delivery scope of Z04 (AT-011, AT-017) stay gated by H06. A05, A12, A15, and AT-026 do not wait on H06.

## H02 resolution (Stitch reference supplied)

- The user supplied the actual reference at `docs/ui/reference/vela-stitch-reference.png` (PNG, 2556 x 1483). It was inspected directly on 2026-10-02 and is recorded in DOCUMENTATION_INDEX.md, docs/ui/REFERENCE_BRIEF.md, and CURRENT_STATE.md. The pixels were not touched.
- Convention followed: a resolved human action is not published as an issue. H02 was removed from the graph (as the Prompt 6 plan only publishes work still to be done) and its only edge, H02 to U19, was dropped. U19 and AT-008 remain. Counts at that point (superseded by the H03 resolution below): 135 tickets, 271 published edges, initial frontier 15; the critical path (17 tickets) and all depths were unchanged.
- Inspection findings (full text in REFERENCE_BRIEF.md): the image is a still of the Stitch landing page that supports the near-black canvas, sparse micro-dot lattice, large edge-concentrated violet-to-blue light masses, oversized clean type, one restrained glass panel, minimal chrome, no sidebar, no card grid, no neon, no heavy bloom, and no 3D decoration. It is silent on the graph, interaction, camera, states, and all non-visual behavior. Differences from the prose (recorded, not resolved): magenta tints and sky-blue-leaning cyan beyond the named spectrum; structured ridge and streak texture within the soft masses; light-to-regular display weight versus "strong"; one opaque white primary action versus translucent controls; pointer response and ambient motion cannot be seen in a still. No frozen specification contradicts the image.
- Tickets changed: U19 (reviews against the asset path, inspects the PNG itself, compares the page area only, written specifications govern where the image is silent, no H02 blocker), and U01, U02, U03, U04 (the asset path in their documents and side-by-side manual evidence). No other UI ticket needed correction. The parallel-safety result is unchanged (zero unprotected overlaps) and H06 stays independent of SP08 and of ADR-020.

## H03 resolution (reference performance hardware)

- The user designated their current development laptop as Vela's canonical reference machine. Its identity was verified with read-only system queries on 2026-10-03 and recorded as profile `REF-HW-1` in the new "Reference Hardware Profile" section of `docs/ui/PERFORMANCE_BUDGET.md` (appended; no budget, graphics mode, or other existing text was changed) and in CURRENT_STATE.md.
- Convention followed (the same as H02): a resolved human action is not published as an issue. H03 was removed from the graph and its only edge, H03 to PF1, was dropped. Counts at that point (superseded by the H01 follow-up below): 134 tickets, 270 published edges (340 authored); the initial frontier was 14; the critical path (17 tickets) and all depths are unchanged.
- Verified directly: Windows 11 Home 25H2 build 10.0.26200.9550; Intel Core 9 270H (14 cores, 20 threads); one 16 GB DDR5-5600 module (single-channel); NVIDIA GeForce RTX 5070 Laptop GPU with 8 GiB, driver 616.92, plus an Intel integrated GPU that reports the display mode; 2560 x 1600 at 180 Hz, about 15.9 in, scaling 150% (device pixel ratio 1.5); power scheme "Acer"; WebView2 154.0.4258.53; 1 TB NVMe. Supplied by the user and not verified: the marketing name "Nitro 5" (Windows reports Nitro ANV16S-71), GPU power limit and any vendor performance mode, and the intent about future RAM.
- Policy recorded: 16 GB is the current configuration. A RAM, CPU, GPU, panel, or machine change creates a new profile and never rewrites REF-HW-1 or its results. Every benchmark records the profile ID and a per-run environment capture. Budgets stay canonical and are not weakened; Full, Balanced (default), and Efficiency are each measured; no benchmark numbers were invented.
- Approved follow-up decisions (2026-10-03): REF-HW-1 stays in `docs/ui/PERFORMANCE_BUDGET.md`. Reference runs that count against the budgets require the REF-HW-1 configuration, AC power, a stable recorded Windows power scheme (name and GUID), no deliberate competing heavy workload, and a full environment capture including the actual rendering GPU, driver and WebView2 versions, graphics mode, reduced-motion state, window state, effective DPR and cap, display mode, and any reliably determinable Acer or Nitro performance mode. No Acer or Nitro mode is mandated (no reliable programmatic fact yet). Non-reference exploratory runs may differ but must be labelled and are never release evidence. No budget changed and no result was invented.
- Graph tooling note: the generator and patch scripts were temporary scratch files outside the repository (nothing tracked contains a machine-specific path); they are disposable and must not be reused. This audit's results are validated by an independent rebuild from the ticket files, not by those scripts.
- Tickets changed: H03 removed; PF1 (blocker H03 dropped, profile and per-run capture requirements added, mode and condition coverage stated, reference versus non-reference labelling added). No other ticket needed correction. The parallel-safety result is unchanged (RK-QUIET-MACHINE still serializes PF1 and the U17 measurement runs) and the H06 and H02 corrections are untouched.

## H01 follow-up (provider-policy-block handling; decisions of 2026-10-03)

- Decisions of record: H01 stays OPEN with the evidence classification PARTIALLY-DOCUMENTED and is a **release gate only** (it gates Z06; no H01 to A03 or other implementation edge). Implementation may proceed under documented uncertainty, which is not a conclusion that Google permits the pattern. An authoritative written Google answer is wanted (draft question in `docs/research/H01_google_position.md`; nothing was sent). The Gemini API-key route is out of v1 scope and ADR-016 is unchanged. See the record for the exact remaining action.
- Requirement placement: the behavior extends **FR-024** (capacity handling already forbids circumventing provider restrictions and owns profile availability) and **AT-007** (retitled "Capacity interruption and provider policy block"). No new FR or AT was created, so FR-001..FR-050 and AT-001..AT-026 coverage and the traceability tables are unchanged.
- Semantics (CAPACITY_AND_PROFILES.md "Provider Policy Blocks"): neutral typed error `PROVIDER_POLICY_BLOCK` (class POLICY_BLOCK, origin provider) carried by the neutral `ProviderPolicyBlock` event; profile state `POLICY_BLOCKED`; worker and run `NEEDS_HUMAN` with kind `PROVIDER_POLICY_BLOCK`; no retry, backoff, profile or account switch, credential change, API-key fallback, or new conversation; persisted and restored on restart with no agent work started; cleared only by an explicit user decision after one user-initiated access re-check. The Antigravity signal mapping is `[U]` (never provoke a block; documented or recorded evidence and labelled synthetic fixtures only; unclassifiable access failures are never retried).
- Tickets changed (text only; write surfaces unchanged): F02 (typed error, event, journal kinds), D01 (kind), S15 (kind, resolution precondition), S16 (state, behavior, persistence, clearing, tests; new blocker S15), A13 (Antigravity mapping and fail-safe), U10 (card variant), F09 (scripted scenario), S09 and R02 (consult the availability service), H01 (status and gating). Specification documents changed: PRODUCT_SPEC.md (FR-024), ACCEPTANCE_TESTS.md (AT-007), CAPACITY_AND_PROFILES.md, ORCHESTRATION_ENGINE.md, HUMAN_IN_THE_LOOP.md, ERROR_HANDLING.md, COMPONENT_SPECIFICATIONS.md, SYSTEM_ARCHITECTURE.md, RECOVERY.md, SHARED_SURFACE_PROTOCOL.md.
- Graph: one edge added, **S15 to S16** (S16 creates its intervention through the S15 model; S15 is not an ancestor of S16, so the edge is not implied). Counts: 134 tickets, **271** published edges (**341** authored). Depths (S16 stays 12), the critical path (17 tickets), the initial frontier (14), and every write surface are unchanged. No ticket was created.

## H04 follow-up (production signing is a release prerequisite; decisions of 2026-10-03)

- Decisions: H04 stays OPEN as a **production-release prerequisite only**. Development and testing use unsigned local builds, throwaway self-signed development certificates, disposable test updater keypairs, or synthetic fixtures; none of it is ever production trust material. **Tauri updater artifact signing** (its own keypair, required by the updater) and **Windows Authenticode** (signs the installer and Windows binaries, route undecided) are separate mechanisms and every ticket names which one it means. Production Authenticode provider or route, purchase and spend, identity validation, production credentials, production updater private-key generation and custody, and the final updater hosting location stay deferred and are the user's; no vendor was chosen, nothing was purchased, no key was generated, and Smart App Control was not changed (it was observed ON on the development machine and may affect unsigned or self-signed development artifacts; surfaced, never weakened).
- Ownership after the correction: **Z01** owns the NSIS build and the Authenticode signing mechanism with development signing (`tauri.dev-signing.conf.json`, `DEV_SIGNING.md`) and a production profile that fails closed; **Z02** owns the updater and its tests with a disposable test keypair (`tauri.updater-test.conf.json`, `UPDATER.md`) and a release profile that rejects the test key; **H04** owns the production decisions and material and the non-secret record (`SIGNING.md`); **Z06** owns executing and verifying the production release-signing gate: the Authenticode signature and the updater signature are verified separately against the identifiers H04 recorded, with no development or test material present.
- Graph: edge **H04 to Z02 removed** (Z02 no longer needs production material, so the edge was not a true implementation blocker) and edge **H04 to Z06 added** (explicit release gate; H04 was previously only transitively an ancestor of Z06 through Z02 and Z05). Net published edges unchanged at **271**, tickets **134**; the authored count was reported as 341 here in error (Z06 already carried H04 as an authored edge, so the second addition was a duplicate): the correct authored count at that point was **340**; depths, critical path (17 tickets), and frontier (14, with H04 still in it because it has no blockers) unchanged. No ticket was created.
- Documents changed: PRODUCT_SPEC.md (FR-047 wording), IMPLEMENTATION_ARCHITECTURE.md (section 12), RELEASE_CHECKLIST.md (separate Authenticode and updater items), SHARED_SURFACE_PROTOCOL.md (section 5 overlay files), CURRENT_STATE.md (H04 and H05 listed), and the new research record. ADR-017 is unchanged: its "signed" and "signature keys" wording is exactly these two mechanisms. Tickets changed (text and write surfaces for new files only): H04, Z01, Z02, Z06, H05.

## H05 follow-up (clean-machine route decided; environment not provisioned; decisions of 2026-10-03)

- Decisions: the canonical clean-machine environment is a local Windows 11 virtual machine under **VMware Workstation Pro** on the development machine (selected for the snapshot and revert workflow, usable on Windows 11 Home, and documented by Broadcom for operation when the Windows Hypervisor Platform or VBS is active; VirtualBox stays a possible alternative whose coexistence evidence was partly secondary). The guest is the official Microsoft **Windows 11 Enterprise 90-day Evaluation**: an Enterprise guest, not Home; time-limited; disposable; not a licensing decision for permanent infrastructure; another edition is tested only if release validation deliberately requires it. A spare physical machine is optional supplementary validation. Initial VM configuration (configurable, not a performance requirement): 4 vCPU, about 4 to 6 GB RAM, a dynamic 60 to 100 GB disk, virtual TPM 2.0 and Secure Boot, no nested virtualization, no GPU passthrough; a quiet-machine workload on the 16 GB host. **H05 stays OPEN: the route is decided, the environment is not provisioned**, and nothing was installed, downloaded, created, or changed on the host.
- States: **S0** pristine baseline (no Rust, Node, Git, Antigravity, or Vela; VMware guest tools recorded as part of the baseline; build, edition, and security state recorded); **S1** N-1 installed (from S0); **S2** Git installed (from S0). A state is created only when a scenario needs it. **WebView2:** removal is not a prerequisite (Windows 11 includes the Evergreen runtime and no supported removal is established); the configured bootstrap, embedded, and offline behavior is tested where technically possible, and a missing-runtime path that cannot be obtained by a supported method is recorded as **NOT VERIFIED**. **Smart App Control:** not mandated on or off; the actual state is recorded for every relevant run, and a rejection of a development or self-signed artifact under an enforcing posture is expected evidence. **Artifact transfer:** SHA-256 manifest and artifact built outside the guest, moved by a read-only virtual disk or ISO, verified in the guest with built-in Windows facilities; no permanent shared folder, drag-and-drop, or clipboard dependency; network off unless a scenario needs it; revert after each scenario.
- Graph restructuring (the smallest coherent change): H05 no longer blocks Z01, because Z01's code work (building and development-signing the installer) needs no VM and only its evidence did. A new ticket **Z07** (Clean-machine installer and lifecycle validation on the H05 environment) owns the environment-dependent evidence: pristine install, no-development-tooling listing, missing-prerequisite first launch, development Authenticode verification, uninstall, reboot, tray, auto-start, and notification evidence (AT-019, AT-023), the real-machine N-1 to N run (AT-024 evidence, executing Z02's updater), and the WebView2 status. Z01 keeps the installer, uninstall configuration, and development signing; Z02 keeps the updater and its simulation tests and produces the N-1 and N artifacts for Z07; Z06 keeps the final production release gate and the production-signed clean-machine run; none is duplicated. Z07 feeds Z05 and therefore Z06, so a release cannot complete without the clean-machine evidence.
- Edges (published) as first generated (superseded by the final audit below): removed H05 to Z01, Z02 to Z05, L01 to Z05, L03 to Z06; added Z07 to Z05 and nine edges into Z07 (from Z02, H05, W01, L02, L03, R07, K10, L01). That generation gave 135 tickets, 350 authored edges, 276 published edges; the final audit removed L02 and R07 as Z07 blockers (below) for 347 authored and 274 published.
- Documents changed: RELEASE_CHECKLIST.md (explicit clean-machine and WebView2 evidence statuses), SHARED_SURFACE_PROTOCOL.md (resource keys and the clean-machine test directory), CURRENT_STATE.md, the new research record `docs/research/H05_clean_machine_2026-10-03.md`, and the tickets Z01, Z02, Z05, Z06, Z07 (new), and H05.

## Final Prompt 7 audit (2026-10-03)

- Method: the audit was rebuilt from the ticket Markdown only (headers, section 7, section 3, section 8) with a new script; `ISSUE_GRAPH.json` was cross-checked, not trusted. It confirmed the ticket set, header, section 7, and JSON blockers agree, no cycles, no self, unknown, or duplicate edges, no redundant published edge, authored and published edge closures identical, depths, frontier, and critical path.
- **Z07 blockers re-derived from the evidence Z07 actually owns** (the earlier set was over-broad). Final published blockers: **Z02** (the real-machine N-1 to N run executes Z02's updater and uses its artifacts; implies Z01), **H05** (the environment), **W01** (production wiring so first launch runs the real preflight on real adapters), **K10** (the Antigravity-missing and authentication preflight providers shown on first launch), **L03** (the login auto-start entry whose presence, launch at login, and removal at uninstall Z07 evidences), **L01** (onboarding creates the persisted settings and the auto-start choice, so first launch and the state preserved across the upgrade are the shipping flow). **Removed: L02 and R07** (and the implied S17): tray and background operation, reboot continuation of a real run, and notification evidence need the complete product running a real workload, so they moved to **Z06**, which already executes those checklist items; Z06 now states them as acceptance criteria and holds `RK-CLEAN-MACHINE`, `RK-FOREGROUND-WINDOW`, and `RK-USER-ATTENTION` for them. L02, R07, and S17 are already ancestors of Z06 (through Z03, W01, and Z05). Z07 depth stays 14 and is not on the critical path; the Z07 ticket now says explicitly that it waits only for the capabilities its own evidence exercises.
- Other corrections: **K05** now lists AT-014 (it implements and tests "unrelated Approve buttons are never matched" and non-correlated windows; K04 is only the harness); **Z03** now exercises the AT-007 provider-policy-block scenario (it listed AT-007 but only the capacity half); the protocol's `RK-USER-ATTENTION` and `RK-FOREGROUND-WINDOW` rows now include Z07 and Z06; the audit matrices list Z06 as evidence for AT-019 and AT-023 and Z07 for AT-024; CURRENT_STATE, DOCUMENTATION_INDEX (ticket count and the new records), and the stale human-decision wording in this file were updated.
- Write-surface audit: 58 unordered overlapping pairs under a path-prefix test, all of them the single surface `crates/vela-persistence/migrations/**` across 16 tickets, each declaring a unique reserved block that matches the protocol table (protected by explicit ownership partition). Every other surface is distinct or ordered by dependency. Unresolved conflicts: **0**.
- Final numbers: **135 tickets, 347 authored edges, 274 published edges**; frontier 14; critical path 17 tickets; FR-001..FR-050 and AT-001..AT-026 covered. The earlier authored counts of 341 (H04 step) and 350 (H05 step) were corrected along the way: 341 contained a duplicate edge and the 350 included L02 and R07.

## Final verdict (2026-10-03)

- **Final verdict: READY FOR CHECKPOINT.** All 75 final Prompt 7 checks passed.
- Final counts: 135 tickets; 347 authored edges; 274 published (transitively reduced) edges; initial frontier 14; critical path 17 tickets; zero unresolved write-surface conflicts; FR-001 through FR-050 covered; AT-001 through AT-026 covered.
- Human items intentionally remaining: H01 (open, partially documented, release gate on Z06); H04 (open, production signing deferred to the release-signing phase); H05 (open, VM route decided, not provisioned until Z07 approaches); H06 (open, deferred until SP09 evidence). H02 and H03 are resolved.
- No GitHub issues were published. Prompt 8 has not begun.

## Canonical Stitch reference baseline

- Asset: `docs/ui/reference/vela-stitch-reference.png` (canonical, must not be modified).
- SHA-256: `ddfc4309bd9839db194d85e349c134c4be3610d63fba5265788ba0f721027295`
- Future sessions and audits verify byte-identity with `sha256sum docs/ui/reference/vela-stitch-reference.png`.
