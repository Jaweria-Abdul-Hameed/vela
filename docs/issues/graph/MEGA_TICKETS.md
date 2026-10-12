# Mega-ticket grouping (proposal)

**Status:** PROPOSAL, not adopted. Derived mechanically from `ISSUE_GRAPH.json` (135 tickets, 277 reduced edges). Group-level dependencies below are computed from the ticket edges and verified acyclic. Done before this proposal: F01, F02, F03, D01, F07; D02 and D05 are in flight and are folded into M01.

Each mega-ticket (M01 to M32) is one autonomous session/loop iteration that implements its member tickets in the listed order, using each member ticket file in `docs/issues/graph/tickets/` as its spec (objective, scope, write surfaces, acceptance criteria). Members stay as the internal checklist; commit at each member boundary so a crash loses at most one member. M00 is the human/spike track and is not run by the loop.

| Mega | Level | Name | Member tickets | Starts after | On dogfood path |
|---|---|---|---|---|---|
| M00 | 0 | Human and spike track (you, not the loop) | SP01, SP02, SP03, SP04, SP05, SP06, SP07, SP08, SP09, SP10, SP11, H01, H04, H05, H06 | - | **yes** |
| M01 | 0 | Domain A: graph, parallel-safety, layout, policy engine | D02, D03, D04, D05 | - | **yes** |
| M02 | 1 | Domain B: review, approval domain, port fakes | D06, D07, F09 | M01 | **yes** |
| M03 | 1 | Persistence and process runtime | F06, P01, P02, P03 | M00 | **yes** |
| M04 | 2 | Trackers and CI supply chain | T01, T02, T03, X05 | M02, M03 | **yes** |
| M05 | 2 | App shell, IPC gateway, renderer, hardening | F04, F05, F08, X02, L04 | M03 | **yes** |
| M06 | 3 | UI design system and canvas base | U01, U02 | M05 | **yes** |
| M07 | 2 | Git layer: read, worktrees, checkpoints, merge, remote | G01, G02, G03, G04, G05 | M03 | **yes** |
| M08 | 2 | Antigravity driver: discovery, parser, fake agy, session driver | A01, A02, TK1, A03 | M03 | **yes** |
| M09 | 3 | Antigravity hook, profiles, skills bootstrap | A04, A05, A08 | M00, M01, M07, M08 | **yes** |
| M10 | 3 | Orchestrator base: actor, interventions, op records, import, settings | O01, S15, R01, S01, S02 | M02, M05 | **yes** |
| M11 | 4 | Orchestrator services and onboarding | S17, X03, S05, U15, L01, L03 | M05, M06, M07, M10 | **yes** |
| M12 | 4 | Trust, preflight, provisioning, PR workflow | S03, PV1, S04, T04 | M01, M04, M06, M07, M10 | **yes** |
| M13 | 5 | Graph ingestion, approval, Build start, scheduler | S06, S07, S08, S09 | M01, M04, M06, M07, M10, M12 | **yes** |
| M14 | 4 | Home scene and ambient field | U03, U04 | M06, M10 | no |
| M15 | 6 | Antigravity sessions: native policy, task envelope, reviewer, analyst | A06, A09, A10, A11 | M00, M02, M08, M09, M11, M13 | **yes** |
| M16 | 6 | Worker lifecycle and review workflow | S10, S11 | M02, M07, M10, M12, M13 | **yes** |
| M17 | 7 | Merge lane, conflicts, finalization | S12, S13, S14 | M07, M12, M16 | **yes** |
| M18 | 7 | Approval broker, headless ASK, intervention UI | K01, K02, A07, U10, K09 | M00, M02, M05, M06, M10, M15 | **yes** |
| M19 | 6 | Project universe UI | U05, U06, U07, U11, U12 | M05, M06, M13 | **yes** |
| M20 | 8 | Recovery core: reconcile, stop/pause, capacity | R02, R04, R08, S16 | M00, M03, M10, M16, M17 | **yes** |
| M21 | 9 | Run controls and evidence UI | U09, U08, U16, U14 | M19, M20 | no |
| M22 | 9 | Resilience: state rebuild, sleep/network, reboot, drift | R05, R06, R07, A13, A14 | M00, M03, M04, M07, M08, M09, M20 | no |
| M23 | 9 | Installer, composition root, tray | Z01, W01, L02 | M00, M03, M05, M07, M09, M11, M12, M16, M20 | **yes** |
| M24 | 10 | Real Antigravity end to end and Desktop visibility | A16, A12, A15 | M00, M15, M16, M17, M18, M19, M23 | **yes** |
| M25 | 4 | UIA approval core (deferrable, see H06) | K04, SP12, SP13, K05, K06 | M00, M10 | no |
| M26 | 9 | UIA late and preflight providers (deferrable) | K07, K08, K10 | M00, M03, M09, M15, M18, M19, M20, M25 | no |
| M27 | 10 | Accessibility and large-graph performance | U13, U17, PF1 | M14, M19, M21 | no |
| M28 | 8 | Security hardening and injection boundaries | X01, X04 | M12, M15, M18, M25 | no |
| M29 | 10 | Crash matrix and fixture suite | R03, Z03 | M17, M18, M20, M22, M23 | no |
| M30 | 11 | Visual regression and fidelity gate | U18, U19 | M14, M18, M19, M21, M27 | no |
| M31 | 11 | Updater, real-environment acceptance, clean machine | Z02, Z04, Z07 | M00, M11, M20, M22, M23, M24, M26 | no |
| M32 | 12 | Traceability and release candidate | Z05, Z06 | M00, M04, M05, M11, M13, M18, M21, M22, M26, M27, M28, M29, M30, M31 | no |

