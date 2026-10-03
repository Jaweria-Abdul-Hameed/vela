# Scheduler Dry-Run (Prompt 8)

**Status:** PASSED. Date 2026-10-03. Planning artifact only: no source code, no GitHub issues, no H05 provisioning, no Prompt 9 work.

**Simulated graph:** the Prompt 7 checkpoint `0f976d1bb67d50c094e729538643f7f7fb847958` (audited graph: 135 tickets, 347 authored edges, 274 published edges), then the **corrected graph** this phase produced (135 tickets, **356** authored edges, **277** published edges, critical path **18** tickets, initial frontier 14). The complete simulation was run on both; sections 6 to 13 describe the **corrected graph**, and section 3 states what changed and why.

**Verdict:** the corrected graph executes from the initial frontier to Z06 in **25 rounds** with no deadlock, no ticket scheduled before a prerequisite, no exclusive resource key shared inside a round, and every one of the 135 tickets scheduled exactly once. It is ready for implementation (Prompt 9 is the F01 bootstrap).

Companion documents: `ISSUE_GRAPH.md` (counts, DAG table, waves), `ISSUE_GRAPH.json`, `SHARED_SURFACE_PROTOCOL.md` (normative: resource keys section 8, migration blocks section 3), `AUDIT_PROMPT7.md`, and `tickets/<KEY>.md`. This file is a record, not a specification; the protocol and tickets stay authoritative.

## 1. Provenance and reconstruction

| Check | Result |
|---|---|
| Prompt 7 checkpoint | `git rev-parse HEAD` and `origin/main` were both `0f976d1bb67d50c094e729538643f7f7fb847958`; working tree clean at the start of Prompt 8 |
| Stitch reference | `docs/ui/reference/vela-stitch-reference.png`, SHA-256 `ddfc4309bd9839db194d85e349c134c4be3610d63fba5265788ba0f721027295` (matches) |
| Stale wording | `CURRENT_STATE.md` still said "awaiting checkpoint commit/push"; Git proved the checkpoint is committed and pushed, so Prompt 8 corrected that wording |
| Graph rebuilt from the 135 ticket files (header, section 7) | identical to `ISSUE_GRAPH.json` and to the `ISSUE_GRAPH.md` DAG table before any change: 135 keys, 274 published edges, depths, frontier, 17-ticket critical path; no unknown, self, or duplicate blockers; no cycle; no redundant published edge; the 347 authored edges have the same transitive closure as the published edges |
| Recount | the longest-chain tie count printed in `ISSUE_GRAPH.md` ("14") recounts as **16** on the audited graph (a documentation miscount, corrected) |
| Requirement coverage | FR-001..FR-050 and AT-001..AT-026 each appear in section 3 of at least one ticket (rechecked after the corrections) |
| Release reachability | every ticket other than Z06 is an ancestor of Z06 (no orphan that release validation could skip) |

## 2. Simulation rules

The unit of simulation is a **round** (not time). One round is: the scheduler computes the ready frontier and chooses a conservatively safe parallel set; each chosen ticket runs in its own worktree and branch from the integration tip recorded at the start of the round, in a **fresh context** (`/implement`, focused validation, ticket gates, checkpoint commit, `/code-review` against the recorded fixed point, fix and re-review loop, worker branch push); workers **never merge themselves**; the **serialized merge lane** then merges the round's tickets one at a time (update the worker branch by merging the current tip, never a rebase or force-push of a pushed branch; non-fast-forward merge; integration validation; discard on failure and return to fixing); only after the **last** merge validates and integration health is `HEALTHY` do the round's tickets become visible to dependents, resource keys release, and the next frontier compute. Integration health `UNKNOWN` or `UNHEALTHY` stops all new starts and all lane entry (`ORCHESTRATION_ENGINE.md` section 1.2).

A ticket is chosen only if every rule below holds against everything already chosen in the round. Priority: the protocol's recommended real-account spike order (SP08, SP09, SP04, SP06, SP05, SP07, SP10, SP11) first, then longest remaining path, then key.

| Rule | Meaning |
|---|---|
| R1 dependency | every published blocker is complete and integrated (human gates excepted, rule R9) |
| R2 write surface | no overlap between declared write surfaces (path prefix) except the protocol-managed surface `crates/vela-persistence/migrations/**`, where each ticket owns a distinct reserved block (`SHARED_SURFACE_PROTOCOL.md` section 3) |
| R3 contract | no consumer runs before its provider (checked structurally; section 3 lists the audit and the defects it found) |
| R4 integration | merges are serialized; the frontier advances only after healthy validation; section 8 lists the shared protocol surfaces each round's merges touch |
| R5 exclusive keys | at most one holder per round of each of `RK-AGY-REAL-ACCOUNT`, `RK-USER-ATTENTION`, `RK-FOREGROUND-WINDOW`, `RK-CLEAN-MACHINE`, `RK-QUIET-MACHINE`. Holders are the union of protocol section 8 and the ticket texts. A holder holds its keys for the **whole round** (the strict reading; section 12 relaxes it) |
| R6 derived rule D1 | **Conservative dry-run assumption, not normative.** Nothing in the protocol, an ADR, or a ticket requires it (protocol section 8 defines the two keys independently); it is not a resource-key semantic and no dependency or ticket depends on it. A `RK-FOREGROUND-WINDOW` holder never shares a round with a `RK-USER-ATTENTION` holder, on the reasoning that a human working on the same interactive desktop could be disturbed by focus movement. Kept only as stricter evidence because uncertain parallelism is serialized; removing it changes the round count only (25 to 24: no change to correctness, deadlock freedom, dependency order, write-surface safety, or contract safety) |
| R7 host headroom | Z07 (the VMware guest, a quiet-machine workload on the 16 GB host) shares a round with no real-agent-work holder (`RK-AGY-REAL-ACCOUNT`) and no `RK-QUIET-MACHINE` holder, per the protocol's prose for `RK-CLEAN-MACHINE` |
| R8 quiet machine | PF1 and U17 hold `RK-QUIET-MACHINE`, never in the same round. A measurement run executes in a quiet window after the round's other workers have stopped and before the merge lane starts (no builds, no other runs); running them alone instead costs two more rounds (section 12) |
| R9 human gates | H01, H04, H05, H06 are simulated gates, not worker tickets. A gate is **armed** when every other blocker of the gated ticket is complete (the scheduler marks the gated ticket human-gated, never failed) and is **SIMULATED SATISFIED** in a human window immediately before the round in which the gated ticket is first chosen. Nothing is fabricated: the simulation records only *when* a human action is required |
| R10 capacity | unbounded for the primary run (maximum safe set); capped runs are in section 12 |

Round table conventions: the worker list is also the lane merge order. Human windows happen **between** rounds, so they never overlap worker execution.

## 3. Contract-consumption audit and graph corrections

Method: (a) every cross-reference to a ticket key in a ticket's scope, constraints, shared contracts, acceptance criteria, and tests was checked against that ticket's ancestors; (b) every keyword that names another ticket's module (operations, broker, intervention, capacity, scheduler, registries, snapshot, provisioning) was checked the same way; (c) every "wires", "registers with", "adds entries to", and "declares" phrase was checked for an owner that exists before the consumer; (d) every pair chosen in the same round was scanned for mentions of each other; (e) the simulation was run on the audited graph first. The audited graph was schedulable (25 rounds, no deadlock) but permitted these violations:

