# Vela Current State

> Checkpoint, not a specification. It summarizes reality and never overrides Git,
> tests, ADRs, specifications, or tracker state. If it conflicts with them, correct
> this file and record the discrepancy.

## Product

Vela

## Current Phase

PROMPT_3_COMPLETE — Specification fixes applied; second consistency audit performed.

## Last Completed Phase

Prompt 3 — Apply legitimate specification fixes (BLOCKING and IMPORTANT findings SA-01..SA-28).

## Next Phase

Prompt 4 — Verify volatile external assumptions against CURRENT primary sources (no
implementation, no issues). Prompt 5 (architecture freeze) follows Prompt 4.

## Canonical Branch and Commit

- Branch: `main` (remote `origin`: `https://github.com/Jaweria-Abdul-Hameed/vela.git`)
- Specification baseline: `bbcecf72b5efe842a48330cd080d4a2180546e97`
- Audited by Prompt 2: `c36eb917d157b9a9eee24b9b55a1cd40c683b9c8`; audit checkpoint `4327f92`
- This checkpoint is the commit that updates this file; a file cannot contain its own SHA.
  Take current Git HEAD as canonical and verify it descends from the commits above.

## Status Summary

| Area | Status |
|---|---|
| Specification | READY_TO_FREEZE_PENDING_EXTERNAL_VERIFICATION (see "Remaining items") |
| Architecture | NOT_FROZEN (Prompt 5, after Prompt 4) |
| Issue graph | NOT_CREATED |
| Implementation | NOT_STARTED |
| Integration branch | None |
| Active workers / worktrees | None |
| Open human decisions | None (the eight decisions are resolved; see below) |

## Runtime Scope

Vela v1 runtime is **Google Antigravity on Windows**; strategy **Antigravity-first,
provider-extensible** (ADR-006 and ADR-008 complementary). `AgentAdapter` is the seam;
`AntigravityAdapter` the required v1 implementation; fakes for tests. Claude, Gemini and Codex are
build-time tools only.

Antigravity runtime readiness: **UNVERIFIED**. The required capability contract is now defined
(`ADAPTERS.md`, CAP-01..CAP-11), but whether the installed Antigravity provides each capability
is an external fact for Prompt 4 and the real-environment suite. The Native Permission Posture
is `UNKNOWN` until verified (ADR-009).

## Accepted ADRs

ADR-001 (Tauri) and ADR-002 (SQLite journal) are **Accepted** (decision 8). ADR-003, ADR-005,
ADR-006, ADR-007 (supersedes the absolute interpretation of ADR-004), ADR-008 accepted. New in
Prompt 3, all Accepted 2026-10-02: ADR-009 policy sovereignty and native permission posture;
ADR-010 guarded UI automation consent (opt-in); ADR-011 deterministic merge lane; ADR-012
background and unattended operation; ADR-013 repository trust model; ADR-014 run completion and
promotion; ADR-015 checkpoint, push, and review ownership. ADR-004 now carries a back-reference
to ADR-007 and ADR-010.

Documentation precedence is defined in `DOCUMENTATION_INDEX.md` (ADRs, then AGENTS.md, then
PRODUCT_SPEC, then subsystem specs, then README/CONTEXT, then research, then this file and the
playbook).

## External Research Last Verified

Dated snapshot in `docs/research/EXTERNAL_INTEGRATIONS_2026-10.md`; NOT re-verified. It now ends
with eight explicit verification items that the specification depends on (Prompt 4 scope).

## Known Compatibility Constraint

In the user's observed Antigravity 2.0 Windows environment, approval prompts persist despite
permissive native settings. The Approval Broker is required (ADR-007). Per decisions 1 and 2:
Vela does not depend on unconditional native auto-execution (ADR-009); guarded UI automation is
opt-in at onboarding and then persists (ADR-010). Invariant: Vela approves operations, not
buttons. No password, 2FA or CAPTCHA automation; no quota circumvention.

## Resolved Human Decisions (2026-10-02) and where they landed

1. Do not depend on Always Proceed; Vela policy stays enforceable; Prompt 4 verifies the native
   mechanism -> ADR-009, FR-038, `AUTONOMY_MODES.md`, `PREFLIGHT.md`, `SECURITY_AND_PERMISSIONS.md`.
2. UI automation opt-in at onboarding, persisted, chain native -> UIA -> guarded visual ->
   ADR-010, FR-040, `CONFIGURATION.md`, `APPROVAL_BROKER.md`.
3. Keep-awake allowed, display-off supported, no lock/secure-desktop bypass, degrade and reconcile
   -> ADR-012, FR-041, `APPROVAL_BROKER.md` section 17, `RECOVERY.md`.
