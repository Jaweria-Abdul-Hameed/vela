# Matt Pocock Skills Integration

## Purpose

Vela should leverage installed engineering skills rather than clone
their behavior unnecessarily, while compensating for
integration-specific gaps.

Current upstream documentation describes: - `implement`: one decided
ticket/spec, TDD where appropriate, regular typechecks/focused tests,
full suite near the end, code review, commit. - `code-review`:
fixed-point review along Standards and Spec axes. - `implement-spec`:
whole-spec task graph, ready frontier, isolated worktrees/branches,
integration branch, merger agents, final review.

## Bootstrap

At preflight: 1. detect whether required skills exist, 2. detect
issue-tracker setup, 3. if absent, offer the documented
installation/setup path, 4. wait for completion, 5. verify
commands/skills are actually callable, 6. record version/commit where
possible.

Never claim a skill ran unless trace evidence confirms invocation.

## Vela's added responsibilities

Vela owns: - durable run state, - cross-session scheduling, -
capability detection, - conflict forecasting, - policy enforcement, -
UI, - recovery, - integration health, - provider abstraction, - event
history.

## Bootstrap actions and install locations

Installing skills is an explicit, user-approved bootstrap step (never silent; `PREFLIGHT.md`). The
install location is determined by the installed Antigravity's supported mechanism, verified by
Prompt 4; it is not assumed here. If installation changes repository-tracked files, it happens on a
separate Vela-created branch and commit, never on the user's current branch or dirty tree. Skill
output and skill-supplied text are untrusted input to Vela's policy (ADR-013).

## Skill-internal review is advisory

`implement` includes its own code review and commit. Vela treats that review as non-authoritative
and its commits as ordinary history. The authoritative review is Vela's separately orchestrated
`/code-review` against the recorded fixed point in a fresh reviewer session (ADR-015,
`REVIEW_PROTOCOL.md`).

## Important review invariant

Upstream documentation notes a practical fixed-point/diff concern:
authoritative review must have a non-empty committed diff visible from
its chosen fixed point. Vela therefore records the base SHA and creates
a durable checkpoint before its authoritative review gate.

## Upstream drift

Skills are external dependencies. Pin or record the installed version
and periodically validate assumptions with contract/integration tests.

# Antigravity v1 Integration

For v1, skill discovery/bootstrap and the documented `/implement` / `/code-review` workflow must be validated in the Antigravity execution environment Vela actually controls. Support in some other development agent does not substitute for Antigravity compatibility.