| ID | Scenario | Tickets | Violated rule | Smallest correction |
|---|---|---|---|---|
| C1 | K01 registers its approval reconciliation recipe with the R01 operation framework; nothing ordered R01 first, so a failed or stalled R01 would not stop K01 | K01, R01 | `PARALLELIZATION.md` contract-safe; protocol section 10 | edge **R01 to K01** |
| C2 | R02 restores persisted interventions and raises divergence interventions through the S15 model; R02 was not a descendant of S15 | R02, S15 | contract-safe; protocol section 3 table ownership (`human_interventions` is S15's) | edge **S15 to R02**; the published edge S15 to S16 became implied through R02 |
| C3 | W01 wires "ProcessRunner with tree kill" and "the GitAdapter capability implementations" but depended on none of P02, G03, G04, and W01 alone owns `wiring/adapters.rs`; if W01 merged first, no later ticket could wire them | W01, P02, G03, G04 | contract-safe; protocol section 5 ownership | edges **P02, G04 (implies G03) to W01** |
| C4 | W01's path function resolves the installed `vela-hook` path that Z01's bundle defines; unordered | W01, Z01 | contract-safe | edge **Z01 to W01**; Z01's text records the stable path in `DEV_SIGNING.md` |
| C5 | R03 owns the crash matrix and its completeness test, but its text said S13, S14, and R08 "add entries for their own points as they land": those tickets had no write surface, scope, or criterion for it, the matrix does not exist before R03, and in the audited schedule **R03, S13, S14, and R08 ran in the same round** (an actual unsafe parallel set with no valid merge order for the entries) | R03, S13, S14, R08 | protocol section 6 (fault points); write-surface ownership | edges **S13, S14, R08 to R03**. R03 now owns every matrix entry; S13, S14, and R08 declare and unit-test their points (one scope line each); the S12 to R03 edge and the S13, S14, R08 edges into Z03 became implied. This is the only correction that lengthens the critical path (17 to 18 tickets); it added no round |
| C6 | F09's scope said the scripted agent "can replay recorded transcripts through the parser added by A02"; F09 and A02 are unordered siblings and ran in the same round | F09, A02 | contract-safe | text only: transcript replay belongs to TK1 (already after A02) |

Net effect: authored edges 347 to **356** (+9: R01 to K01; S15 to R02; P02, G03, G04, Z01 to W01; S13, S14, R08 to R03), published edges 274 to **277** (+K01, +R02, -S16, +3 W01, +2 net R03, -3 Z03). Depths changed only for R03 (13 to 14), Z03 (14 to 15), Z05 (15 to 16), Z06 (16 to 17). Frontier (14), requirement coverage, and write surfaces are unchanged; unresolved write-surface conflicts remain 0 (57 unordered overlapping pairs, all of them the migrations directory with distinct reserved blocks; the Prompt 7 audit counted 58, and R01 to K01 now orders one of them). The **entire** simulation was rerun on the corrected graph; the audited graph also took 25 rounds, so the corrections cost no round.

### Watch items (no graph change; for the implementing agents and the lead)

| ID | Observation | Disposition |
|---|---|---|
| W1 | O01 (round 6) owns `workers` in block 0070; S06 (round 7) owns `tickets` in block 0060 and builds after O01. Numerically the foreign key is legal, but the parent table's ticket merges later. | O01 must not require a populated `tickets` table in its own tests; if a hard foreign key is wanted, record a decision request rather than rebuilding another ticket's table. Not a graph defect: S09 and S10, which write worker rows, follow S06. |
| W2 | Resource-key lease release when a holder parks (`NEEDS_HUMAN`, `PAUSED`, `FAILED`, `CANCELLED`) is not specified for the runtime scheduler (D03, S09, PV1). | Recommended rule for those tickets: release on any non-active state and re-acquire on resume; holding a key through a human wait would starve the lane. |
| W3 | Protocol section 8 lists holders (for example A05, A06, A07, A10, A11, A16, Z04) that their tickets do not repeat. | The protocol is the single declaration; the scheduler (not the worker) enforces it. No ticket edit. |
| W4 | SP12's real-Desktop part, SP13, and K08 are evidence-gathering and valid before H06. If H06 decides "no Desktop-hosted sessions", SP12 reduces to the fake window, SP13 closes, K08 closes as dropped, and K07 reduces to the headless pause semantics (as its ticket already says). The graph is valid for either outcome. | Keep H06 open; never run real-Desktop approval work as if H06 were "yes". |
| W5 | PF1 (round 12) measures before later UI surfaces land (U09 round 14, A12 and K10 round 17). Z06 lists "security and performance evidence", but no existing requirement mandates a PF1 rerun on the release candidate, and the protocol does not list Z06 as an `RK-QUIET-MACHINE` holder. | **Watch item only.** No Z06 dependency or release requirement was added; the lead may decide at issue publication. |
| W6 | U07 and U08 are built against fixture streams; the real `timeline.get` and `evidence.get` commands (S10 round 11, S11 round 12) arrive later. | Acceptable by design (closed event vocabulary, F02); the W01 smoke and U18 should include one real-command panel check. |
| W7 | The first real-account gated runs (A03 round 9, A05 10, A06 11, A07 13) precede S16 (round 13) and A13 (round 14), which hold the provider-policy-block state and the Antigravity signal mapping. | Accepted: those runs are attended single-turn runs, F02 already defines the typed error and event, A13 asserts that no code path retries, and a block must never be provoked to obtain evidence. |
| W8 | The first fake-agent worker slice is S10 (round 11), the first real window is W01 (round 14), and the first real-agent end to end is A16 (round 19). | Accepted: the unknowns are front-loaded in spikes (rounds 1 to 8) and TK1 stands in for the real agent; A16 cannot move earlier (A10 round 14, S12 round 13, W01, A07, and the real-account lane). |
| W9 | H01 needs an answer from Google whose latency Vela does not control. | The simulation needs it only at round 25, but asking early costs nothing and development is not blocked either way. |
| W10 | `RK-CLEAN-MACHINE` carries a prose-only exclusion (never with PF1, heavy agent work, or large WSL workloads). | Modeled as rule R7; no protocol edit. |

## 4. Final graph metrics

| Measure | Audited graph (Prompt 7) | Corrected graph (this phase) |
|---|---|---|
| Tickets | 135 | 135 |
| Authored edges | 347 | 356 |
| Published (reduced) edges | 274 | 277 |
| Cycles / unknown / self / duplicate / redundant | none | none |
| Initial frontier | 14 (F01, H01, SP01, SP02, SP03, SP04, SP06, SP07, SP08, SP09, SP10, SP11, H04, H05) | 14 (same) |
| Critical path | 17: F01 F02 F04 F05 F08 S01 S03 S04 S08 S09 S10 S11 S12 S14 Z03 Z05 Z06 | 18: F01 F02 F04 F05 F08 S01 S03 S04 S08 S09 S10 S11 S12 S14 R03 Z03 Z05 Z06 (8 distinct longest chains tie) |
| Rounds to complete (primary run) | 25 | 25 |
| Unresolved write-surface conflicts | 0 | 0 |

**Is the critical path real?** Every edge was checked. F01 to F02 to F04 to F05 to F08 is ownership of one stack (ports, shell, renderer skeleton, IPC gateway and its generated contracts; F08 edits the renderer `ipc/` directory that F05 owns). F08 to S01 to S03 to S04 to S08 is the registry, project, trust, preflight, and Build-start chain (S03 needs a project and settings, S04 needs trust, S08 needs a passed preflight). S08 to S09 to S10 to S11 to S12 is the Orchestrator chain that the protocol serializes on shared message types and one state machine. S12 to S14 to R03 to Z03 to Z05 to Z06 is evidence (merge-lane features, the recovery matrix over their fault points, the end-to-end suite, traceability, release). None is an accidental chain. The one non-structural contributor to schedule length is the single human (section 6).

**Bottlenecks (descendant counts):** F01 119, F02 114, F03 80, F04 79, F07 79, F05 76, F06 75, F08 72, F09 59, D01 57, U01 57, P01 56, S02 56, O01 55. These are foundations that the doctrine wants early, and they all land in rounds 1 to 6. The largest single orchestration ticket, S12, carries a pre-authorized split inside its own text. Z05 has 15 blockers (an intentional traceability umbrella).

## 5. Human gates (simulated, never fabricated)

| Gate | Real status (unchanged by this phase) | DAG-ready | Armed (gated ticket otherwise ready) | SIMULATED SATISFIED (window before round) | What it blocks, and why this is not a deadlock |
|---|---|---|---|---|---|
| H01 | OPEN, PARTIALLY-DOCUMENTED, authoritative Google answer still required, release gate only, no Gemini API-key fallback, provider-policy blocks stay non-retryable | round 1 (no blockers) | round 25 (Z06) | 25 | **Only Z06.** The other 133 tickets do not depend on it, so development, integration, and every spike proceed under documented uncertainty. |
| H04 | OPEN, production signing deferred | round 1 | round 25 (Z06) | 25 | **Only Z06.** Z01 and Z02 use development and test signing material and finish in rounds 5 and 14. |
| H05 | OPEN, route decided (VMware Workstation Pro, Windows 11 Enterprise 90-day Evaluation), **not provisioned** | round 1 | round 19 (Z07; the last other blocker, L03, finishes in round 18) | 23 | **Only Z07.** Z07 waits in the gated state in rounds 19 to 22 while Z04, SP12, SP13, U19, and K08 run, because the single human and the host are busy, not because of H05. Real provisioning should start no earlier than round 19 and finish before round 23 (the evaluation clock is 90 days). The VM is validation infrastructure only; nothing except Z07 and Z06 touches it. |
| H06 | OPEN/deferred, blocker SP09 (done in round 2), separate from ADR-020 | round 3 | round 17 (K07; K06, the last other blocker, finishes in round 16) | 17 | **Only K07** (and the real-Desktop approval-delivery scope of Z04, which already depends on K07). K05 (round 12) and K06 (round 16) run against the fake window K04 (round 2) and do not assume H06. Both outcomes keep the graph valid (W4). H06 never weakens ADR-020, FR-050, CAP-12, or AT-026: SP08 is round 1, A05 round 10, A12 round 17, Z04 round 22, and none depends on H06. |

H02 and H03 are resolved and are not in the graph.

## 6. Resource-key serialization

| Resource key | Holders (protocol section 8 plus ticket text) | Rounds used, in order |
|---|---|---|
| RK-AGY-REAL-ACCOUNT | 21 | SP08@1, SP09@2, SP04@3, SP06@4, SP05@5, SP07@6, SP10@7, SP11@8, A03@9, A05@10, A06@11, K05@12, A07@13, A10@14, A11@15, K06@16, A12@17, A15@18, A16@19, SP12@20, Z04@22 |
| RK-USER-ATTENTION | 22 | SP08@1, SP09@2, SP04@3, SP06@4, SP05@5, SP07@6, SP10@7, SP11@8, P03@10, S17@11, K05@12, L02@15, K06@16, A12@17, L03@18, R06@19, SP12@20, U19@21, Z04@22, Z07@23, L04@24, Z06@25 |
| RK-FOREGROUND-WINDOW | 8 | SP02@9, K05@12, SP01@13, W01@14, K06@16, SP12@20, Z07@23, Z06@25 |
| RK-CLEAN-MACHINE | 2 | Z07@23, Z06@25 |
| RK-QUIET-MACHINE | 2 | U17@11, PF1@12 |

Lower bounds: the DAG alone needs 18 rounds (critical path); the real-account key has 21 holders, so at least 21 rounds; the human-attention key has 22 holders, so at least 22 rounds. The primary schedule takes 25: the real-account key is idle in rounds 21, 23, 24, 25 and the attention key in rounds 9, 13, 14. **The single human and the single real account, not graph depth, are the real scheduling bottleneck.** The graph has no ordering problem here; the protocol interpretation sets the length: if a key means "held for the real session" rather than "for the whole ticket", the same graph finishes in 22 rounds (section 12). Recommendation recorded and not applied: keep the strict reading when scheduling the project, and batch each ticket's manual evidence into one attended session.

Resource-key findings: SP08 runs first (round 1) as the protocol requires; SP11, which provokes quota and self-update behavior, runs last of the spikes (round 8); SP05 follows SP04; SP01 and SP02 (foreground window) never share a round with each other or with a human-attention holder; PF1 and U17 never share a round; Z07 and Z06 hold the clean-machine, foreground, and attention keys alone; Z07 never shares a round with real-agent work.

## 7. Round-by-round schedule

| Round | Human window (before the round) | Workers started together (also the serialized merge-lane order) | Count |
|---|---|---|---|
| 1 | - | SP08 F01 SP03 | 3 |
| 2 | - | SP09 F02 K04 X05 | 4 |
| 3 | - | SP04 F04 F06 F07 F03 D01 D02 D05 T01 D06 | 10 |
| 4 | - | SP06 F05 P01 F09 D03 D04 A02 A04 D07 | 9 |
| 5 | - | SP05 F08 G01 U01 T02 A01 P02 TK1 X02 Z01 | 10 |
| 6 | - | SP07 S01 S02 G05 O01 G02 U02 G03 T03 X03 | 10 |
| 7 | - | SP10 S03 S06 R01 G04 S05 S15 T04 U03 R05 U04 U15 | 12 |
| 8 | - | SP11 S04 PV1 U05 A08 K01 U10 L01 S07 | 9 |
| 9 | - | S08 A03 SP02 U06 U07 K02 U12 K09 | 8 |
| 10 | - | S09 A05 A09 P03 U14 U08 U11 U16 U13 | 9 |
| 11 | - | S10 A06 S17 U17 X01 | 5 |
| 12 | - | S11 K05 R02 U18 PF1 | 5 |
| 13 | - | R04 S12 A07 SP01 R07 S16 A14 | 7 |
| 14 | - | A10 R08 S13 S14 W01 Z02 A13 U09 | 8 |
| 15 | - | A11 L02 R03 | 3 |
| 16 | - | K06 Z03 | 2 |
| 17 | H06 SIMULATED SATISFIED | A12 K07 K10 X04 | 4 |
| 18 | - | A15 L03 | 2 |
| 19 | - | A16 R06 | 2 |
| 20 | - | SP12 | 1 |
| 21 | - | SP13 U19 | 2 |
| 22 | - | Z04 K08 | 2 |
| 23 | H05 SIMULATED SATISFIED | Z07 | 1 |
| 24 | - | L04 Z05 | 2 |
| 25 | H01 SIMULATED SATISFIED, H04 SIMULATED SATISFIED | Z06 | 1 |

Every round's workers start from the integration tip left by the previous round's last validated merge. Round 1 has no CI to validate against (F01 creates it): its integration validation is F01's own workspace checks plus documentation checks for the spikes; X05 (secret scanning and locked builds) merges in round 2, before any substantial code.

### 7.1 Ready but deferred (meaningful rejected combinations)

No pair was ever rejected for a write-surface overlap or a contract conflict in the corrected graph: the edges and the protocol partitions already order those (section 3 is where such pairs were found and fixed). Every deferral below is a resource key, the derived rule D1, or host headroom:

| Round | Ready but deferred, and why |
|---|---|
| 1 | RK-AGY-REAL-ACCOUNT,USER-ATTENTION held by SP08: SP09, SP04, SP06, SP07, SP10, SP11; derived rule D1 (foreground window vs human attention) with SP08: SP02, SP01 |
| 2 | RK-AGY-REAL-ACCOUNT,USER-ATTENTION held by SP09: SP04, SP06, SP07, SP10, SP11; derived rule D1 (foreground window vs human attention) with SP09: SP02, SP01 |
| 3 | RK-AGY-REAL-ACCOUNT,USER-ATTENTION held by SP04: SP06, SP07, SP10, SP11; derived rule D1 (foreground window vs human attention) with SP04: SP02, SP01; RK-USER-ATTENTION held by SP04: P03 |
| 4 | RK-AGY-REAL-ACCOUNT,USER-ATTENTION held by SP06: SP05, SP07, SP10, SP11; derived rule D1 (foreground window vs human attention) with SP06: SP02, SP01; RK-USER-ATTENTION held by SP06: P03, L04 |
| 5 | RK-AGY-REAL-ACCOUNT,USER-ATTENTION held by SP05: SP07, SP10, SP11; derived rule D1 (foreground window vs human attention) with SP05: SP02, SP01; RK-USER-ATTENTION held by SP05: P03, L04 |
| 6 | RK-AGY-REAL-ACCOUNT,USER-ATTENTION held by SP07: SP10, SP11; RK-AGY-REAL-ACCOUNT held by SP07: A03, A05; derived rule D1 (foreground window vs human attention) with SP07: SP02, SP01; RK-USER-ATTENTION held by SP07: P03, L04 |
| 7 | RK-AGY-REAL-ACCOUNT,USER-ATTENTION held by SP10: SP11; RK-AGY-REAL-ACCOUNT held by SP10: A03, A05; derived rule D1 (foreground window vs human attention) with SP10: SP02, SP01; RK-USER-ATTENTION held by SP10: P03, S17, L03, L04 |
| 8 | RK-AGY-REAL-ACCOUNT held by SP11: A03, A05; derived rule D1 (foreground window vs human attention) with SP11: SP02, SP01; RK-USER-ATTENTION held by SP11: P03, S17, L03, L04 |
| 9 | RK-AGY-REAL-ACCOUNT held by A03: A05; derived rule D1 (foreground window vs human attention) with SP02: P03, S17, L03, L04; RK-FOREGROUND-WINDOW held by SP02: SP01 |
| 10 | RK-AGY-REAL-ACCOUNT held by A05: K05, SP12; RK-USER-ATTENTION held by P03: S17, L03, L04; derived rule D1 (foreground window vs human attention) with P03: SP01 |
| 11 | RK-AGY-REAL-ACCOUNT held by A06: K05, A10, A11, SP12; derived rule D1 (foreground window vs human attention) with S17: SP01; RK-USER-ATTENTION held by S17: L03, L04 |
| 12 | RK-AGY-REAL-ACCOUNT held by K05: A07, A10, A11, A15; RK-FOREGROUND-WINDOW held by K05: SP01, W01; RK-USER-ATTENTION held by K05: L03, L04; RK-AGY-REAL-ACCOUNT,FOREGROUND-WINDOW,USER-ATTENTION held by K05: SP12 |
| 13 | RK-AGY-REAL-ACCOUNT held by A07: A10, A11, K06, A15, SP12; RK-FOREGROUND-WINDOW held by SP01: W01; derived rule D1 (foreground window vs human attention) with SP01: L03, R06, U19, L04 |
| 14 | RK-AGY-REAL-ACCOUNT held by A10: A11, K06, A15, SP12; derived rule D1 (foreground window vs human attention) with W01: L02, L03, R06, U19, L04 |
| 15 | RK-AGY-REAL-ACCOUNT held by A11: K06, A15, A16, SP12; RK-USER-ATTENTION held by L02: L03, R06, U19, L04 |
| 16 | RK-AGY-REAL-ACCOUNT,USER-ATTENTION held by K06: A12; RK-AGY-REAL-ACCOUNT held by K06: A15, A16; RK-USER-ATTENTION held by K06: L03, R06, U19, L04; RK-AGY-REAL-ACCOUNT,FOREGROUND-WINDOW,USER-ATTENTION held by K06: SP12 |
| 17 | RK-AGY-REAL-ACCOUNT held by A12: A15, A16; RK-USER-ATTENTION held by A12: L03, R06, U19, L04; RK-AGY-REAL-ACCOUNT,USER-ATTENTION held by A12: SP12 |
| 18 | RK-AGY-REAL-ACCOUNT held by A15: A16, SP12; RK-USER-ATTENTION held by L03: R06, U19, L04 |
| 19 | RK-AGY-REAL-ACCOUNT held by A16: SP12; RK-USER-ATTENTION held by R06: U19, Z07, L04 |
| 20 | RK-USER-ATTENTION held by SP12: U19, L04; RK-AGY-REAL-ACCOUNT,USER-ATTENTION held by SP12: Z04; RK-FOREGROUND-WINDOW,USER-ATTENTION held by SP12: Z07 |
| 21 | RK-USER-ATTENTION held by U19: Z04, Z07, L04 |
| 22 | RK-USER-ATTENTION held by Z04: Z07, L04 |
| 23 | RK-USER-ATTENTION held by Z07: L04 |

### 7.2 Pairs that look independent but are serialized by an edge (kept)

The Orchestrator chain S08, S09, S10, S11, S12, then S13, S14, R08 (shared message types, transition tables, one state machine); vela-git (G01 before G02, G03, G05; G04 after G02 and G03); the broker (K01 before K02, K09, A07; K06 after K05; K07 after K06, K02, H06); the canvas (U01, U02, U05 before U06, U07, U12, U11; U17 after U03, U07, U14); the preflight panel (S04, then U11 with S08, then K10); settings UI (U15 before L01); release (Z01 before Z02; X05 and H04 before Z06); and the new C1 to C5 edges.

### 7.3 Pairs that share a zone and are deliberately allowed together

Parallel domain tickets in `vela-domain` (D01, D02, D05, D06 in round 3; D03, D04, D07 in round 4), `vela-core` services (rounds 6 to 8), `vela-persistence` migrations (rounds 6, 7, 8, 11), and generated contracts and permissions (rounds 6 to 8). Safe because F01 pre-creates every module, each ticket owns its module, each owns a reserved migration block and its `repositories/<ticket>_*.rs` discovered by `build.rs`, each command group owns its own file, and generated files are regenerated by the lane, never hand-merged (protocol sections 1 to 4).

## 8. Integration lane

Merge order inside a round is the listed worker order. The outcome is order-insensitive in every round because the surfaces the workers share are protocol-managed (partitioned or regenerated). The lane re-runs the migration ladder from an empty database after every merge in rounds 6, 7, 8, and 11 (a duplicate number is a test failure) and regenerates the lockfile and generated contract files from the merged manifests whenever more than one ticket in a round touched them.

| Round | Merge-lane notes (protocol-managed surfaces touched by more than one ticket in the round) |
|---|---|
| 1 | none |
| 2 | none |
| 3 | same crate or package, disjoint modules: crates/vela-domain (D01 D02 D05 D06) |
| 4 | same crate or package, disjoint modules: crates/vela-domain (D03 D04 D07) |
| 5 | tauri config, plugin list, capabilities: X02, Z01; same crate or package, disjoint modules: apps/desktop/src (F08 U01); apps/desktop/src-tauri (F08 X02 Z01); crates/vela-adapters (A01 T02); crates/vela-testkit (T02 TK1); packages/ui (U01 X02) |
| 6 | migrations S01 0010-0019, S02 0020-0029, O01 0070-0079, T03 0170-0179; generated contracts and permissions: S01, S02; same crate or package, disjoint modules: apps/desktop/src-tauri (O01 S01 S02); crates/vela-core (O01 S01 S02 T03 X03); crates/vela-git (G02 G03 G05); crates/vela-persistence (O01 S01 S02 T03); packages/contracts (S01 S02) |
| 7 | migrations S03 0030-0039, S06 0060-0069, R01 0130-0139, S15 0110-0119; generated contracts and permissions: S03, S06; same crate or package, disjoint modules: apps/desktop/src (S03 S06 U04 U15); apps/desktop/src-tauri (S03 S06); crates/vela-core (R01 R05 S03 S05 S06 S15 T04); crates/vela-persistence (R01 R05 S03 S06 S15); packages/contracts (S03 S06) |
| 8 | migrations S04 0040-0049, K01 0140-0149; generated contracts and permissions: S04, K01; same crate or package, disjoint modules: apps/desktop/src (L01 S04 U05 U10); apps/desktop/src-tauri (K01 S04); crates/vela-core (K01 PV1 S04 S07); crates/vela-persistence (K01 S04); packages/contracts (K01 S04) |
| 9 | same crate or package, disjoint modules: apps/desktop/src (K09 S08 U06 U07 U12); crates/vela-core (K02 K09 S08); packages/ui (U06 U07) |
| 10 | same crate or package, disjoint modules: apps/desktop/src (U08 U11 U16); crates/vela-adapters (A05 A09); packages/ui (U13 U14) |
| 11 | migrations S10 0090-0099, S17 0160-0169; same crate or package, disjoint modules: apps/desktop/src-tauri (S10 S17); crates/vela-adapters (A06 S17); crates/vela-core (S10 S17); crates/vela-persistence (S10 S17) |
| 12 | same crate or package, disjoint modules: crates/vela-core (R02 S11); tests/e2e (PF1 U18) |
| 13 | same crate or package, disjoint modules: crates/vela-adapters (A07 A14); crates/vela-core (R04 R07 S12 S16) |
| 14 | same crate or package, disjoint modules: apps/desktop/src-tauri (W01 Z02); crates/vela-adapters (A10 A13); crates/vela-core (R08 S13 S14) |
| 15 | none |
| 16 | none |
| 17 | same crate or package, disjoint modules: apps/desktop/src (A12 K10); crates/vela-core (A12 K07 K10) |
| 18 | none |
| 19 | none |
| 20 | none |
| 21 | none |
| 22 | none |
| 23 | none |
| 24 | none |
| 25 | none |

Wide rounds cost the later lane entries more update-merges (round 7 has 12 workers, so the last merge updates its branch against 11 interim tips). That is bounded because the surfaces are disjoint; if the conflict-sensitive gate fires, the worker goes to `CONFLICT_RESOLUTION` (ADR-011) and the lane is released while it works. Integration-ordering hazards found: F09 with A02 in round 4 and R03 with S13, S14, R08 in round 14 of the audited schedule (section 3, both removed). No round in the corrected schedule contains a pair that needs a specific merge order.

## 9. Ticket accounting (all 135 exactly once)

Rounds are positive integers; `W<n>` marks a SIMULATED HUMAN GATE satisfied in the window before round n (not a worker round). "Latest blocker" is the blocker that finished last.

| Key | Round | Latest blocker (round) | Resource keys | Title |
|---|---|---|---|---|
| F01 | 1 | none | - | Workspace bootstrap: Cargo and npm workspaces, toolchain pins, … |
| SP03 | 1 | none | - | Spike S-PROC-JOB: does a Windows Job Object with kill-on-close … |
| SP08 | 1 | none | AGY,ATTN | Spike S-DESKTOP-VISIBILITY: are headless CLI conversations visi… |
| F02 | 2 | F01 (1) | - | Domain primitives, ports, and TypeScript contract generation |
| K04 | 2 | F01 (1) | - | Fake approval window harness reproducing the verified card roles |
| SP09 | 2 | none | AGY,ATTN | Spike S-INTERACTIVE-TRUST: how do the CLI and Desktop treat a n… |
| X05 | 2 | F01 (1) | - | CI supply-chain gates: secret scanning, dependency audit, licen… |
| D01 | 3 | F02 (2) | - | State machines: run, worker, and ticket projection with exhaust… |
| D02 | 3 | F02 (2) | - | Dependency graph engine: DAG, cycle detection, frontier, topolo… |
| D05 | 3 | F02 (2) | - | Policy engine core: operation model, PowerShell and cmd normali… |
| D06 | 3 | F02 (2) | - | Review domain: finding identity, exit policy, loop state, and o… |
| F03 | 3 | F02 (2) | - | Test kit foundation: fixture repositories, fault-injection harn… |
| F04 | 3 | F02 (2) | - | Tauri shell walking skeleton with strict CSP and a typed comman… |
| F06 | 3 | F02 (2) | - | Persistence foundation: single-writer SQLite, migration ladder,… |
| F07 | 3 | F02 (2) | - | Structured logging, correlation ids, and the central redaction … |
| SP04 | 3 | none | AGY,ATTN | Spike S-NATIVE-POSTURE: do generated allow rules plus a fail-cl… |
| T01 | 3 | F02 (2) | - | Local Markdown tracker: ticket parsing, blockers, and malformed… |
| A02 | 4 | F03 (3) | - | stream-json parser and result interpreter against recorded tran… |
| A04 | 4 | D05 (3) | - | vela-hook binary, rule-table format, and spool records (fail-cl… |
| D03 | 4 | D02 (3) | - | Parallel-safety engine: predicted write sets, hard blockers, ev… |
| D04 | 4 | D02 (3) | - | Deterministic layered graph layout stored with the snapshot |
| D07 | 4 | D05 (3) | - | Approval domain: requests, decisions, fingerprints, progress de… |
| F05 | 4 | F04 (3) | - | Renderer skeleton: React, store, IPC wrappers, and front-end te… |
| F09 | 4 | F03 (3) | - | Port fakes and the shared conformance suite |
| P01 | 4 | F07 (3) | - | ProcessRunner: spawn, allowlisted environment, timeouts, stream… |
| SP06 | 4 | none | AGY,ATTN | Spike S-HOOK-GLOBAL: are global-path hooks in an isolated profi… |
| A01 | 5 | P01 (4) | - | Antigravity capability discovery: installation, versions, flags… |
| F08 | 5 | F05 (4) | - | IPC gateway: command registry, snapshot plus sequenced event st… |
| G01 | 5 | P01 (4) | - | GitAdapter: hardened invocation, read operations, and hostile-c… |
| P02 | 5 | P01 (4) | - | Process tree kill and cancellation |
| SP05 | 5 | SP04 (3) | AGY,ATTN | Spike S-ASK-RESUME: after a hook-blocked ASK and a human allow,… |
| T02 | 5 | F09 (4) | - | GitHub tracker via gh: read issues, native blocking links, labe… |
| TK1 | 5 | A02 (4) | - | Scripted fake agy executable replaying recorded Antigravity beh… |
| U01 | 5 | F05 (4) | - | Design system: tokens, typography, restrained glass, icons, and… |
| X02 | 5 | F05 (4) | - | Renderer hardening verification: CSP, inert rendering, capabili… |
| Z01 | 5 | A04 (4) | - | Installer: NSIS per-user package with development signing, WebV… |
| G02 | 6 | G01 (5) | - | Worktree manager: root policy, create, remove, lock, repair, sl… |
| G03 | 6 | G01 (5) | - | Checkpoint commits, per-worktree operation lock, and hook-aware… |
| G05 | 6 | G01 (5) | - | Remote operations: fetch, non-force push, remote ref verificati… |
| O01 | 6 | F08 (5) | - | Orchestrator actor skeleton: message bus, transactional transit… |
| S01 | 6 | F08 (5) | - | Project import: choose a repository, persist it, and list recen… |
| S02 | 6 | F08 (5) | - | Settings layers, onboarding choices, and the immutable run snap… |
| SP07 | 6 | none | AGY,ATTN | Spike S-SCHEMA-OUTPUT: does --json-schema enforce the reviewer … |
| T03 | 6 | T02 (5) | - | Remote update queue with idempotent retries and outage behavior |
| U02 | 6 | U01 (5) | - | Canvas foundation: reactive dot lattice with demand rendering a… |
| X03 | 6 | F08 (5) | - | Diagnostics export with redaction verification and log retention |
| G04 | 7 | G02 (6) | - | Merge operations: integration merge into branch, no-ff merge, d… |
| R01 | 7 | O01 (6) | - | Operation records and idempotency framework |
| R05 | 7 | G02 (6) | - | State-store failure: backup, failed migration, and Git-derived … |
| S03 | 7 | S02 (6) | - | Repository trust: untrusted by default, trust sheet, data-only … |
| S05 | 7 | S01 (6) | - | Context discovery and documentation manifest |
| S06 | 7 | O01 (6) | - | Issue ingestion to an approved-ready graph snapshot with cycle … |
| S15 | 7 | O01 (6) | - | Human intervention model: records, kinds, resume state, decisio… |
| SP10 | 7 | none | AGY,ATTN | Spike S-SKILLS-ANTIGRAVITY: do the Matt Pocock implement and co… |
| T04 | 7 | G05 (6) | - | PR workflow and promotion modes: draft PR, ready, local-ready, … |
| U03 | 7 | U02 (6) | - | Ambient violet, blue, and cyan field with Full, Balanced, and E… |
| U04 | 7 | U02 (6) | - | Home scene: drop a project folder, recent project anchors, and … |
| U15 | 7 | S02 (6) | - | Command palette and settings sheet |
| A08 | 8 | SP10 (7) | - | Skills detection and bootstrap for worker worktrees |
| K01 | 8 | S15 (7) | - | Approval Broker core: classification pipeline, evidence tiers, … |
| L01 | 8 | U15 (7) | - | Onboarding experience: consent choices, background, auto-start,… |
| PV1 | 8 | S03 (7) | - | Worktree provisioning contract: confirmed commands, file allowl… |
| S04 | 8 | S03 (7) | - | Preflight engine end to end: repository, toolchain, trust, and … |
| S07 | 8 | S06 (7) | - | GitHub issue ingestion into the same graph pipeline |
| SP11 | 8 | none | AGY,ATTN | Spike S-CAP-REMAINING: WAITING status, concurrency ceiling, bac… |
| U05 | 8 | S06 (7) | - | Project universe: dependency constellation rendering, camera, a… |
| U10 | 8 | S15 (7) | - | Intervention sheet for every intervention kind |
| A03 | 9 | P02 (5) | AGY | Per-turn session driver: spawn, conversation id, explicit timeo… |
| K02 | 9 | K01 (8) | - | Approval Watchdog and loop guard: progress heartbeats, threshol… |
| K09 | 9 | K01 (8) | - | Scoped rule creation from an intervention with the scope shown … |
| S08 | 9 | S04 (8) | - | Graph approval and Build start: integration branch, integration… |
| SP02 | 9 | none | FG | Spike S-UIA-BINDINGS: does the windows crate expose the UI Auto… |
| U06 | 9 | U05 (8) | - | Node states and event-driven effects: ripples, edge flow, settl… |
| U07 | 9 | U05 (8) | - | Issue focus transition and inspector with progressive disclosure |
| U12 | 9 | U05 (8) | - | Conflict forecasting visualization |
| A05 | 10 | TK1 (5) | AGY | Per-worker profile manager and generated settings and allow rul… |
| A09 | 10 | A03 (9) | - | Worker task envelope and prompts: implement invocation, recover… |
| P03 | 10 | F02 (2) | ATTN | Keep-awake thread and interactive-session probe |
| S09 | 10 | S08 (9) | - | Scheduler tick: frontier, safety, capacity, and worker records … |
| U08 | 10 | U07 (9) | - | Timeline and evidence panels: commands, tests, review findings |
| U11 | 10 | S08 (9) | - | Build-ready view and preflight panel with parallelization reaso… |
| U13 | 10 | U07 (9) | - | Accessibility: graph tree and list, keyboard navigation, focus,… |
| U14 | 10 | U06 (9) | - | Reduced motion everywhere and the WebGL failure fallback |
| U16 | 10 | U07 (9) | - | Completion state and execution summary |
| A06 | 11 | A05 (10) | AGY | Native policy enforcement and the posture probe (MEETS, DOES_NO… |
| S10 | 11 | S09 (10) | - | Worker lifecycle with a fake agent: provision, implement, focus… |
| S17 | 11 | O01 (6) | ATTN | Desktop notifications for human-required, failure, and completi… |
| U17 | 11 | U14 (10) | QUIET | Large graphs: 500-node performance and adaptive quality suggest… |
| X01 | 11 | A09 (10) | - | Prompt-injection and untrusted-input boundaries verified end to… |
| K05 | 12 | SP02 (9) | AGY,FG,ATTN | vela-uia discovery and read: windows, warm-up, correlation, and… |
| PF1 | 12 | U17 (11) | QUIET | Performance harness and budget measurement on the reference har… |
| R02 | 12 | S10 (11) | - | Startup reconciliation and recovery-resume |
| S11 | 12 | S10 (11) | - | Checkpoint and review loop with a fake reviewer: fixed point, f… |
| U18 | 12 | U17 (11) | - | Visual regression suite and fixtures for the key states |
| A07 | 13 | A06 (11) | AGY | Headless ASK path: spool, APPROVAL_ASK, one-time allow, and res… |
| A14 | 13 | R02 (12) | - | Antigravity recovery and version drift: orphan sessions, stale … |
| R04 | 13 | S11 (12) | - | Stop All, pause, and resume for running workers |
| R07 | 13 | R02 (12) | - | Reboot continuation and the recovery_continuation setting |
| S12 | 13 | S11 (12) | - | Merge lane and integration validation: two parallel tickets mer… |
| S16 | 13 | R02 (12) | - | Execution profiles and capacity model with simulated capacity e… |
| SP01 | 13 | none | FG | Spike S-TRAY: does prevent_exit with a tray keep the Tauri proc… |
| A10 | 14 | A09 (10) | AGY | Authoritative reviewer session: fresh conversation, code-review… |
| A13 | 14 | S16 (13) | - | Capacity detection from Antigravity output and pause/cooldown m… |
| R08 | 14 | R04 (13) | - | Stop All safe points for the merge lane and push; integration h… |
| S13 | 14 | S12 (13) | - | Conflict resolution attempt and MERGE_CONFLICT escalation |
| S14 | 14 | S12 (13) | - | Finalization: final review, push by Vela, promotion, cleanup, a… |
| U09 | 14 | R04 (13) | - | Floating run bar: worker count, status, mode, pause, and an alw… |
| W01 | 14 | S10 (11) | FG | Production composition root: real adapter wiring and a fixture … |
| Z02 | 14 | R04 (13) | - | Updater: signed updates, deferral during runs, pre-update backu… |
| A11 | 15 | A09 (10) | AGY | Dependency analyst session: read-only, validated, stored in the… |
| L02 | 15 | SP01 (13) | ATTN | Tray, background operation, hide-on-close, and safe pause on cl… |
| R03 | 15 | S14 (14) | - | Crash-injection recovery matrix across every worker transition |
| K06 | 16 | K05 (12) | AGY,FG,ATTN | vela-uia delivery: single-use allow or refusal through patterns… |
| Z03 | 16 | R03 (15) | - | End-to-end fixture suite with fakes: AT-001 to AT-007, AT-010, … |
| A12 | 17 | A11 (15) | AGY,ATTN | Conversation visibility in Antigravity Desktop: read model, Ope… |
| H06 | W17 | SP09 (2) | - | Decide whether v1 creates Desktop-hosted Antigravity sessions f… |
| K07 | 17 | H06 (W17) | - | Interactive-session degradation and reconciliation for UI-autom… |
| K10 | 17 | K06 (16) | - | Antigravity preflight providers and unattended readiness: capab… |
| X04 | 17 | K06 (16) | - | Security regression suite: normalizer corpus, hostile repositor… |
| A15 | 18 | A06 (11) | AGY | Real-environment compatibility suite gated by VELA_REAL_ANTIGRA… |
| L03 | 18 | S02 (6) | ATTN | Login auto-start integration |
| A16 | 19 | W01 (14) | AGY | Antigravity-backed single-ticket run end to end (AT-001 with th… |
| R06 | 19 | R02 (12) | ATTN | Sleep, wake, network loss, and GitHub outage handling |
| SP12 | 20 | SP02 (9) | AGY,FG,ATTN | Spike S-UIA-RELIABILITY: repeat and vary UI Automation approval… |
| SP13 | 21 | SP12 (20) | - | Spike S-VISUAL: is the guarded visual tier ever needed on the p… |
| U19 | 21 | U18 (12) | ATTN | UI fidelity review gate against the Stitch reference |
| K08 | 22 | SP13 (21) | - | Guarded visual fallback module (only if the spike says it is ne… |
| Z04 | 22 | A16 (19) | AGY,ATTN | Antigravity release gate: real-environment acceptance for AT-00… |
| H05 | W23 | none | - | Provide a clean Windows test environment for install, upgrade, … |
| Z07 | 23 | H05 (W23) | CLEAN,FG,ATTN | Clean-machine installer and lifecycle validation on the H05 env… |
| L04 | 24 | F04 (3) | ATTN | Window state persistence and single-instance behavior |
| Z05 | 24 | Z07 (23) | - | Requirements traceability report: every FR and AT with implemen… |
| H01 | W25 | none | - | Confirm Google position on external orchestration of the headle… |
| H04 | W25 | none | - | Decide and provision production release signing: Windows Authen… |
| Z06 | 25 | H04 (W25) | CLEAN,FG,ATTN | Release candidate validation against the release checklist |

## 10. High-risk areas

| Area | Result in the corrected schedule |
|---|---|
| Deadlocks, hidden cycles | none; the simulation always had a runnable ticket; DAG acyclic by independent recomputation |
| Massive bottlenecks | foundations only (section 4); S12 has a documented split; no accidental hub |
| Artificial serialization | only the strict key reading (section 6), rule D1 (one round), and the quiet-machine window |
| Unsafe parallelism | C5 and C6 were real; none after correction |
| Contract consumed before creation | C1 to C4 fixed; watch items W1, W5, W6 |
| Migration ordering | 16 distinct reserved blocks, unique numbers, ladder from empty with gaps; W1 is the only temporal subtlety |
| Shared surfaces, worktree conflicts | protocol partitions hold; each worker has its own worktree and branch from the recorded tip |
| Integration points too late | S12 round 13 (first merge lane), W01 round 14, A16 round 19; accepted (W8) |
| Test infrastructure | F03 round 3, F09 round 4, TK1 round 5, A02 round 4, before every consumer (first consumers: O01 round 6, A03 round 9) |
| Fixture infrastructure | F03 and `packages/test-fixtures` (F05 round 4) precede every UI and scenario ticket |
| UI foundations | F05 round 4, U01 round 5, U02 round 6, before S03's trust sheet (round 7) and every surface |
| Recovery infrastructure | F02 fault mechanism round 2, F03 harness round 3, R01 round 7, R02 round 12, R03 round 15; every slice declares and unit-tests its fault points as it lands; R03 now sees all of them |
| Approval Broker architecture | D05 round 3, D07 round 4, A04 round 4, K04 round 2, K01 round 8, K02 round 9, A06 round 11, A07 round 13, K05 round 12, K06 round 16, K07 round 17; all before A16 (round 19) |
| Policy engine | D05 round 3, before S03 (round 7), A05 (round 10), and X04 (round 17) |
| Antigravity adapter capabilities | A01 round 5, A02 4, TK1 5, A08 8, A03 9, A05 10, A09 10, A06 11, A07 13, A10 14, A11 15, A13 14, A14 13, A15 18, A16 19 |
| Desktop-visible conversations (ADR-020) | SP08 round 1 (first of all real-account spikes), A05 round 10, conversation ids persisted by S10 round 11 and S11 round 12, A12 round 17, Z04 round 22; none waits on H06 |
| Approval/UIA assuming H06 resolved | no: K05 and K06 use the fake window, K07 waits for the H06 window (round 17), SP12 and K08 are conditional (W4) |
| Clean-machine work before H05 | no: Z07 is the only consumer (round 23), after the H05 window (round 23); Z01 and Z02 need no VM |
| Production signing blocking development | no: H04 gates Z06 only |
| H01 or H04 blocking ordinary implementation | no: both gate Z06 only |
| Provider-policy handling after code that needs it | S16 round 13 and A13 round 14 precede A16 round 19, A15 round 18, and Z04 round 22; earlier attended real runs are W7 |
| Security gates late | X05 round 2, X02 round 5, X03 round 6, X01 round 11, X04 round 17; the normalizer corpus lives in D05 (round 3) |
| Performance instrumentation | F07 round 3, D04 round 4, U17 round 11, PF1 round 12; see W5 |
| Z07 as an accidental bottleneck | Z07 waits only for Z02, H05, W01, K10, L03, L01 (not for L02, R07, or S17); it is not on the DAG critical path; it lands in round 23 because the human and host are exclusive |
| Z06 evidence | Z06 is last; every RELEASE_CHECKLIST item maps to a ticket that is an ancestor of Z05 or Z06 (traceability Z05; secret scan X05; accessibility U13; reduced motion U14; graphics modes U03 and PF1; crash matrix R03; outage R06; Stop All R04, R08, R03; diagnostics X03; clean machine Z07; updater Z02; capability matrix and skills contract A15 and Z04; fidelity U19; signing H04 and Z06). No item lacks a producer. |

## 11. Scheduler behavior under failure states

State names are from `ORCHESTRATION_ENGINE.md` sections 1 to 3 and `CAPACITY_AND_PROFILES.md`. Reasoned over representative rounds (round 7: SP10 and eleven others; round 11: S10, A06, S17, U17, X01; round 14: A10, R08, S13, S14, W01, Z02, A13, U09; round 19: A16 and R06) and checked mechanically: a ticket only ever blocks its descendants, and the rest remain schedulable.

| State of one worker | Effect on that worker | Effect on unrelated work | Keys | Integration |
|---|---|---|---|---|
| `FAILED` | terminal for that ticket; branch and worktree preserved | all non-descendants proceed. Descendants blocked: S10 blocks 25 tickets, R01 31, S11 16, K01 11, SP08 13, A06 9 (F08 would block 72, which is why it is early) | release | unchanged |
| `NEEDS_HUMAN` (any kind) | parked with `resume_state`; a recorded decision resumes it | unrelated tickets proceed; the run goes `NEEDS_HUMAN` only for run-level kinds | release while parked (W2) | unchanged |
| `REVIEW_STALLED` | `NEEDS_HUMAN` kind `REVIEW_STALLED` after the iteration cap; never success | as above | release | unchanged |
| `WAITING_APPROVAL` | overlay sub-state; the worker keeps its state | only that worker waits | keeps its keys | unchanged |
| `APPROVAL_STALLED` | `NEEDS_HUMAN`; the watchdog and loop guard (K02) trip rather than loop | unrelated proceed | release | unchanged |
| `RATE_LIMITED` | `PAUSED` with reason `CAPACITY`; only the affected worker | unrelated proceed; no account switching | release | unchanged |
| provider `POLICY_BLOCKED` | profile `POLICY_BLOCKED`; provider work stops at a safe point; worker `NEEDS_HUMAN` kind `PROVIDER_POLICY_BLOCK`; the run goes `NEEDS_HUMAN` only if no ticket can progress without the profile | tickets that do not need the provider (every fixture, fake, UI, Git, and persistence ticket) continue; those that do wait on the user | release | unchanged |
| cancelled by Stop All | run `STOPPING` then `PAUSED` (reason `STOP_ALL`); branches and worktrees preserved; an in-progress merge validates or is discarded to `integration_before`, a push completes or is abandoned | everything pauses by design; resume re-enters the frontier | release | R08 defines the safe points; if neither can be established, health goes `UNKNOWN` |
| integration `UNKNOWN` or `UNHEALTHY` | running workers continue to `READY_TO_MERGE` and wait | **no new worker starts and none enters the lane** (intentional: dependent work must not advance while health is unknown); not a deadlock because reconciliation or a recorded decision restores health, otherwise the run becomes `NEEDS_HUMAN` (`INTEGRATION_UNKNOWN`) | held until the lane releases | blocks advancement as required |

Provider-policy-block handling in the graph (F02, D01, S15, S16, A13, U10, R02, Z03, F09) never retries, backs off, switches profile or account, replaces credentials, falls back to an API key, starts a new conversation, or clears on its own. R02 now depends on S15, so recovery restores the block exactly as recorded (C2). No state can cause a scheduling deadlock: the only global stall is integration health, and the only scheduler-visible wait on a person is the human gate, which holds one ticket.

## 12. Sensitivity runs (the primary result is the first row)

| Variant | Rounds | Notes |
|---|---|---|
| Primary: strict keys, rule D1, quiet window, unbounded concurrency | **25** | maximum 12 workers in one round (round 7) |
| Derived rule D1 off | 24 | D1 costs one round |
| PF1 and U17 run alone in their round | 27 | the quiet window is cheaper |
| Keys held only for the real session (gated real runs of A03, A05, A06, A07, A10, A11, A15, A16 queue in a lane) | 25 | with D1 off: 22, the human-attention bound |
| Audited graph, same rules | 25 | corrections cost no round |
| Concurrency cap 6 / 4 / 3 / 2 (2 is the product default for concurrent workers) | 28 / 35 / 45 / 66 | no deadlock at any cap |

## 13. Validation performed

| Check | Result |
|---|---|
| Every ticket scheduled exactly once | 135 scheduled (131 worker tickets in rounds, 4 human gates in windows); none missing, none duplicated |
| No ticket before a prerequisite | every published blocker finished in an earlier round or window; human gates satisfy in the window before the gated ticket's round |
| Exclusive keys respected | at most one holder per key in each of the 25 rounds; Z07 never with a real-agent or quiet-machine holder |
| Serialized integration | the lane takes one ticket at a time; the frontier is recomputed once per round after validation |
| Human gates labelled | four windows marked SIMULATED SATISFIED; no substantive H01, H04, H05, or H06 result invented |
| Acyclic final graph | independently recomputed from the ticket Markdown; no cycle, unknown, self, duplicate, or redundant published edge; the `ISSUE_GRAPH.md` table and waves equal `ISSUE_GRAPH.json` |
| FR-001..FR-050, AT-001..AT-026 | all covered; no ticket requirement text was changed |
| Write-surface safety | 0 unresolved overlaps; 57 migrations-only unordered pairs, each a distinct reserved block |
| Phase boundary | no source or application code added; H05 not provisioned; no GitHub issues published; Prompt 9 not begun |

## 14. Reproducing this check

The scratch scripts used here are disposable and not retained (a standing rule in `CURRENT_STATE.md`). To re-verify: rebuild the DAG from each ticket's header and section 7; confirm identity with `ISSUE_GRAPH.json` and the `ISSUE_GRAPH.md` table; recompute the transitive reduction, depths, frontier, and critical path; take the key holders from protocol section 8; walk the round table in section 7 checking rules R1 to R10 in section 2; and confirm every key appears exactly once in section 9.

## 15. Instructions for the next agent

Prompt 9 is the F01 bootstrap (the only code ticket in the initial frontier). Its prompt names a GitHub issue; GitHub publication has not happened and needs the user's explicit confirmation, so either the user publishes first or F01 runs from `docs/issues/graph/tickets/F01.md`. Follow the round order, the protocol's real-account spike order, and the human-gate triggers in section 5; do not provision H05 before round 19; do not resolve H01, H04, or H06 early to make progress.