4. Deterministic serialized lane, dedicated integration worktree, merge over rebase, no routine
   force-push, semantic/uncertain conflicts escalate -> ADR-011, `ORCHESTRATION_ENGINE.md`,
   `GIT_WORKFLOW.md`.
5. Background opt-in; close does not terminate; tray; login auto-start opt-in; reconcile after
   reboot; continuation per persisted setting -> ADR-012, FR-042, `CONFIGURATION.md`,
   `RECOVERY.md`.
6. Default promotion: PR with human-controlled merge; local-ready without remote -> ADR-014,
   FR-045, `GITHUB_WORKFLOW.md`.
7. Untrusted by default; explicit trust; repo text is untrusted input -> ADR-013, FR-043.
8. ADR-001 and ADR-002 Accepted.

## Prompt 2 Findings: Resolution Map

BLOCKING SA-01..SA-07 and IMPORTANT SA-08..SA-28 have been addressed in the specification:

| Findings | Where resolved |
|---|---|
| SA-01 | `ADAPTERS.md` Antigravity Required Capability Contract; FR-037; `PREFLIGHT.md`. Capability facts pending Prompt 4. |
| SA-02, SA-03, SA-08 | ADR-009, ADR-010; `APPROVAL_BROKER.md` sections 14-16; FR-038..FR-040; `AUTONOMY_MODES.md`. |
| SA-04, SA-17, SA-18 | ADR-012; FR-041, FR-042; `ORCHESTRATION_ENGINE.md` section 6; `RECOVERY.md`; `CONFIGURATION.md`. |
| SA-05 | `ORCHESTRATION_ENGINE.md` sections 1-3 (run and worker machines, integration health, intervention kinds); `DOMAIN_MODEL.md` projection and UI mapping; `UI_UX_SPEC.md`. |
| SA-06, SA-09 | ADR-015; `REVIEW_PROTOCOL.md`; `PROMPT_CONTRACTS.md`; `AGENT_PROTOCOL.md`; `AGENTS.md` pointers. |
| SA-07 | ADR-011; `GIT_WORKFLOW.md`; `PARALLELIZATION.md`. |
| SA-10 | `APPROVAL_BROKER.md` sections 8-10; `CONFIGURATION.md` defaults. |
| SA-11 | `RECOVERY.md` Resume Semantics; `AGENT_PROTOCOL.md`. |
| SA-12 | `PARALLELIZATION.md` dependency analyst; `PROMPT_CONTRACTS.md`; seed I047. |
| SA-13, SA-14 | `GIT_WORKFLOW.md` provisioning, cleanup, Windows specifics, base selection; `PREFLIGHT.md`; FR-044. |
| SA-15 | ADR-013; FR-043; `SECURITY_AND_PERMISSIONS.md`. |
| SA-16 | `SYSTEM_ARCHITECTURE.md` section 5; FR-048. |
| SA-19, SA-20 | `PERSISTENCE.md`; `SYSTEM_ARCHITECTURE.md` section 4; `RECOVERY.md`; FR-046. |
| SA-21 | ADR-014; FR-045; `GITHUB_WORKFLOW.md`; `PRODUCT_SPEC.md` section 2.10. |
| SA-22 | `DOCUMENTATION_INDEX.md` precedence; `CONTEXT_SHARING.md`; `README.md`; ADR-004 back-reference. |
| SA-23 | `IMPLEMENTATION_PLAN.md` (Phase 0 items 6-9, Phase 2b); `ISSUE_GRAPH_SEED.md` additions and ordering. |
| SA-24 | `PRODUCT_SPEC.md` FR-037..FR-048 and v1 requirement IDs; `ACCEPTANCE_TESTS.md` AT-015..AT-024 and AT-to-FR table; `TEST_STRATEGY.md`; `REQUIREMENTS_TRACEABILITY.md`. |
| SA-25 | `REFERENCE_BRIEF.md` reference asset policy. The asset itself is still missing (see remaining items). |
| SA-26 | `PERFORMANCE_BUDGET.md` initial numeric budgets; AT-009 rewritten. Reference hardware still to be recorded. |
| SA-27 | FR-047; `RECOVERY.md`; `RELEASE_CHECKLIST.md`; `IMPLEMENTATION_PLAN.md` Phase 7. |
| SA-28 | `PREFLIGHT.md` safe bootstrap actions and severity table; `MATT_POCKOCK_SKILLS.md`. |
| SA-29 | Done by decision 8. |

## Remaining Items (nothing here is an unresolved human decision)