## Dogfood cut

The "dogfood path" column marks the 22 groups (97 tickets) that must land before a real Antigravity-backed run (A16) works end to end and Vela can build your projects. Everything not marked (M14, M21, M22, M25 to M32: home polish, run-control UI, resilience, UIA approval path, accessibility/perf, hardening, crash matrix, release gates) can follow while you use Vela. M19 and M24's A12 are on the path only because A12 (Desktop conversation visibility) needs U07; drop A12 from M24 to remove M19 from the critical route if you want an even shorter cut.

## Decisions this plan needs from you

1. **Drop review loops.** ADR-021/022 and AGENTS.md currently require one independent `/code-review` per ticket. Dropping it needs a short ADR superseding them (otherwise agents will keep following AGENTS.md). Suggested: keep self-audit against acceptance criteria and CI green; drop independent review and verification.
2. **Keep tests that ship with the code.** The ticket specs already embed TDD tests. Skipping them to save time costs more later in the policy engine, redaction, git hostility, and crash recovery areas. The test-only tickets (X04, R03, Z03, U18, A15, PF1, Z04, Z07) are what is safe to defer; they sit in M28 to M31.
3. **UIA path (M25, M26).** Whether v1 needs UI-automation approval delivery depends on H06 and the SP12/SP13 spikes. If A07 (headless ASK) is enough, M25 and M26 can be cut entirely.
4. **Spikes gate real-agent work.** M08/M09/M15/M18 depend on M00 outcomes (SP03, SP04, SP05, SP06, SP07, SP08, SP10). Run M00 now, one real-account spike at a time, so the loop is never blocked on you.
5. **Local builds.** Rust build/test is blocked on this host (Smart App Control); gates only pass in CI. An autonomous loop needs a CI-feedback step or it cannot verify anything.

## Estimated time per mega-ticket (agent wall-clock, descending)

Basis: per member ticket 0.5 h (low risk), 0.8 h (medium), 1.3 h (high), plus 20% for CI waits and fixes, no review loops. These are estimates from risk labels, not measurements; calibrate after the first two or three runs (run M01 and M03, compare to this table, scale the rest). Spikes and human actions (M00) are not included and are mostly your time.

