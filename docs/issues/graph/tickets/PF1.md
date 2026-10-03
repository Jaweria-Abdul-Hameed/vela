# PF1: Performance harness and budget measurement on the reference hardware

**Planned labels:** `type:test` `area:perf` `risk:medium` `phase:6` `ready-for-agent`  
**Depth in the DAG:** 11 · **Direct blockers (published, transitively reduced):** U17

> Symbolic key: `PF1` · Area: perf · Phase: 6 · Risk: medium

## 2. Objective
Startup, idle, minimized, event-burst, graph, and leak measurements run against the documented budgets.

## 3. Requirement IDs
FR-027, AT-009, NFR Performance

## 4. User-visible behavior
None (no user-visible behavior).

## 5. Technical scope
- Scripted measurements for cold launch, 100/500-node frame rates, idle and minimized CPU/GPU, event burst, and canvas mount/unmount memory; report with before and after evidence.
- Measure Full, Balanced (the default), and Efficiency, each against every budget, including reduced motion, minimized and hidden, unfocused, the device-pixel-ratio cap, and adaptive quality suggestions.
- Capture the per-run environment listed in the "Reference Hardware Profile" section of docs/ui/PERFORMANCE_BUDGET.md with every result, using read-only system queries only (no installs, no power-plan or other setting changes), and label each run "reference run" or "non-reference exploratory run" according to that section's "Conditions for a reference run".

## 6. Architectural constraints
- Budgets are initial; measure on recorded reference hardware before optimizing.
- The reference machine is profile REF-HW-1, recorded in docs/ui/PERFORMANCE_BUDGET.md (the former human action H03 was resolved before publication). Do not edit REF-HW-1: a hardware change (for example a RAM upgrade) is a new profile entry, and every result stays tied to the profile and the per-run capture it was measured on.
- Choosing the reference machine does not weaken any budget; do not invent or pre-fill benchmark numbers.
- Do not mandate or change an Acer or Nitro performance mode: record it if it can be reliably determined, otherwise record "not determined".

## 7. Dependencies / blocked by
- U17 Large graphs: 500-node performance and adaptive quality suggestions

## 8. Expected write surfaces
- tests/e2e/perf/**
- docs/research/PF1_performance_report.md (own record only)

## 9. Known shared contracts
- docs/ui/PERFORMANCE_BUDGET.md budgets and the Reference Hardware Profile (REF-HW-1) with its per-run capture list

## 10. Parallelization notes
After U17. The reference hardware profile is already recorded; it needs no ticket. The run needs the machine quiet (RK-QUIET-MACHINE).

## 11. Acceptance criteria
- [ ] Measured results per mode are recorded against each budget.
- [ ] AT-009 passes with orchestration events still journaled while minimized.
- [ ] Every recorded result names profile REF-HW-1, carries the per-run environment capture, and is labelled a reference run or a non-reference exploratory run. Only runs meeting every condition in "Conditions for a reference run" (REF-HW-1 hardware, AC power, a stable recorded power scheme, no competing heavy workload, full capture) count as release evidence; any other run is labelled non-reference and is not used to satisfy a budget.

## 12. Required unit tests
- Not applicable beyond the acceptance criteria.

## 13. Required integration tests
- Not applicable.

## 14. Required UI / visual tests
- Not applicable (no UI).

## 15. Required manual evidence
- Report on the reference hardware, including the per-run environment capture for each run.

## 16. Relevant specification documents
- `docs/ui/PERFORMANCE_BUDGET.md`
- `docs/testing/TEST_STRATEGY.md`

## 17. Relevant ADRs
- ADR-019

## 18. Non-goals
- Implementing any other ticket in the graph; only small prerequisite fixes strictly needed for correctness, recorded in the execution log.
- Adding Claude, Codex, or any other production runtime adapter (ADR-008).
- Reopening or redesigning accepted ADRs; contradictions become a NEEDS_HUMAN decision request.

## 19. Definition of done
- [ ] Every acceptance criterion is met and demonstrable.
- [ ] Required tests were written (TDD where appropriate), run, and are green; commands, exit codes, and durations are recorded in the completion report.
- [ ] Typecheck, lint, and build are green for everything the ticket touches.
- [ ] A checkpoint commit exists and the fixed-point /code-review passes the Engineering exit policy (docs/agents/REVIEW_PROTOCOL.md).
- [ ] No element tagged unverified [U] in docs/architecture/IMPLEMENTATION_ARCHITECTURE.md section 15 is claimed ready unless its spike ticket has closed with recorded evidence.
- [ ] Durable decisions or discoveries are written to the specification, an ADR, or docs/research, and docs/project/CURRENT_STATE.md is updated when the phase state changes.
- [ ] The structured completion report required by AGENTS.md is returned (base and final commits, files changed, tests, review result, branch/worktree, push status, merge risk, discovered dependencies).