- **Prompt 4 dependency.** Every Antigravity capability and the native posture mechanism are
  defined as requirements, not verified facts: see `docs/research/EXTERNAL_INTEGRATIONS_2026-10.md`
  "Verification Items". If a Required capability proves unavailable, that is a specification
  change decision (ADR), not an adapter workaround.
- **User action:** the Stitch reference screenshot is still not in the repository (SA-25). Add it
  under `docs/ui/reference/` and index it before UI fidelity work or review.
- **Calibration:** performance budgets are initial; reference hardware must be recorded before the
  performance pass (Prompt 18).
- **Untouched MINOR findings** (Prompt 3 fixes only BLOCKING/IMPORTANT): SA-30 (playbook and
  `docs/project` not in `DIRECTORY_STRUCTURE.md`; playbook not in the index list; `to-tickets` and
  `setup` skills unlisted), SA-31 (full config schema; a defaults table now exists), SA-33
  (WCAG target, OS reduced-motion default, multi-monitor/DPI, WebGL context-loss fallback), SA-34
  (log retention, redaction scope), SA-35 (boilerplate duplication), SA-36 (scope of "Command Prompt
  preferred"), SA-37 (graph layout/persistence, manual-edge persistence, serialization override).
  SA-32 and SA-38 were addressed incidentally.

## Judgment Calls Made in Prompt 3 (not covered by the eight decisions; revisit if undesired)

1. Authoritative review runs in a separate fresh reviewer session; fix iterations stay in the
   worker session (ADR-015, `REVIEW_PROTOCOL.md`).
2. Vela, not the worker, pushes for Vela-supervised workers; `AGENTS.md` push step is scoped to
   build-time agents (ADR-015).
3. When posture is not `MEETS`, Autonomous is blocked; Supervised is allowed only with a recorded
   acknowledgement and banner (ADR-009).
4. Decision 4 interpretation: a textual conflict goes to a reasoning-agent conflict-resolution
   attempt (re-reviewed and validated) or a human; an unpublished failed merge is discarded by a
   local reset of the clean Vela-managed integration worktree to the recorded SHA (ADR-011).
5. Untrusted repositories allow data-only analysis; Build requires trust (ADR-013).
6. `recovery_continuation` defaults to `ask`; background, auto-start and UI automation default off.
7. Initial defaults: 2 concurrent workers, 3 review iterations, approval repeat threshold 3, 120 s
   watchdog window, 30 s Stop All wait, 3 state backups (`CONFIGURATION.md`).
8. `REVIEW_STALLED` and `APPROVAL_STALLED` are `NEEDS_HUMAN` kinds, preserving their names.
9. Initial numeric performance budgets (`PERFORMANCE_BUDGET.md`).

## Second Consistency Audit (Prompt 3)

Performed: stale-term sweep (rebase, merger, Always Proceed, Proposed/preferred, state names),
ADR cross-reference check (all ADR-001..015 references resolve to existing files), documentation
index completeness check, and a review of every removed line in the diff. Result: no original
requirement was removed (FR-001..FR-036 intact; rewordings, restructurings and supersessions only);
remaining mentions of rebase/merger in `VELA_MASTER_BUILD_PLAYBOOK.md` describe the build-time
workflow and are scoped by ADR-011. This audit was performed by the same agent that made the
changes; an independent review is still advisable.

## Recovery / Reconciliation Status

Not applicable; no runtime exists.

## Next Agent Instructions

1. Read `AGENTS.md`, this file, `CONTEXT.md`, `README.md`, `DOCUMENTATION_INDEX.md` (including the
   precedence section), ADR-007 and ADR-009..015, then the documents relevant to Prompt 4. Verify
   Git status and that HEAD descends from the commits above.
2. Run **Prompt 4** from `VELA_MASTER_BUILD_PLAYBOOK.md`: verify external assumptions against
   CURRENT primary sources, not memory, with the eight verification items in the research
   document as a mandatory subset. Classify each assumption (VERIFIED, CHANGED, UNDOCUMENTED,
   UNSUPPORTED, REQUIRES RUNTIME CAPABILITY DETECTION). The user's observed persistent-prompt
   behavior remains a requirement even if documentation claims otherwise.
3. Do not invent Antigravity capabilities. If a Required capability (CAP-01..CAP-07) or the
   posture mechanism is unavailable, record it and raise a specification-change decision.
4. Do not reopen the eight resolved decisions or the judgment calls without evidence.
5. No source code and no GitHub issues until Prompt 6.
6. Before reporting Prompt 4 complete, update this file, commit and push.
