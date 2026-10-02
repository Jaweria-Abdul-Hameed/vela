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

# Verified Facts (2026-10-02; see `docs/research/EXTERNAL_VERIFICATION_2026-10-02.md`)

-   **Present on the default branch:** `implement`, `implement-spec`, `code-review`, `to-tickets`,
    `to-spec`, `setup-matt-pocock-skills`, `tdd`, `pr`, `triage`, among others. Pin the installed version
    by commit SHA; the repository has no documented release tags.
-   **`implement`** implements a spec or set of tickets (not specifically an issue URL), uses TDD at
    agreed seams, runs the full suite once, invokes `/code-review`, and **commits afterwards** to the
    current branch. It does not say how it chooses the review fixed point.
-   **`code-review`** requires a fixed point, checks that it resolves and that the diff is non-empty, uses
    `git diff <fixed-point>...HEAD`, and runs parallel Standards and Spec sub-agents. Output is two
    headed sections plus a one-line summary. **It emits no severity scale and no machine-readable
    output**, so the reviewer-severity assumption in `REVIEW_PROTOCOL.md` and `PROMPT_CONTRACTS.md` is
    not met by the skill alone; Vela must obtain or compute structure itself.
-   **`implement-spec`** matches the description above (task graph, frontier, worktrees, integration
    branch, merger subagents, final review) and is framed for Claude Code subagents. Vela does not use it
    at runtime.
-   **Setup** (`/setup-matt-pocock-skills`) edits `CLAUDE.md` (else `AGENTS.md`) and writes
    `docs/agents/issue-tracker.md`, `docs/agents/domain.md`, and optionally `docs/agents/triage-labels.md`;
    its default domain layout assumes `GLOSSARY.md` and `docs/adr/`, which differ from this repository's
    `docs/decisions/`.
-   **Installation** options: `npx skills@latest add mattpocock/skills` (the installer supports an
    Antigravity target), a Claude Code plugin, or the setup skill. Antigravity discovers `SKILL.md` skills
    in `<workspace>/.agents/skills/`; global locations differ between documentation and the installer, so
    prefer project-level installation and detect at runtime. Skill behavior inside Antigravity is
    unproven until a real-environment probe passes.
