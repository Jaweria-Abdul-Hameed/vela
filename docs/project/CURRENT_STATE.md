# Vela Current State

> Checkpoint, not a specification. It summarizes reality and never overrides Git,
> tests, ADRs, specifications, or tracker state. If it conflicts with them, correct
> this file and record the discrepancy.

## Product

Vela

## Current Phase

PROMPT_1_COMPLETE — Complete Project Comprehension accepted.

## Last Completed Phase

Prompt 1 — Complete Project Comprehension (read-only; no files modified by that phase).

## Next Phase

Prompt 2 — Rigorous Specification Audit.

## Canonical Branch and Commit

- Branch: `main` (remote `origin`: `https://github.com/Jaweria-Abdul-Hameed/vela.git`)
- Specification baseline commit audited by Prompt 1: `bbcecf72b5efe842a48330cd080d4a2180546e97`
  (`docs: establish Vela product specification`)
- This checkpoint is the commit that adds this file on top of the baseline. A file
  cannot contain its own commit SHA; the next agent must take the current Git HEAD
  as canonical and verify that it descends from the baseline above.

## Status Summary

| Area | Status |
|---|---|
| Specification | UNDER_AUDIT (not READY: unresolved blocking and important gaps from Prompt 1) |
| Architecture | NOT_FROZEN |
| Issue graph | NOT_CREATED |
| Implementation | NOT_STARTED |
| Integration branch | None (no implementation run exists) |
| Active workers / worktrees | None |
| Open human decisions | None formally opened; see "Known Blockers and Risks" |

## Runtime Scope

Vela v1 runtime is **Google Antigravity on Windows**. Strategy: **Antigravity-first,
provider-extensible** (ADR-006 and ADR-008 are complementary).

- `AgentAdapter` is the architectural seam; `AntigravityAdapter` is the required v1
  implementation; fake/test adapters remain for deterministic tests.
- Claude, Gemini, Codex and others are build-time tools only. They are not v1 runtime
  targets unless an approved specification change says otherwise.
