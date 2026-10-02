# TK1: Scripted fake agy executable replaying recorded Antigravity behavior

**Planned labels:** `type:test` `area:antigravity` `risk:medium` `phase:1` `ready-for-agent`  
**Depth in the DAG:** 4 · **Direct blockers (published, transitively reduced):** A02  
**Added by the Prompt 6 dependency audit** (not in the first draft).

> Symbolic key: `TK1` · Area: antigravity · Phase: 1 · Risk: medium

## 2. Objective
Tests can run the real session driver and policy code against an executable that behaves like agy 1.2.14 as verified in Section S, without touching a real installation.

## 3. Requirement IDs
NFR Testability, FR-037

## 4. User-visible behavior
None (no user-visible behavior).

## 5. Technical scope
- A fake agy executable that replays recorded stream-json transcripts and reproduces verified behaviors: soft-denial of commands with exit 0 and stderr text, allow-rule semantics (exact, regex, path-scoped write), hook invocation with the verified input and fail-closed handling, hook deny, conversation resume, quota text, auth error, help output, and timeouts.
- Scenario scripting API and a conformance suite derived from the recorded transcripts; versioned to the agy version that produced them.

## 6. Architectural constraints
- Only vela-adapters::antigravity and vela-uia may contain Antigravity-specific behavior (ADR-008, ADR-018).
- Never trust exit code or result.status; interpret events, stderr, and Git state.
- Never read, copy, or persist Antigravity credentials or tokens (ADR-016).
- Elements tagged [U] are gated by their spike; do not claim them ready.

## 7. Dependencies / blocked by
- A02 stream-json parser and result interpreter against recorded transcripts

## 8. Expected write surfaces
- crates/vela-testkit/src/fake_agy/**
- tests/fixtures/transcripts/** (shared with A02: serialize after A02)

## 9. Known shared contracts
- AgentAdapter port and the CAP-01..CAP-12 capability contract
- Hook rule-table format (vela-domain hook-table feature)
- Fake agy scenario format (used by A03, A05, A06, A07, A15)

## 10. Parallelization notes
After A02; blocks the Antigravity adapter tickets that are accepted against the fake. Parallel with A01.

## 11. Acceptance criteria
- [ ] Each recorded Section S scenario (soft-denial, allow rule, hook deny, hook crash, resume, quota) is reproduced and classified identically by the real parser.
- [ ] The fake never reads the user home directory, credentials, or global settings.

## 12. Required unit tests
- Scenario conformance tests.

## 13. Required integration tests
- Parser plus fake end-to-end scenarios.

## 14. Required UI / visual tests
- Not applicable (no UI).

## 15. Required manual evidence
- None required.

## 16. Relevant specification documents
- `docs/architecture/COMPONENT_SPECIFICATIONS.md`
- `docs/architecture/ADAPTERS.md`
- `docs/research/EXTERNAL_VERIFICATION_2026-10-02.md`

## 17. Relevant ADRs
- ADR-008
- ADR-009
- ADR-016
- ADR-018
- ADR-020

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
