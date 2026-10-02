# Documentation Index

This index lists the complete current context pack. The approval-broker
update preserves the original pack and adds/supplements only the
affected specifications.

-   `AGENTS.md`
-   `CLAUDE.md`
-   `CODEX.md`
-   `CONTEXT.md`
-   `GEMINI.md`
-   `README.md`
-   `VELA_MASTER_BUILD_PLAYBOOK.md`
-   `docs/agents/AGENT_PROTOCOL.md`
-   `docs/agents/CONTEXT_SHARING.md`
-   `docs/agents/MATT_POCKOCK_SKILLS.md`
-   `docs/agents/PROMPT_CONTRACTS.md`
-   `docs/agents/REVIEW_PROTOCOL.md`
-   `docs/architecture/ADAPTERS.md`
-   `docs/architecture/DIRECTORY_STRUCTURE.md`
-   `docs/architecture/COMPONENT_SPECIFICATIONS.md`
-   `docs/architecture/DOMAIN_MODEL.md`
-   `docs/architecture/IMPLEMENTATION_ARCHITECTURE.md`
-   `docs/architecture/SYSTEM_ARCHITECTURE.md`
-   `docs/config/CONFIGURATION.md`
-   `docs/constraints/CONSTRAINTS.md`
-   `docs/decisions/ADR-001-DESKTOP-TAURI.md`
-   `docs/decisions/ADR-002-SQLITE-EVENT-JOURNAL.md`
-   `docs/decisions/ADR-003-WORKTREES.md`
-   `docs/decisions/ADR-004-NO-PIXEL-AUTOCLICKER.md`
-   `docs/decisions/ADR-005-REVIEW-AFTER-CHECKPOINT.md`
-   `docs/decisions/ADR-006-PROVIDER-INDEPENDENCE.md`
-   `docs/decisions/ADR-007-GUARDED-APPROVAL-FALLBACK.md`
-   `docs/git/GIT_WORKFLOW.md`
-   `docs/github/GITHUB_WORKFLOW.md`
-   `docs/issues/ISSUE_AUTHORING.md`
-   `docs/issues/ISSUE_GRAPH_SEED.md`
-   `docs/issues/graph/ISSUE_GRAPH.md`
-   `docs/issues/graph/ISSUE_GRAPH.json`
-   `docs/issues/graph/tickets/` (130 ticket bodies, one file per symbolic key, `<KEY>.md`; listed in `ISSUE_GRAPH.md`)
-   `docs/operations/AUTONOMY_MODES.md`
-   `docs/operations/ERROR_HANDLING.md`
-   `docs/operations/HUMAN_IN_THE_LOOP.md`
-   `docs/operations/OBSERVABILITY.md`
-   `docs/operations/PREFLIGHT.md`
-   `docs/orchestration/APPROVAL_BROKER.md`
-   `docs/orchestration/CAPACITY_AND_PROFILES.md`
-   `docs/orchestration/ORCHESTRATION_ENGINE.md`
-   `docs/orchestration/PARALLELIZATION.md`
-   `docs/orchestration/RECOVERY.md`
-   `docs/persistence/PERSISTENCE.md`
-   `docs/product/NON_GOALS.md`
-   `docs/product/PRODUCT_SPEC.md`
-   `docs/product/REQUIREMENTS_TRACEABILITY.md`
-   `docs/project/CURRENT_STATE.md`
-   `docs/release/RELEASE_CHECKLIST.md`
-   `docs/research/EXTERNAL_INTEGRATIONS_2026-10.md`
-   `docs/research/EXTERNAL_VERIFICATION_2026-10-02.md`
-   `docs/roadmap/IMPLEMENTATION_PLAN.md`
-   `docs/security/SECURITY_AND_PERMISSIONS.md`
-   `docs/testing/ACCEPTANCE_TESTS.md`
-   `docs/testing/TEST_STRATEGY.md`
-   `docs/ui/ACCESSIBILITY.md`
-   `docs/ui/COPY_AND_TONE.md`
-   `docs/ui/DESIGN_SYSTEM.md`
-   `docs/ui/INTERACTION_SPEC.md`
-   `docs/ui/MOTION_AND_3D.md`
-   `docs/ui/PERFORMANCE_BUDGET.md`
-   `docs/ui/REFERENCE_BRIEF.md`
-   `docs/ui/SCREEN_INVENTORY.md`
-   `docs/ui/UI_ACCEPTANCE_CHECKLIST.md`
-   `docs/ui/UI_UX_SPEC.md`
- `docs/decisions/ADR-008-ANTIGRAVITY-FIRST-PROVIDER-EXTENSIBLE.md`
-   `docs/decisions/ADR-009-POLICY-SOVEREIGNTY-AND-NATIVE-PERMISSION-POSTURE.md`
-   `docs/decisions/ADR-010-GUARDED-UI-AUTOMATION-CONSENT.md`
-   `docs/decisions/ADR-011-DETERMINISTIC-MERGE-LANE.md`
-   `docs/decisions/ADR-012-BACKGROUND-AND-UNATTENDED-OPERATION.md`
-   `docs/decisions/ADR-013-REPOSITORY-TRUST-MODEL.md`
-   `docs/decisions/ADR-014-RUN-COMPLETION-AND-PROMOTION.md`
-   `docs/decisions/ADR-015-CHECKPOINT-PUSH-AND-REVIEW-OWNERSHIP.md`
-   `docs/decisions/ADR-016-ANTIGRAVITY-SURFACE-PRIORITY-AND-AUTHENTICATION-OWNERSHIP.md`
-   `docs/decisions/ADR-017-IMPLEMENTATION-STACK-AND-PROCESS-MODEL.md`
-   `docs/decisions/ADR-018-ANTIGRAVITY-ADAPTER-AND-POLICY-ENFORCEMENT.md`
-   `docs/decisions/ADR-019-UI-RENDERING-ARCHITECTURE.md`
-   `docs/decisions/ADR-020-CONVERSATION-VISIBILITY-IN-ANTIGRAVITY-DESKTOP.md`

