# Review Protocol

This document is the **canonical** definition of the review exit policy. Summaries elsewhere
(`AGENTS.md`) are non-normative and defer to this file.

## Objective

Determine independently: 1. Does the change satisfy the originating
spec/ticket? 2. Does it satisfy repository engineering standards? 3. Are
there correctness/security/integration risks that required gates reveal?

## Fixed point

Never guess. Each worker stores the base integration SHA as `fixed_point_sha`. It is constant across
fix iterations, so each review sees the cumulative diff `fixed_point_sha...review_head_sha` (the skill uses a
three-dot diff). **When Vela merges the integration tip into the worker branch, `fixed_point_sha` advances to that
integration tip**, so the diff continues to show only the ticket's own changes (and any conflict resolution) and not
other tickets' merged work. Review uses an explicit resolvable fixed point and a non-empty committed diff. Vela owns
the checkpoint invariant (ADR-015). Final review uses `run_base_sha` (ADR-014).

## Reviewer independence

The authoritative review runs in a **fresh reviewer session**, separate from the implementing
session and carrying no implementer conversation state. Reviews performed inside `/implement` are
advisory and non-authoritative. Fix iterations continue in the ticket's worker session.

## Finding schema

Each finding: - ID, - axis/category, - severity, - evidence, - affected
location, - requirement violated, - actionable fix, - blocking
boolean, - disposition.

-   **Identity:** Vela assigns a stable finding ID from a fingerprint of category, location, and
    requirement, so the same finding is recognized across iterations. A finding is `RESOLVED` only
    when a later review does not re-raise it at the new head; reviewer re-confirmation is the
    evidence.
-   **Severity and blocking:** the `/code-review` skill, as verified on 2026-10-02, emits **no severity scale
    and no machine-readable output**: it reports `## Standards` and `## Spec` sections, with Standards findings
    split into hard violations and judgement calls. Severity and structure must therefore come from Vela's own
    reviewer prompt contract (`PROMPT_CONTRACTS.md`, optionally with the CLI's `--json-schema`) or be classified
    by Vela; the skill's native output is never assumed to carry severity. **Vela's policy engine decides
    `blocking`** from the exit policy below, not the reviewer's prose.
-   **External constraint (resolved by the fixed-point rule above):** `/code-review` computes
    `git diff <fixed-point>...HEAD`; advancing the fixed point to the merged integration tip prevents other tickets'
    changes from polluting the review.
-   **Output contract:** the reviewer returns the structured review output defined in
    `PROMPT_CONTRACTS.md`. Unparseable output gets one correction retry, then the worker becomes
    `NEEDS_HUMAN`.

## Default Engineering policy

Blocks: - security vulnerabilities introduced by diff, - incorrect
behavior, - missing acceptance criteria, - broken
tests/build/typecheck, - architecture boundary violation with real
impact, - data loss/race/error-path bugs, - medium-or-higher
spec/correctness findings, - any high-severity correctness, security, or spec finding,
- changes to execution-defining files (ADR-013) that are not explained by the ticket scope.

Normally nonblocking: - subjective naming, - optional abstraction, -
speculative future-proofing, - style already accepted by
formatter/linter, - micro-optimizations without measured need.

The exit condition also requires: tests green, typecheck green, and build green when the ticket
affects buildable output. "Engineering" is the only review policy defined in v1; configurable
review policy selects the iteration limit and may tighten, but not loosen, the blocking classes.

## Loop

`review → classify → fix blocking/actionable → focused tests → checkpoint → review again`.

Do not blindly fix contradictory review suggestions. If repeated reviews
oscillate, mark `REVIEW_STALLED` (a `NEEDS_HUMAN` kind). Reaching the iteration limit
(default 3) is also `REVIEW_STALLED`, never success.

## Exit

`PASS` only when blocking findings are zero and required validation
gates pass.
