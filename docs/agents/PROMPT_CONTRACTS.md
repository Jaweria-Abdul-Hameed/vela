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
-   review_cycles\[\]
-   unresolved_findings\[\]
-   warnings\[\]
-   discovered_dependencies\[\]

## Merger output

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
