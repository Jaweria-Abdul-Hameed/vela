# PV1: Worktree provisioning contract: confirmed commands, file allowlist, caches, resource keys

**Planned labels:** `type:feature` `area:git` `risk:high` `phase:1` `ready-for-agent`  
**Depth in the DAG:** 7 · **Direct blockers (published, transitively reduced):** G02, S03

> Symbolic key: `PV1` · Area: git · Phase: 1 · Risk: high

## 2. Objective
A fresh worktree becomes runnable through user-confirmed provisioning commands, with failures reported before the worker starts.

## 3. Requirement IDs
FR-044, AT-001

## 4. User-visible behavior
None (no user-visible behavior).

## 5. Technical scope
- Project provisioning profile (commands, explicit file-copy allowlist, shared-cache declarations, resource keys); run only in trusted repositories through the policy engine; no automatic secret copying; failure is a validation failure before the worker starts.

## 6. Architectural constraints
- Shell out to the git binary under the verified hardened invocation (IMPLEMENTATION_ARCHITECTURE.md; no libgit2).
- Never force-push; never rebase pushed branches; never touch the primary checkout working tree, index, or branch (ADR-011).

## 7. Dependencies / blocked by
- G02 Worktree manager: root policy, create, remove, lock, repair, slug and case safety, cleanup
- S03 Repository trust: untrusted by default, trust sheet, data-only analysis, Build refusal

## 8. Expected write surfaces
- crates/vela-core/src/provisioning/**

## 9. Known shared contracts
- GitAdapter port
- per-worktree operation lock

## 10. Parallelization notes
Parallel with S09.

## 11. Acceptance criteria
- [ ] Provisioning runs confirmed commands only, through the policy engine.
- [ ] An allowlisted file is copied without its values appearing in any log (log-capture test).
- [ ] A provisioning failure is a validation failure and prevents the worker from starting.
- [ ] An untrusted repository never provisions.

## 12. Required unit tests
- Plan validation (only confirmed commands); allowlist copy value non-disclosure; resource-key collision yields a serialization hint; cache declaration handling.

## 13. Required integration tests
- Fixture with dependency install stand-in.

## 14. Required UI / visual tests
- Not applicable (no UI).

## 15. Required manual evidence
- None required.

## 16. Relevant specification documents
- `docs/architecture/COMPONENT_SPECIFICATIONS.md`
- `docs/git/GIT_WORKFLOW.md`

## 17. Relevant ADRs
- ADR-003
- ADR-011
- ADR-013
- ADR-015

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
