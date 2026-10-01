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
