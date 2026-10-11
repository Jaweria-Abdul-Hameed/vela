# ADR-021: Build-Time Review Workflow (One Comprehensive Review, Then Targeted Verification)

## Status

Accepted (2026-10-11). Refines ADR-015 for the agents that build Vela. It does not change ADR-005 or ADR-015 or the
Engineering exit policy.

**Amended by ADR-022 (2026-10-11):** decision 1 (the runtime loop is unchanged) and decision 8 (one repair round)
are superseded. The same workflow now governs Vela's own runtime review, and a failed verification goes straight to a
human with no automatic repair round. The remaining decisions stand.

## Context

Tickets F01, F02, F03, D01 and F07 were each reviewed with up to three full `/code-review` iterations (F02 needed six).
Every iteration re-read the whole cumulative diff in a fresh session. The pattern in the evidence files is the same
each time: the first review finds most of the real defects; later full reviews mostly find further edge cases in an
open-ended input space and defects introduced by the previous round of fixes, at the cost of a complete review each
time, and a ticket whose third review is not clean stalls regardless of how small the remaining items are.

The three-iteration limit also appears in the **product** specification, as the default of Vela's own configurable
review loop (`ORCHESTRATION_ENGINE.md` section 4, `CONFIGURATION.md`, tickets D06 and S11, `COPY_AND_TONE.md`). That is
runtime behavior of the shipped application, frozen with the architecture and covered by ticket acceptance criteria.

## Decision

1. **Scope.** *(Superseded by ADR-022: the workflow now also governs the runtime loop.)* This ADR governs how agents and people **building Vela** review a ticket (the build-time process:
   `AGENTS.md`, the playbook, the "Build-time review workflow" section of `REVIEW_PROTOCOL.md`). It does not change
   the review loop Vela runs for its own workers at runtime: that loop keeps its configurable iteration cap (default 3),
   its states and its tickets exactly as specified.
2. **One comprehensive authoritative review.** After the pre-review gate (item 3), a ticket receives **one**
   `/code-review` of its whole cumulative diff against the recorded fixed point, in a fresh reviewer session with no
   implementer context (ADR-015), on both axes (Spec and Standards), with a severity and a category on every finding.
   Effort is `high`; it is `xhigh` for a ticket labelled `risk:high` or `area:security`.
3. **Pre-review gate.** The review starts only on a checkpoint commit (ADR-005) whose required gates are green in CI
   (tests, typecheck, lint, build, structure checks), after the implementer's own audit against every acceptance
   criterion. A review is not spent on a diff that does not build.
4. **Classification is unchanged.** Findings are classified by the Engineering policy in `REVIEW_PROTOCOL.md`. The
   blocking classes are not loosened. The implementer does not lower a reviewer's severity; a disputed classification
   is a human decision.
5. **Fix.** Every blocking finding is fixed. Each fix carries a regression test that fails without it, or a recorded
   reason why none can be written. Cheap non-blocking findings are fixed; the rest are dispositioned in the evidence
   file. The fixes are checkpointed and the full required gates run in CI on the fix head.
6. **Targeted independent verification.** A fresh, independent verifier session (not the implementer, no implementer
   context) then checks, and only checks:
   - each blocking finding: resolved or not, by reading the fix and its regression test;
   - the fix diff (`review_head_sha..fix_head_sha`): defects the fixes introduced.

   It is not a second full review. A defect it notices outside the fix diff is recorded as a follow-up and does not
   gate the ticket, **unless it is high severity** (a security vulnerability, data loss, a crash or hang, or a missed
   acceptance criterion): a high-severity defect blocks whenever and however it is found.
7. **Exit.** `PASS` when the comprehensive review reported no blocking finding (no verification is needed), or when
   verification confirms every blocking finding resolved and reports no blocking finding in the fix diff; in both
   cases with the required gates green on the final head.
8. **One repair round, then a human.** *(Superseded by ADR-022 decision 3: a failed verification goes to a human; there is no repair round.)* If verification reports a blocking finding unresolved or a blocking defect in
   the fix diff, the implementer gets one repair round (fix, regression test, CI) and the verifier re-checks those
   items only. If that does not pass, the ticket is `NEEDS_HUMAN` with kind `REVIEW_STALLED`. Nobody self-authorizes
   a further full review or a further verification round, and a ticket is never merged on an unverified blocking fix.
9. **When a second full review is required.** If the fixes replace the reviewed design rather than repair it (the fix
   diff is no longer something a targeted check can cover), the ticket goes to a human, who may authorize a new
   comprehensive review. That is a decision, never a default.
10. **Unchanged gates.** The fixed-point rule and its advancement, checkpoints (ADR-005), reviewer independence
    (ADR-015), severity classification, the required test, typecheck and build gates, the CI supply-chain and secret
    scanning gates, write-surface and scope checks, the serialized merge lane, post-merge CI, the final integration
    review of the whole product (playbook final-review prompt, run with this same workflow), and every release gate
    remain as they are.
11. **Prospective.** The workflow applies to a ticket whose first authoritative review starts after this ADR is on
    `main`. A ticket already under review continues under the rules it started with. This ADR approves no ticket and
    clears no stall: D02 and D05 keep the state recorded for them, and an existing `REVIEW_STALLED` is resolved only by
    the human decision it was raised for.

## Rationale

- The independent gate is kept, and kept honest: a fresh reviewer sees the whole change once, at higher effort where
  the risk is higher, and a second independent session confirms that what was found is fixed. What is removed is the
  repeated full re-read, not the independence.
- Blocking fixes are exactly where regressions come from, so verification is aimed there, with regression tests as
  evidence rather than a reviewer's impression.
- The high-severity rule keeps the narrower verification from becoming a way to miss something serious.
- A stall still stops the ticket and still goes to a person; the workflow only stops manufacturing stalls from
  low-severity findings that arrive in a third pass.

## Consequences

- `REVIEW_PROTOCOL.md` gains a "Build-time review workflow" section; `AGENTS.md`, the provider files and the playbook
  prompts for ticket implementation, the worker and the final integrated review are aligned to it.
- Evidence files record: the comprehensive review (findings, severities, dispositions), the regression test for each
  blocking fix, the CI run on the fix head, and the verifier's per-finding verdict.
- When this ADR was written the product specifications were not changed; ADR-022 (2026-10-11, user-approved) applied
  the same shape to Vela's runtime review loop and updated them.