## Documentation precedence

When documents disagree about **intended behavior**, resolve in this order (earlier wins):

1.  **Accepted ADRs.** A later ADR that explicitly supersedes or refines an earlier one controls
    that point; the earlier ADR is preserved and carries a back-reference. Position in this index
    or in a file ("appended later") never confers precedence. Current chain: ADR-007 supersedes the
    absolute interpretation of ADR-004; ADR-009 and ADR-010 refine ADR-007; ADR-016 refines ADR-008 (surface priority and
    authentication ownership); ADR-017, ADR-018, and ADR-019 realize ADR-001/002, ADR-007/009/010/016, and the UI
    specifications respectively without changing them; ADR-020 refines ADR-016 and ADR-018 (conversation visibility in Desktop; makes the isolated-profile design provisional); ADR-008 scopes
    ADR-006 (complementary); ADR-001 and ADR-002 are Accepted.
2.  **`AGENTS.md`**: universal agent conduct rules; provider-specific files may add ergonomics but
    may not weaken it.
3.  **`docs/product/PRODUCT_SPEC.md`**: functional and non-functional requirements.
4.  **Subsystem specifications** (architecture, orchestration, agents, git, security, persistence,
    operations, testing, config). `docs/ui/UI_UX_SPEC.md` is authoritative for visual and
    interaction behavior within its domain.
5.  **`README.md` and `CONTEXT.md`**: product overview and intent; they do not override the above.
6.  **`docs/research/`**: dated external snapshots; informational, never overriding an ADR or
    specification, and to be revalidated.
7.  **`docs/project/CURRENT_STATE.md` and `VELA_MASTER_BUILD_PLAYBOOK.md`**: process and status;
    they summarize and sequence work and never override a specification.

Code and tests are the record of **actual** behavior (see `docs/agents/CONTEXT_SHARING.md`); a
contradiction between code and an accepted ADR or specification is a defect or a decision request
(`AGENTS.md` Mission), never a silent override. Contradictions between documents are surfaced and
resolved by amending the lower-precedence document or by a new ADR, not by whichever agent reads
last.
