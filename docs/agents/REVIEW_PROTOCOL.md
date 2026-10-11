# Review Protocol

This document is the **canonical** definition of the review workflow and exit policy, for the review Vela runs
on its own workers and for the agents and people who build Vela (ADR-022). Summaries elsewhere (`AGENTS.md`) are
non-normative and defer to this file.

## Objective

Determine independently: 1. Does the change satisfy the originating
spec/ticket? 2. Does it satisfy repository engineering standards? 3. Are
there correctness/security/integration risks that required gates reveal?

## Fixed point

Never guess. Each worker stores the base integration SHA as `fixed_point_sha`. It is constant across
fixes, so the comprehensive review sees the cumulative diff `fixed_point_sha...review_head_sha` (the skill uses a
three-dot diff). **When Vela merges the integration tip into the worker branch, `fixed_point_sha` advances to that
integration tip**, so the diff continues to show only the ticket's own changes (and any conflict resolution) and not
other tickets' merged work. Review uses an explicit resolvable fixed point and a non-empty committed diff. Vela owns
the checkpoint invariant (ADR-015). Final review uses `run_base_sha` (ADR-014).

## Reviewer independence

The authoritative review runs in a **fresh reviewer session**, separate from the implementing
session and carrying no implementer conversation state. Reviews performed inside `/implement` are
advisory and non-authoritative, and so is the worker's own self-audit. Fixes continue in the ticket's worker session.
The verifier (below) is a further fresh session: not the worker's, and not the implementer's.

## Finding schema

Each finding: - ID, - axis/category, - severity, - evidence, - affected
location, - requirement violated, - actionable fix, - blocking
boolean, - disposition.

-   **Identity:** Vela assigns a stable finding ID from a fingerprint of category, location, and
    requirement, so the same finding is recognized in the verification that follows the review. A finding
    is `RESOLVED` only when the verifier reports it `RESOLVED` with evidence (the fix and its regression test);
    the worker's own statement is not evidence.