- Antigravity runtime readiness: **UNVERIFIED**. No Antigravity capability has been
  verified against current primary documentation or the installed environment
  (that is Prompt 4's job).

## Accepted ADRs Relevant to Current Work

- ADR-003 (worktrees), ADR-005 (review after checkpoint), ADR-006 (provider-independent
  core), ADR-007 (guarded approval fallback; supersedes the absolute interpretation of
  ADR-004), ADR-008 (Antigravity-first, provider-extensible).
- ADR-004: marked Accepted in its own file; its absolute interpretation is superseded
  by ADR-007.
- ADR-001 (Tauri) and ADR-002 (SQLite journal): status "Proposed / preferred" in their
  files.

## External Research Last Verified

Snapshot dated 2026-10 in `docs/research/EXTERNAL_INTEGRATIONS_2026-10.md`. It has NOT
been re-verified against current primary sources. Treat every external claim
(Antigravity modes, permission engine, Matt Pocock skills) as unverified until Prompt 4.

## Known Compatibility Constraint

In the user's observed Antigravity 2.0 Windows environment, approval prompts persist
despite permissive native settings. Native Antigravity permissions are preferred but are
not assumed sufficient. The **Approval Broker** (policy classification, Approval
Watchdog, native delivery, Windows UI Automation fallback, guarded visual last resort,
loop protection) is a required v1 subsystem (ADR-007,
`docs/orchestration/APPROVAL_BROKER.md`). Invariant: Vela approves operations, not
buttons. No password, 2FA or CAPTCHA automation, and no quota circumvention.

## Known Blockers and Risks (from Prompt 1; unresolved and not yet audited or classified)

These are observations to be examined by Prompt 2, not decisions. Do not treat them as
settled findings or as fixes.

1. **Authority/precedence.** No single stated precedence among AGENTS.md, README,
   PRODUCT_SPEC, ADRs and CONTEXT_SHARING; ADR-004 carries no supersession marker;
   ADR-001/002 are "Proposed" while treated as settled; opt-in versus default-on status
   of UI automation is unclear (ADR-004, ADR-007, AUTONOMY_MODES, APPROVAL_BROKER).
2. **State models.** Ticket (DOMAIN_MODEL), worker (ORCHESTRATION_ENGINE) and UI node
   (UI_UX_SPEC) state vocabularies are unmapped; the run state machine omits `STOPPING`
   and has undefined exit edges; `REVIEW_STALLED`, `NEEDS_HUMAN` and `APPROVAL_STALLED`
   successors are undefined; DENY outcome is undefined; the effect of an integration
   gate failure on in-flight workers is undefined.
3. **Approval/policy/security.** Vela's policy veto may be unenforceable if native
   permissive mode removes prompts; operation normalization may rely on untrusted screen
   text; the policy engine has no schema and is scheduled late (ISSUE_GRAPH_SEED I070 vs
   I016); auto-allow of project scripts conflicts with the malicious-test-script threat;
   the loop fingerprint and "progress" are undefined; unattended UIA preconditions
   (lock screen, elevation, accessibility tree) are unspecified.
4. **Git/review/merge ownership.** Who pushes (AGENTS vs PRODUCT_SPEC); who creates
   checkpoint commits and how that interacts with `/implement`'s own review and commit;
   fixed point for re-review and final review; rebase vs merge (force-push is a protected
   action); whether the merger is an AI agent; the integration branch's working tree;
   integration-to-main promotion.
5. **Antigravity and Windows runtime.** The programmatic control model (CLI, SDK, daemon,
   sessions, skill invocation, parallel sessions) is unverified; the user's own manual
   Antigravity instance versus Vela-managed ones; background, tray, reboot and Stop All
   semantics; Windows Git/worktree/process specifics, including cloud-synced folders (the
   repo currently sits under OneDrive).
6. **Persistence/journal.** Approval entities, policy rules and several records are absent
   from PERSISTENCE; the journal event list lacks approval, stop and failure events;
   authority between materialized state and journal is undefined.
7. **UI.** The Stitch reference image is not in the repository; rendering technology,
   graph layout algorithm and WebGL fallback are unspecified; performance targets are
   unquantified; some approval-related UI surfaces are missing from inventories.
8. **Planning/traceability.** IMPLEMENTATION_PLAN Phase 0 has no approval/UIA
   feasibility prototype and no broker phase; the issue-graph seed lacks tickets for
   several areas (AntigravityAdapter lifecycle, notifications, final review, settings,
   onboarding, background behavior); v1 runtime requirements have no FR IDs; default
   parameters are unspecified; `VELA_MASTER_BUILD_PLAYBOOK.md` is absent from
   DOCUMENTATION_INDEX and DIRECTORY_STRUCTURE, and `docs/project/` is absent from
   DIRECTORY_STRUCTURE (this file is now listed in DOCUMENTATION_INDEX only).
9. **Duplication.** The approval requirement and the Antigravity-first paragraph are
   repeated across many files; the review exit policy is stated in two places with
   slightly different wording.

## Current Blockers and Failures

None operational. The specification is not ready for architecture freeze until Prompt 2
audits these areas and Prompt 3 resolves genuine gaps.

## Recovery / Reconciliation Status

Not applicable; no runtime exists.

## Next Agent Instructions

1. Read `AGENTS.md`, this file, `CONTEXT.md`, `README.md`, `DOCUMENTATION_INDEX.md`, the
   relevant ADRs, then the documents needed for the audit. Verify Git status and that
   HEAD descends from the baseline commit above.
2. Run **Prompt 2 — Specification audit** from `VELA_MASTER_BUILD_PLAYBOOK.md` exactly as
   written. Re-read source documents; do not rely on the Prompt 1 summary.
3. Treat the list above as unverified leads only. Do not assume they are real findings,
   and do not pre-fix them. Where the existing specification already resolves a point,
   cite that resolution and classify it NOT ACTUALLY A PROBLEM.
4. Do not modify specifications, ADRs, architecture, or issues during Prompt 2. Fixes
   belong to Prompt 3 and only for findings classified BLOCKING or IMPORTANT.
5. Do not write source code or create GitHub issues.
6. Before reporting Prompt 2 complete, update this file with the phase result and the
   audit verdict (SPECIFICATION READY FOR ARCHITECTURE FREEZE or SPECIFICATION NOT
   READY), commit and push.
