# ADR-022: One Independent Review, Targeted Verification, Human Escalation (Product and Build Process)

## Status

Accepted (2026-10-11, approved by the user). Refines ADR-005 and ADR-015. Supersedes the review-loop default of
`ORCHESTRATION_ENGINE.md` section 4 (maximum iterations, default 3), FR-014 as previously worded, and ADR-021
decision 1 (scope: build process only) and decision 8 (one repair round). It does not change the Engineering exit
policy, severity classification, reviewer independence, checkpointing, the fixed-point rule, or any security gate.

## Context

Repeated full reviews cost a complete independent review each time, mostly rediscover edge cases in an open-ended
input space and defects the previous round of fixes introduced, and turn small low-severity remainders into stalls
(F02 needed six iterations; F07 three). The same rule governed both the process that builds Vela and the loop Vela
runs for its own workers (FR-014, D06, S11). ADR-021 changed only the first; the user has approved changing both.

## Decision

1. **One workflow, runtime and build time.** For every ticket, for the final integrated review, and for the people
   and agents building Vela:
   1. implementation and a **comprehensive self-audit** against every acceptance criterion (advisory,
      non-authoritative, recorded);
   2. a durable checkpoint whose required gates are green (ADR-005);
   3. **exactly one independent full review**, the *comprehensive review*;
   4. **fix every blocking finding and add a regression test** for each (or record why none can be written);
   5. rerun the required tests and CI, then **targeted independent verification** (decision 4);
   6. **complete only when** every acceptance criterion and quality gate passes (decision 6).
2. **No automatic repeat of a full review.** Neither a failed verification, a fix, an advance of the fixed point, a
   retry, a recovery, nor any configuration triggers a second comprehensive review. A second one exists only as an
   explicit human decision, recorded as such (`Cause::UserDecision` for the worker, an intervention decision in the
   journal).
3. **Blocking findings that remain go to a human.** If verification does not confirm every blocking finding
   resolved, or finds a blocking defect in the fix diff, or a high-severity defect anywhere, the worker (or run) becomes
   `NEEDS_HUMAN` with kind `REVIEW_STALLED`. There is no automatic repair round. **An exhausted review is never
   approval**: nothing in this workflow, and no human timeout or default, converts an unresolved blocking finding into a
   pass. A person may direct a further fix (followed by a new targeted verification they authorize), request a new
   comprehensive review, accept a documented risk on a *non-blocking* finding, or cancel; accepting an unresolved
   blocking finding is not an available outcome of the review workflow.
4. **Targeted verification.** A fresh independent session (never the worker's or the implementer's session, with no
   implementer context) receives the comprehensive review's blocking findings and the fix diff
   `review_head_sha..fix_head_sha`. It returns, for each blocking finding, `RESOLVED` or `NOT_RESOLVED` with evidence
   (including the regression test), and reviews the fix diff for defects the fixes introduced. It is not a second
   full review. A defect it notices outside the fix diff is a follow-up, unless it is high severity (a security
   vulnerability, data loss, a crash or hang, a missed acceptance criterion), which blocks wherever it is found.
   Severity and `blocking` are decided by Vela's Engineering policy, not by the verifier's prose.
5. **Review stages.** A review cycle has a `stage`: `COMPREHENSIVE` or `VERIFICATION`. A ticket has at most one
   `COMPREHENSIVE` cycle unless a human requests another. A `VERIFICATION` cycle follows each set of fixes (the fixes for
   the comprehensive review; the fixes after a failed integration validation; the resolution of a merge conflict). The
   worker state machine is unchanged: `REVIEWING` is entered for both stages, the first visit being the comprehensive
   review and each later visit a verification (`ORCHESTRATION_ENGINE.md` section 3). The bound is enforced by the review
   domain (D06) and the review service (S11), not by a state-table edge: no path leaves the review stages except to
   `READY_TO_MERGE` (pass) or `NEEDS_HUMAN` (escalation).
6. **Exit.** `PASS` when: the comprehensive review reported no blocking finding, or verification confirms every
   blocking finding `RESOLVED` with no blocking finding in the fix diff and no open high-severity finding; **and** the
   required tests, typecheck, lint and build are green on the final head; **and** the self-audit and the worker's
   completion report cover every acceptance criterion; **and** Vela's independent validation of the report agrees.
7. **Removed.** The review iteration cap (default 3) and oscillation detection: with one comprehensive review, one
   verification per fix set, and escalation on failure, a review cannot loop or oscillate. Finding identity (stable ids
   from a fingerprint) is kept: verification matches its verdicts to the findings by id.
8. **Integration regression.** A failed integration validation discards the merge (ADR-011), returns the worker to
   `FIXING`, and the fix is verified like any other (a `VERIFICATION` cycle). The count of such fix attempts is bounded
   by the setting `integration_fix_attempts` (default 3, configurable); exhaustion gives `NEEDS_HUMAN` kind
   `INTEGRATION_REGRESSION`. This bounds fixes, not full reviews.
