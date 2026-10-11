# Prompt and Structured Output Contracts

Prompts should be versioned templates with explicit inputs/outputs.

## Dependency analyst output

-   ticket_id
-   explicit_blockers
-   inferred_dependencies\[\]
-   predicted_write_set\[\]
-   contract_impacts\[\]
-   risk
-   notes

## Worker completion output

-   status
-   base_sha
-   head_sha
-   commits\[\]
-   tests\[\]
-   self_audit\[\] (each acceptance criterion with its evidence)
-   review_cycles\[\]
-   unresolved_findings\[\]
-   warnings\[\]
-   discovered_dependencies\[\]

## Merger output (conflict-resolution attempt only; see ADR-011)

-   worker_head
-   integration_before
-   integration_after
-   conflicts
-   integration_tests
-   result

Schema validation failure is not silently accepted as success. Retry
with correction or escalate.

# Runtime Scope in Prompts

Production orchestration prompts/contracts should refer to the abstract worker contract where possible, but v1 acceptance and operational instructions must be concrete enough to validate Antigravity. Do not add Claude/Codex runtime contracts without an explicitly scoped provider-adapter issue.

# Reviewer Output Contract

The authoritative reviewer (`REVIEW_PROTOCOL.md`) returns:

-   review_id
-   fixed_point_sha
-   review_head_sha
-   findings[] (each: axis (Standards | Spec), severity, category, location, evidence, requirement,
    suggested_fix)
-   axes_covered[]
-   summary

Vela maps this to the finding schema, assigns stable finding IDs, and decides blocking status.
Free-form prose cannot substitute for these fields.

# Verifier Output Contract

The independent verifier (`REVIEW_PROTOCOL.md`, ADR-022) receives the blocking findings of the comprehensive review
(with their ids), the `fixed_point_sha`, the `review_head_sha` and the `fix_head_sha`, and the regression test named
for each fix. It is told to verify the fixes and the fix diff only, not to re-review the change. It returns:

-   review_id, stage (`VERIFICATION`)
-   fixed_point_sha, review_head_sha, fix_head_sha
-   verdicts\[\] (each: finding id, `RESOLVED` | `NOT_RESOLVED`, evidence, regression test checked)
-   findings\[\] in the fix diff (same fields as the reviewer contract)
-   observations\[\] outside the fix diff (same fields; high severity blocks, the rest are follow-ups)
-   axes_covered\[\], summary

Vela decides blocking status from its policy; a missing verdict for a blocking finding is `NOT_RESOLVED`.

# Conflict-Resolution Attempt Contract

The merger contract above describes the **conflict-resolution attempt** only (ADR-011): integration
merges themselves are deterministic Vela code, not an agent. The attempt returns: worker_head,
integration_tip_merged, conflicts_encountered[], resolution_summary, uncertainty (none | low | high),
tests, result. Any `uncertainty` above none, or any high-risk surface touched, escalates to
`NEEDS_HUMAN`.

# Recovery Envelope

A recovery-resume session (`RECOVERY.md`) receives: run ID, ticket, `fixed_point_sha`, last verified
checkpoint SHA, branch/worktree, completed steps summary from the journal, and the standard
prohibited actions. It never receives previous conversation state as authority.

# Analyst Execution

The dependency analyst (`PARALLELIZATION.md`) runs read-only during run state `ANALYZING`. Its output
is validated and deterministic hard blockers always apply.

# Note on the Reviewer Contract (2026-10-02)

The reviewer output contract above is **Vela's wrapper contract**, not the native output of the
`/code-review` skill, which emits no severity and no structured fields. Vela obtains the fields by
instructing the reviewer session (optionally enforcing a schema with the CLI's `--json-schema`) or by
classifying the skill's two-section output itself.
