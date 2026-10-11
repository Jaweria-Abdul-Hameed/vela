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

This section and "Exit" describe the review loop **Vela runs for its own workers at runtime**
(`ORCHESTRATION_ENGINE.md` section 4). Agents building Vela follow "Build-time review workflow" below.

`review → classify → fix blocking/actionable → focused tests → checkpoint → review again`.

Do not blindly fix contradictory review suggestions. If repeated reviews
oscillate, mark `REVIEW_STALLED` (a `NEEDS_HUMAN` kind). Reaching the iteration limit
(default 3) is also `REVIEW_STALLED`, never success.

## Exit

`PASS` only when blocking findings are zero and required validation
gates pass.

## Build-time review workflow

Normative for agents and people **building Vela** (ADR-021). Everything above this section applies to it unchanged
(objective, fixed point, reviewer independence, finding schema, the Engineering policy and its blocking classes)
except the shape of the loop: the default is **one comprehensive review followed by targeted verification of the
blocking fixes**, not repeated full reviews. The runtime loop above is not affected.

1.  **Pre-review gate.** The implementer audits the change against every acceptance criterion, creates the
    checkpoint commit, and the required gates (tests, typecheck, lint, build, structure checks) are green in CI on
    that commit. Review does not start before this.
2.  **Comprehensive review.** One `/code-review` of the whole cumulative diff
    `fixed_point_sha...review_head_sha`, in a fresh reviewer session with no implementer context, on both axes
    (Spec, Standards), returning the reviewer output contract of `PROMPT_CONTRACTS.md` with a severity and a
    category on every finding. Effort `high`; `xhigh` for a ticket labelled `risk:high` or `area:security`.
3.  **Classify.** By the Engineering policy above. The implementer does not lower a reviewer's severity; a disputed
    classification is a human decision.
4.  **Fix.** Every blocking finding is fixed, each with a regression test that fails without the fix (or a recorded
    reason why none can be written). Cheap non-blocking findings are fixed; the others are dispositioned in the
    ticket's evidence file. Checkpoint; the full required gates run in CI on the fix head.
5.  **Targeted independent verification.** A fresh, independent verifier session (not the implementer, no
    implementer context) reports, for each blocking finding, `resolved` or `not resolved` with evidence, and reviews
    the fix diff `review_head_sha..fix_head_sha` for defects the fixes introduced. It does not re-review the whole
    change. A defect it notices outside the fix diff is recorded as a follow-up and does not gate the ticket unless
    it is high severity (a security vulnerability, data loss, a crash or hang, a missed acceptance criterion); a
    high-severity defect blocks whenever it is found.
6.  **Exit.** `PASS` when the comprehensive review reported no blocking finding (verification is then not needed),
    or when verification confirms every blocking finding resolved and reports no blocking finding in the fix diff;
    in both cases with the required gates green on the final head.
7.  **One repair round.** If verification fails, the implementer fixes the reported items (regression test, CI) and
    the verifier re-checks those items only. If that fails too, the ticket is `NEEDS_HUMAN` with kind
    `REVIEW_STALLED`.
8.  **Escalation.** No one self-authorizes a further full review or a further verification round, and a ticket is
    never merged on an unverified blocking fix. If the fixes replace the reviewed design rather than repair it, the
    ticket goes to a human, who may authorize a new comprehensive review.

The evidence file records the review (findings, severities, dispositions), the regression test for each blocking
fix, the CI runs, and the verifier's verdict per finding.

**Applicability.** A ticket whose first authoritative review starts after ADR-021 is on `main` uses this workflow. A
ticket already under review continues under the rules it started with (up to three full iterations, then
`REVIEW_STALLED`). ADR-021 approves no ticket and clears no existing stall.