9. **Conflict resolution.** A resolution attempt (ADR-011) is verified by a `VERIFICATION` cycle over the resolution
   diff at the advanced fixed point, plus the required gates. It does not trigger a comprehensive review.
   High-risk surfaces and any uncertainty still escalate to `NEEDS_HUMAN` kind `MERGE_CONFLICT`.
10. **Final integrated review.** The same shape at run level: one comprehensive review of the integrated change at
    `run_base_sha` (`FINAL_REVIEW`), fixes (`FINAL_FIXING`), a verification (the second visit to `FINAL_REVIEW`), then
    `FINAL_VALIDATION`. Verification failure makes the run `NEEDS_HUMAN` with kind `REVIEW_STALLED`.
11. **Independence and visibility preserved.** The comprehensive reviewer and the verifier each run in a fresh
    reviewer conversation distinct from every worker conversation (ADR-015). Both are authoritative reviewer sessions for
    FR-050, CAP-12 and AT-026: their conversation ids are recorded per review cycle and shown in the inspector.
12. **Crash recovery preserved.** The review cycle (stage, reviewed head, findings, verdicts, conversation id) is
    persisted in the same transaction as its journal event. A review or verification that was interrupted before its
    result was recorded is **re-run as the same cycle** (the operation is idempotent by operation id); that is not a
    second review. A recorded result is never replayed or re-requested. Recovery never starts a comprehensive review
    for a ticket that already has a recorded one.
13. **Unchanged.** The fixed-point rule and its advancement, the checkpoint invariant, the Engineering policy and its
    blocking classes (a reviewer's severity may not be lowered by the implementer or by Vela unless the policy says
    so), one correction retry for unparseable reviewer output then `NEEDS_HUMAN`, the security and supply-chain gates,
    the serialized merge lane, and human escalation.
14. **Configuration.** `Review maximum iterations` is removed. `integration_fix_attempts` is added. The review
    policy setting may tighten the blocking classes and select reviewer effort; it cannot loosen them or enable an
    automatic repeat of a full review.
15. **Effective date and scope of history.** Prospective. Tickets whose authoritative review began under the earlier
    rule (F01, F02, F03, D01, F07, and D02 and D05 in progress) keep their recorded history and verdicts; this ADR
    approves none of them and clears no existing `REVIEW_STALLED`. A ticket not yet reviewed uses this workflow.
    Product code that implements the old loop does not exist yet (D06 and S11 are not started).

## Rationale

- Independence is the safety property, not repetition: a fresh reviewer sees the whole change once, and a second
  fresh session confirms that what was found is fixed and that the fixes broke nothing.
- Regression tests turn "the reviewer was satisfied" into evidence, and verification is aimed where regressions
  come from, the fixes.
- Escalating a failed verification, rather than looping or retrying, keeps a person in the decision whenever
  automation could not close a blocking finding, and removes the state in which exhausted reviews could look like
  consent.
- Reusing `REVIEWING` and the existing edges keeps the verified state machine, the event vocabulary and the generated
  contracts intact; the change is in what the review domain permits.

## Consequences

- Specifications updated consistently: `PRODUCT_SPEC.md` (FR-013, FR-014, FR-050, section 2.8),
  `ACCEPTANCE_TESTS.md` (AT-004), `REQUIREMENTS_TRACEABILITY.md`, `ORCHESTRATION_ENGINE.md`, `DOMAIN_MODEL.md`,
  `COMPONENT_SPECIFICATIONS.md` (C09, FR coverage), `PERSISTENCE.md`, `CONFIGURATION.md`, `RECOVERY.md`,
  `AGENT_PROTOCOL.md`, `PROMPT_CONTRACTS.md`, `REVIEW_PROTOCOL.md`, `HUMAN_IN_THE_LOOP.md`, `COPY_AND_TONE.md`,
  `AGENTS.md`, the provider files, `CONTEXT.md`, the playbook, and ADR-021 (amended).
- Tickets updated: D06, S11 (rewritten scope and acceptance criteria), S13, A10, S14, A12; titles in
  `ISSUE_GRAPH.md`, `ISSUE_GRAPH.json` and `SCHEDULER_DRY_RUN.md`. No dependency edge, depth, frontier or write surface
  changed.
- No code change: the D01 transition tables and tests, the F02 event vocabulary (`ReviewStarted.iteration` is the
  review cycle ordinal; stage follows from it and from the persisted cycle) and the generated TypeScript contracts
  stay as merged. Adding an explicit `stage` to an event is a documented vocabulary change if the owners want it.
- Open items (decisions for the owners) are recorded in `docs/project/REVIEW_WORKFLOW_DECISION_REQUESTS.md`.
