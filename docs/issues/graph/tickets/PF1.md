# PF1: Performance harness and budget measurement on the reference hardware

**Planned labels:** `type:test` `area:perf` `risk:medium` `phase:6` `ready-for-agent`  
**Depth in the DAG:** 10 · **Direct blockers (published, transitively reduced):** U17, H03

> Symbolic key: `PF1` · Area: perf · Phase: 6 · Risk: medium

## 2. Objective
Startup, idle, minimized, event-burst, graph, and leak measurements run against the documented budgets.

## 3. Requirement IDs
FR-027, AT-009, NFR Performance

## 4. User-visible behavior
None (no user-visible behavior).

## 5. Technical scope
- Scripted measurements for cold launch, 100/500-node frame rates, idle and minimized CPU/GPU, event burst, and canvas mount/unmount memory; report with before and after evidence.

## 6. Architectural constraints
- Budgets are initial; measure on recorded reference hardware before optimizing.

## 7. Dependencies / blocked by
- U17 Large graphs: 500-node performance and adaptive quality suggestions
- H03 Record the performance reference hardware profile

## 8. Expected write surfaces
- tests/e2e/perf/**
- docs/research/**

## 9. Known shared contracts
- docs/ui/PERFORMANCE_BUDGET.md budgets

## 10. Parallelization notes
After U17.

## 11. Acceptance criteria
- [ ] Measured results per mode are recorded against each budget; AT-009 passes with orchestration events still journaled while minimized.

## 12. Required unit tests
- Not applicable beyond the acceptance criteria.

## 13. Required integration tests
- Not applicable.

## 14. Required UI / visual tests
- Not applicable (no UI).

## 15. Required manual evidence
- Report on the reference hardware.

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
