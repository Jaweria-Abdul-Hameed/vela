# AGENTS.md --- Universal Repository Instructions

These instructions apply to **all AI coding agents** working in this
repository, including Antigravity/Gemini, Codex, Claude, and any future
provider. Provider-specific files may add ergonomics but may not weaken
these rules.

## Mission

Build Vela according to the repository specifications. Do not silently
redesign settled product behavior. If implementation reality contradicts
a specification, stop and create a concise decision record or
`NEEDS_HUMAN` report rather than improvising a product change.

## Required reading order

Before implementing a ticket:

1.  This file.
2.  The ticket in full, including blocking relationships and acceptance
    criteria.
3.  `docs/product/PRODUCT_SPEC.md`.
4.  Relevant architecture/UI/security/testing documents referenced by
    the ticket.
5.  Relevant ADRs.
6.  Existing implementation and tests in the touched area.
7.  The integration branch history since the ticket was authored if it
    may affect assumptions.

Do not load every document blindly into context. Follow references and
load what is relevant.

## Ticket scope

One implementation session owns one ticket. A ticket is expected to be a
tracer-bullet vertical slice. Do not opportunistically implement
unrelated tickets. Small prerequisite fixes are allowed only when
necessary to make the ticket correct; record them in the execution log.

## Planning behavior

The issue/spec is already the plan. Do not reopen settled design choices
during `/implement`. If the ticket is impossible, contradictory, unsafe,
or underspecified in a way that changes externally visible behavior,
transition it to `NEEDS_HUMAN` with: - exact ambiguity, - affected
files/contracts, - viable alternatives, - consequence of each
alternative, - smallest decision required.

## Git isolation

Parallel work must never share a working tree. Each concurrent ticket
receives: - its own branch, - its own worktree, - a recorded base
integration commit.

Before work begins, verify the worktree branch is based on the expected
integration tip. Do not use `git stash` as an isolation mechanism; stash
refs are repository-wide and can create cross-worktree surprises.

Never force-push protected/integration branches. Never run destructive
reset/clean operations outside the assigned worktree. Never rewrite
another worker's branch.

## Implementation loop

Preferred flow:

``` text
read ticket
→ inspect relevant code
→ identify testable seams
→ TDD where appropriate
→ implement smallest vertical slice
→ run focused tests
→ typecheck/lint frequently
→ complete ticket
→ run full required ticket gate
→ create durable checkpoint commit
→ run code review against the recorded fixed point
→ fix actionable findings
→ rerun affected tests
→ repeat review until exit policy passes
→ final ticket commit
→ push
→ report structured result
```

If Matt Pocock skills are installed, use their intended semantics. Do
not pretend to invoke a skill: the execution trace must show the real
invocation when supported.

## Review exit policy

Default Vela policy is **Engineering**: - zero blocking findings, -
zero high-severity correctness/security/spec findings, - zero unresolved
medium correctness/spec findings, - tests green, - typecheck green, -
build green when the ticket affects buildable output.

Pure naming preferences, speculative refactors, formatting already
enforced by tooling, and subjective micro-style findings do not block
completion unless repository standards explicitly make them
requirements.

A review loop has a configurable maximum iteration count. Hitting the
maximum is not success; transition to `NEEDS_HUMAN` or `REVIEW_STALLED`.

## Tests

Never delete, weaken, skip, or rewrite a failing test merely to obtain
green status unless the ticket explicitly changes that behavior and the
new expectation is justified by the spec.

Record: - command, - exit code, - duration when available, - summary, -
relevant failure excerpt.

## Git commits

Commits must be coherent and recoverable. Include issue references.
Prefer Conventional Commit style unless repository policy overrides it.

Examples: - `feat(orchestrator): persist worker state (#42)` -
`fix(graph): prevent unsafe schema tickets from running concurrently (#51)` -
`test(review): cover stalled review loop (#63)`

Do not commit secrets, tokens, generated credentials, local account
state, `.env` values, or provider cookies.

## External research

For volatile APIs or tool behavior, prefer primary documentation. Record
material findings under `docs/research/` when they affect
implementation. Never encode undocumented third-party behavior as a core
invariant without an adapter/fallback.

## UI implementation rules

The UI is not a generic admin dashboard. Before UI work, read: -
`docs/ui/UI_UX_SPEC.md` - `docs/ui/DESIGN_SYSTEM.md` -
`docs/ui/MOTION_AND_3D.md` - `docs/ui/ACCESSIBILITY.md` -
`docs/ui/PERFORMANCE_BUDGET.md`

Do not introduce conventional permanent sidebars, dense card grids,
gratuitous glass panels, random neon, or animation unrelated to state.

## Safety

Automatic approval is not permission for recklessness. Commands that can
destroy uncommitted work, expose secrets, alter credentials, affect
unrelated directories, rewrite remote history, or perform irreversible
external actions must obey Vela's policy engine.

## Completion report

Every worker returns structured facts: - ticket ID/title, - base
commit, - final commit(s), - files changed, - tests/commands and
results, - review result, - unresolved warnings, - branch/worktree, -
push status, - merge risk, - follow-up dependencies discovered.

Never report success if a required gate was skipped.

# Approval Broker and Unattended Execution

Native Antigravity permission configuration is preferred but is **not
assumed sufficient**. The user's observed environment continues to
surface approval prompts even when permissive settings are selected.

When Vela is supervising a worker: - ordinary project-scoped actions
may be approved automatically only after Vela's own policy engine
classifies the exact request as allowed; - an approval UI is a transport
surface, not the authority; - unknown, destructive, credential-related,
outside-workspace, or policy-sensitive requests must not be
auto-approved; - repeated equivalent approval prompts must trip an
`APPROVAL_STALLED` guard rather than loop forever; - approval decisions
and delivery results are journaled without leaking secrets.

If native permission APIs/settings cannot deliver an already-authorized
decision, a guarded Antigravity UI-automation adapter may deliver the
approval using robust accessibility/control discovery first and
visual/pixel methods only as a last-resort compatibility path.

# Runtime Target Invariant

All agents working on Vela must preserve this distinction:

- **Build-time agents:** Claude, Gemini, Codex, Antigravity, or other tools may be used to implement/review Vela.
- **Vela v1 runtime target:** Google Antigravity on Windows is first-class, default, required, and release-blocking.
- **Future runtime targets:** Claude/Codex/other coding-agent adapters remain extension possibilities, not v1 deliverables unless an approved issue explicitly changes scope.

Do not generate, implement, or prioritize peer runtime adapters merely because the core uses `AgentAdapter`.

Provider independence is an internal architectural property, not an equal-provider product promise.
