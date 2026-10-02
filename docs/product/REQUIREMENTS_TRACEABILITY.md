# Requirements Traceability

Every implementation issue must reference one or more requirement IDs
from `PRODUCT_SPEC.md`.

Minimum traceability fields:

  Field            Meaning
  ---------------- --------------------------------
  Requirement      `FR-###` or NFR name
  Ticket           GitHub issue ID
  Implementation   key modules/files
  Verification     automated test/manual evidence
  Status           planned/implemented/verified

A feature is not complete merely because a ticket is closed. Release
readiness requires every in-scope requirement to have verification
evidence.

Critical chains requiring explicit end-to-end verification: 1. project →
preflight → graph → build; 2. ready frontier → worktree → implementation
→ review → merge; 3. crash → restart → reconciliation → resume; 4.
unsafe parallel pair → rejected scheduling; 5. review finding → fix →
test → re-review; 6. stop-all → quiescent durable state; 7. minimized
app → rendering throttled while orchestration continues.

# Antigravity-First Traceability Rule

All requirements involving agent execution, unattended operation, approval handling, session lifecycle, runtime recovery, or agent capability detection must have a concrete Antigravity v1 implementation/test path.

A generic interface or fake adapter alone does not satisfy the corresponding v1 runtime requirement. Future-provider extensibility may be verified at interface/architecture level without shipping another provider adapter.

# Traceability Updates (Prompt 3)

-   The v1 runtime release requirements now carry requirement IDs; the mapping is in
    `PRODUCT_SPEC.md` ("V1 runtime requirement identifiers"). Issues must cite these IDs.
-   Non-functional requirements are cited by their NFR name from `PRODUCT_SPEC.md` section 4.
-   Every acceptance test AT-001..AT-026 maps to requirement IDs in `ACCEPTANCE_TESTS.md`
    ("Acceptance-test traceability").
-   Requirements without an automated or manual acceptance test are not verified. FR-007 and FR-029
    are verified by the additional mandatory scenarios in `TEST_STRATEGY.md`; FR-028 by AT-023.
-   Additional critical chains requiring explicit end-to-end verification: 8. untrusted repo ->
    trust -> build; 9. posture check -> autonomy availability; 10. approval prompt -> evidence
    binding -> policy -> delivery -> verified progress; 11. locked session -> pause -> reconcile;
    12. deterministic merge lane with conflict escalation; 13. state-store corruption -> backup or
    inventory -> no auto-resume.
