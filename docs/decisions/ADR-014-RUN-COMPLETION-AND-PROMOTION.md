# ADR-014: Run Completion and Promotion Policy

## Status

Accepted (2026-10-02).

## Context

Prompt 2 finding SA-21: how a validated integration branch reaches the default branch, PR
policy, no-remote repositories, and the fixed point of the final review were undefined. Human
decision (2026-10-02): default promotion is PR with human-controlled merge; Vela does not merge
`main` silently; without a remote the result is a validated local integration branch.

## Decision

1. **Default promotion with a remote:** validated integration branch -> pull request ->
   human-controlled merge into the default branch. Vela does not merge the default branch by
   default.
2. **PR lifecycle.** A draft PR may be opened once the integration branch has content; it is
   marked ready for review only after final validation passes. Vela does not merge the PR.
3. **No remote.** Completion yields a validated local integration branch recorded as ready for
   human promotion.
4. **Promotion mode** is a per-project policy: `pr` (default), `local_only`, or an explicitly
   opted-in mode that merges the default branch automatically. Any mode that merges the default
   branch requires explicit per-project opt-in with a visible warning, and is never the default.
5. **Run outcome record.** `COMPLETED` carries `promotion` = `PR_OPEN`, `PUSHED_NO_PR`,
   `LOCAL_READY`, or `MERGED_BY_POLICY`.
6. **Final review fixed point.** At run start (`STARTING`), Vela records `run_base_sha`, the
   default-branch commit the integration branch was cut from. Final review diffs
   `run_base_sha...integration_head`. If the default branch advances during the run, Vela
   reports it and leaves reconciliation to the human at promotion; it does not rebase pushed
   integration history.

## Consequences

- `PRODUCT_SPEC.md` section 2.10, `GITHUB_WORKFLOW.md`, `GIT_WORKFLOW.md`, `ORCHESTRATION_ENGINE.md`
  and `CONFIGURATION.md` are updated; FR-045 and AT-021 cover it.