-   **Severity and blocking:** the `/code-review` skill, as verified on 2026-10-02, emits **no severity scale
    and no machine-readable output**: it reports `## Standards` and `## Spec` sections, with Standards findings
    split into hard violations and judgement calls. Severity and structure must therefore come from Vela's own
    reviewer prompt contract (`PROMPT_CONTRACTS.md`, optionally with the CLI's `--json-schema`) or be classified
    by Vela; the skill's native output is never assumed to carry severity. **Vela's policy engine decides
    `blocking`** from the exit policy below, not the reviewer's prose.
-   **External constraint (resolved by the fixed-point rule above):** `/code-review` computes
    `git diff <fixed-point>...HEAD`; advancing the fixed point to the merged integration tip prevents other tickets'
    changes from polluting the review.
-   **Output contract:** the reviewer and the verifier return the structured outputs defined in
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
review policy may tighten, but not loosen, the blocking classes, and cannot enable an automatic repeat of a full
review. A reviewer's severity is not lowered by the worker, by the implementer, or by Vela's orchestration; Vela's
policy engine classifies `blocking` from the policy above.

## Review workflow

Normative for the loop Vela runs on its workers and for agents and people building Vela (ADR-022; ADR-021 for the
build-time details). There is **one comprehensive review**, then verification of the fixes, then a human if anything
blocking remains. There is no iteration cap, because there is no loop; there is no oscillation, because a review is
not repeated.

`implement + self-audit → checkpoint (gates green) → ONE comprehensive review → classify → fix all blocking + regression tests → gates in CI → targeted verification → pass | NEEDS_HUMAN (REVIEW_STALLED)`

1.  **Self-audit.** The implementer audits the change against every acceptance criterion and records it. It is
    advisory; it never replaces the review.
2.  **Pre-review gate.** The checkpoint commit exists (ADR-005) and the required gates (tests, typecheck, lint,
    build, structure checks) are green on it. Review is not spent on a diff that does not build. In Vela's runtime the
    focused validation and the checkpoint invariant (ADR-015) are this gate.
3.  **Comprehensive review (`stage = COMPREHENSIVE`).** Exactly one independent full review of
    `fixed_point_sha...review_head_sha`, in a fresh reviewer session with no implementer context, on both axes (Spec,
    Standards), returning severity and category on every finding. Build-time effort is `high`, and `xhigh` for a
    ticket labelled `risk:high` or `area:security`.
4.  **Classify.** By the Engineering policy above.
5.  **Fix.** Every blocking finding is fixed, each with a regression test that fails without the fix (or a recorded
    reason why none can be written). Cheap non-blocking findings are fixed; the others are dispositioned in the
    ticket's evidence. The fixes are checkpointed and the required gates run again (in CI).
6.  **Targeted verification (`stage = VERIFICATION`).** A fresh, independent verifier session receives the blocking
    findings and the fix diff `review_head_sha..fix_head_sha`. For each blocking finding it returns `RESOLVED` or
    `NOT_RESOLVED` with evidence, and it reviews the fix diff for defects the fixes introduced. It does not re-review
    the whole change. A defect noticed outside the fix diff is a follow-up, unless it is high severity (a security
    vulnerability, data loss, a crash or hang, a missed acceptance criterion), which blocks wherever it is found. If
    the comprehensive review reported no blocking finding there is nothing to verify and this step is skipped.
7.  **Failure goes to a human.** A blocking finding `NOT_RESOLVED`, a blocking defect in the fix diff, or an open
    high-severity defect makes the worker `NEEDS_HUMAN` with kind `REVIEW_STALLED`. There is no automatic second
    round. A person may direct a further fix (with a new targeted verification they authorize), request a new
    comprehensive review, accept a documented risk on a *non-blocking* finding, or cancel. An unresolved blocking
    finding is never accepted by the workflow.
8.  **Never automatic, never approval.** A full review is repeated only on an explicit human decision, recorded as
    such. Advancing the fixed point, a fix, a retry or a recovery never starts one. Exhaustion, a timeout, a default,
    or silence is never a pass.

Other fix sources use the same verification, not a new full review: the fix after a failed integration validation
(bounded by `integration_fix_attempts`, ADR-022 decision 8) and the resolution of a merge conflict (verified over the
resolution diff at the advanced fixed point, with the required gates; ADR-022 decision 9). The final integrated review
(`FINAL_REVIEW` at `run_base_sha`) has the same shape at run level.

An interrupted review or verification (no result recorded) is re-run as the same cycle by operation id; a recorded
result is never replayed, and recovery never starts a comprehensive review for a ticket that already has a recorded
one (`RECOVERY.md`).

## Exit

`PASS` only when all of these hold: the comprehensive review reported no blocking finding, or the verifier reports
every blocking finding `RESOLVED`, no blocking defect in the fix diff and no open high-severity defect; the required
tests, typecheck, lint and build are green on the final head; every acceptance criterion is covered by the worker's
report and Vela's independent validation agrees. Anything else is `NEEDS_HUMAN` with kind `REVIEW_STALLED` (or the
kind of the failing gate), never success.

## Build-time details (ADR-021, ADR-022)

Agents and people building Vela follow the workflow above, with these additions. The review and verification are
performed in fresh sessions that are not the implementer (a subagent or a separate reviewer session); the evidence
file for the ticket records the review (findings, severities, dispositions), the regression test for each blocking
fix, the CI runs, and the verifier's verdict per finding. If verification fails the ticket stops at `REVIEW_STALLED`
for a human decision; nobody self-authorizes a further full review or a further verification round, and a ticket is
never merged on an unverified blocking fix.

**Applicability.** The workflow applies to a ticket whose first authoritative review starts after ADR-022 is on
`main`. A ticket already under review continues under the rules it started with (up to three full iterations, then
`REVIEW_STALLED`). ADR-022 approves no ticket and clears no existing stall.