| Mega | Est. hours | Tickets | Name | Dogfood path |
|---|---|---|---|---|
| M07 | 7.2 | 5 | Git layer: read, worktrees, checkpoints, merge, remote | yes |
| M18 | 6.6 | 5 | Approval broker, headless ASK, intervention UI | yes |
| M25 | 6.6 | 5 | UIA approval core (deferrable, see H06) | no |
| M10 | 6.2 | 5 | Orchestrator base: actor, interventions, op records, import, settings | yes |
| M08 | 5.6 | 4 | Antigravity driver: discovery, parser, fake agy, session driver | yes |
| M13 | 5.6 | 4 | Graph ingestion, approval, Build start, scheduler | yes |
| M15 | 5.6 | 4 | Antigravity sessions: native policy, task envelope, reviewer, analyst | yes |
| M20 | 5.6 | 4 | Recovery core: reconcile, stop/pause, capacity | yes |
| M22 | 5.4 | 5 | Resilience: state rebuild, sleep/network, reboot, drift | no |
| M03 | 5.0 | 4 | Persistence and process runtime | yes |
| M12 | 5.0 | 4 | Trust, preflight, provisioning, PR workflow | yes |
| M19 | 5.0 | 5 | Project universe UI | yes |
| M01 | 4.7 | 4 | Domain A: graph, parallel-safety, layout, policy engine | yes |
| M05 | 4.7 | 5 | App shell, IPC gateway, renderer, hardening | yes |
| M17 | 4.7 | 3 | Merge lane, conflicts, finalization | yes |
| M23 | 4.7 | 3 | Installer, composition root, tray | yes |
| M09 | 4.1 | 3 | Antigravity hook, profiles, skills bootstrap | yes |
| M24 | 4.1 | 3 | Real Antigravity end to end and Desktop visibility | yes |
| M27 | 4.1 | 3 | Accessibility and large-graph performance | no |
| M31 | 4.1 | 3 | Updater, real-environment acceptance, clean machine | no |
| M11 | 4.0 | 6 | Orchestrator services and onboarding | yes |
| M04 | 3.5 | 4 | Trackers and CI supply chain | yes |
| M21 | 3.5 | 4 | Run controls and evidence UI | no |
| M26 | 3.5 | 3 | UIA late and preflight providers (deferrable) | no |
| M16 | 3.1 | 2 | Worker lifecycle and review workflow | yes |
| M28 | 3.1 | 2 | Security hardening and injection boundaries | no |
| M29 | 3.1 | 2 | Crash matrix and fixture suite | no |
| M02 | 2.9 | 3 | Domain B: review, approval domain, port fakes | yes |
| M32 | 2.5 | 2 | Traceability and release candidate | no |
| M06 | 2.2 | 2 | UI design system and canvas base | yes |
| M14 | 1.6 | 2 | Home scene and ambient field | no |
| M30 | 1.6 | 2 | Visual regression and fidelity gate | no |

**Totals:** all 32 mega-tickets about 139 h of agent time run back to back. The dogfood path (21 loop groups, M00 excluded) is about 100 h sequential, or about 49 h along its critical path if independent groups run in parallel in separate worktrees. At 6 to 8 productive agent hours per day that is roughly 2 to 3 weeks sequential for the dogfood cut and about 1 week with 2 to 3 parallel lanes, versus months at the current pace.

**Usage limits:** the binding limit is probably tokens, not hours. A 5 to 8 hour mega-ticket is likely several context windows. Rough plan: run 1 to 2 mega-tickets per usage window, largest first when limits are fresh (M07, M18, M10, M08, M13, M15, M20). Split a mega-ticket at its member boundaries if it stalls on limits; the per-member commits make that safe. Skip M25 and M26 (about 10 h) if headless approval is enough.
