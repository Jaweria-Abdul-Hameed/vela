# Human Intervention Protocol

An intervention card contains: - title, - exact blocking state, -
affected ticket/run, - preserved work, - last safe commit, - evidence, -
available actions, - consequences.

Examples: - choose between contradictory requirements, - authenticate
GitHub/provider, - resolve a semantic merge decision, - authorize a
destructive migration, - decide whether to accept a nonblocking review
finding after repeated oscillation.

While one worker needs human input, unrelated workers may continue only
if graph/integration safety permits.

# Intervention Kinds

Each intervention card names its `NEEDS_HUMAN` kind (`ORCHESTRATION_ENGINE.md` section 3.2):
`AMBIGUITY`, `REVIEW_STALLED`, `APPROVAL_ASK`, `APPROVAL_STALLED`, `APPROVAL_UNDELIVERABLE`,
`POLICY_DENIED`, `POLICY_VIOLATION`, `MERGE_CONFLICT`, `INTEGRATION_REGRESSION`, `AUTH_REQUIRED`,
`EXTERNAL_DIVERGENCE`, and run-level kinds `GRAPH_INVALID`, `INTEGRATION_UNKNOWN`,
`INTEGRATION_UNHEALTHY`. The card states the recorded `resume_state` so a decision returns the
run or worker to where it was. Trust decisions and onboarding choices are not interventions: they
are explicit settings actions recorded in the journal.
