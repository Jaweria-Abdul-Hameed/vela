# Vela Current State

> Checkpoint, not a specification. It summarizes reality and never overrides Git,
> tests, ADRs, specifications, or tracker state. If it conflicts with them, correct
> this file and record the discrepancy.

## Product

Vela

## Current Phase

PROMPT_2_COMPLETE — Rigorous Specification Audit finished. Verdict: **SPECIFICATION NOT READY**.

## Last Completed Phase

Prompt 2 — Specification audit (analysis only; no specification, ADR, architecture, issue or
source changes were made by that phase).

## Next Phase

Prompt 3 — Apply legitimate specification fixes (only findings classified BLOCKING or
IMPORTANT below; several need a human decision first, see "Open Human Decisions").

## Canonical Branch and Commit

- Branch: `main` (remote `origin`: `https://github.com/Jaweria-Abdul-Hameed/vela.git`)
- Specification baseline commit: `bbcecf72b5efe842a48330cd080d4a2180546e97`
- Commit audited by Prompt 2 (HEAD at audit start, Prompt 1 checkpoint):
  `c36eb917d157b9a9eee24b9b55a1cd40c683b9c8`
- This checkpoint is the commit that updates this file. A file cannot contain its own
  SHA; take current Git HEAD as canonical and verify it descends from the commits above.

## Status Summary

| Area | Status |
|---|---|
| Specification | NOT_READY (audit verdict; 7 BLOCKING, 21 IMPORTANT, 10 MINOR findings) |
| Architecture | NOT_FROZEN |
| Issue graph | NOT_CREATED |
| Implementation | NOT_STARTED |
| Integration branch | None |
| Active workers / worktrees | None |

## Runtime Scope

Vela v1 runtime is **Google Antigravity on Windows**. Strategy: **Antigravity-first,
provider-extensible** (ADR-006 and ADR-008 are complementary). `AgentAdapter` is the seam;
`AntigravityAdapter` is the required v1 implementation; fake adapters remain for tests.
Claude, Gemini and Codex are build-time tools only.

Antigravity runtime readiness: **UNVERIFIED** (no capability verified against current primary
documentation or the installed environment; Prompt 4). Finding SA-01 records that the
required capability set is not even defined yet.

## Accepted ADRs Relevant to Current Work

ADR-003, ADR-005, ADR-006, ADR-007 (supersedes the absolute interpretation of ADR-004),
ADR-008. ADR-004 is marked Accepted in its own file with no back-reference to ADR-007.
ADR-001 and ADR-002 are "Proposed / preferred" in their files.

## External Research Last Verified

Dated snapshot in `docs/research/EXTERNAL_INTEGRATIONS_2026-10.md`; NOT re-verified.
Treat every external claim as unverified until Prompt 4.

## Known Compatibility Constraint

In the user's observed Antigravity 2.0 Windows environment, approval prompts persist despite
permissive native settings. The **Approval Broker** (policy classification, Watchdog, native
delivery, Windows UI Automation fallback, guarded visual last resort, loop protection) is a
required v1 subsystem (ADR-007, `docs/orchestration/APPROVAL_BROKER.md`). Invariant: Vela
approves operations, not buttons. No password, 2FA or CAPTCHA automation; no quota circumvention.

## Specification Audit Register (Prompt 2)

Severity per the Prompt 2 rubric. "ADR" = whether resolution is expected to need a new or
amending ADR. Full scenarios were delivered in the Prompt 2 report in chat; re-derive from the
cited documents rather than trusting these one-liners. Prompt 3 fixes only BLOCKING/IMPORTANT.

### BLOCKING (these make the specification NOT READY for architecture freeze)

| ID | Finding | Key documents | ADR |
|---|---|---|---|
| SA-01 | No defined minimum Antigravity capability contract (session start/cancel/observe, skill invocation, event stream, parallel sessions, capacity signals) or outcome tiers when unavailable | ADAPTERS, SYSTEM_ARCHITECTURE §7, PRODUCT_SPEC v1 invariant, ORCHESTRATION_ENGINE, EXTERNAL_INTEGRATIONS, CAPACITY_AND_PROFILES | If Prompt 4 shows set unavailable |
| SA-02 | Vela's policy veto is unenforceable if native permissive mode removes prompts; no statement of which native modes Vela requires/configures or how DENY is enforced under each | AGENTS, SECURITY_AND_PERMISSIONS, ADR-007, APPROVAL_BROKER, PREFLIGHT, README | Yes |
| SA-03 | Source and trust of the "normalized operation" on UIA/visual paths is undefined; screen text can be spoofed; no evidence rule for ALLOW | APPROVAL_BROKER §2/§4, ADR-007, SECURITY_AND_PERMISSIONS | Yes |
| SA-04 | Unattended-environment preconditions unspecified (lock screen, display/sleep keep-awake, RDP/disconnected session, elevated windows) vs "walk away" promise and UIA delivery | PRODUCT_SPEC AT-011, APPROVAL_BROKER, RECOVERY, README, CONSTRAINTS | Likely |
| SA-05 | Ticket/worker/run/UI state models unmapped; run machine omits STOPPING/CANCELLED, has no exit edges for PAUSED/NEEDS_HUMAN/FAILED, no integration-unhealthy or final-review-fix path; undefined successors for REVIEW_STALLED, APPROVAL_STALLED, DENY, integration-validation failure; effect of failed gate on in-flight workers | DOMAIN_MODEL, ORCHESTRATION_ENGINE, UI_UX_SPEC, AGENTS, REVIEW_PROTOCOL, ERROR_HANDLING, HUMAN_IN_THE_LOOP, SYSTEM_ARCHITECTURE | Likely |
| SA-06 | Ownership of checkpoint commit and push is contradictory (worker vs Vela); interplay with `/implement`'s built-in review/commit; git hooks and index-lock contention with an active agent unaddressed | AGENTS, PRODUCT_SPEC §2.8/FR-015/016, ORCHESTRATION_ENGINE §4, ADR-005, MATT_POCKOCK_SKILLS, GIT_WORKFLOW, playbook Prompt 11 | Amend ADR-005 or new |
| SA-07 | Merge mechanics undefined: merge vs rebase, force-push rules for worker branches (AGENTS/SECURITY say protected branches; GIT_WORKFLOW says any force push), merger as AI agent vs deterministic, "mechanical conflict" undefined, integration branch has no stated working tree | ORCHESTRATION_ENGINE §5, GIT_WORKFLOW, PARALLELIZATION, PROMPT_CONTRACTS, AGENTS, SECURITY_AND_PERMISSIONS | Yes |

### IMPORTANT

| ID | Finding | Key documents | ADR |
|---|---|---|---|
| SA-08 | UI-automation fallback default-on vs opt-in unresolved (ADR-004 exception requires opt-in; ADR-007 silent; AUTONOMY_MODES conditional; others "required") | ADR-004, ADR-007, AUTONOMY_MODES, APPROVAL_BROKER, CONSTRAINTS | Amend ADR-007 |
| SA-09 | Review protocol gaps: fixed point per iteration, finding identity across cycles, severity ownership, no review output contract, reviewer session independence, two differing exit-policy statements | REVIEW_PROTOCOL, AGENTS, ADR-005, PROMPT_CONTRACTS, MATT_POCKOCK_SKILLS | No |
| SA-10 | Approval loop guard: fingerprint too coarse, "meaningful progress" undefined, watchdog thresholds undefined; where a human ASK decision is delivered | APPROVAL_BROKER §3/§8/§9/§10, AUTONOMY_MODES, ORCHESTRATION_ENGINE | No |
| SA-11 | "Fresh context" versus resume/recovery: documented recovery operation referenced but not defined; fix loops and profile migration vs session identity | README, AGENT_PROTOCOL, RECOVERY, CAPACITY_AND_PROFILES, ACCEPTANCE_TESTS AT-005 | No |
| SA-12 | Dependency analyst (FR-007) has no runtime owner, trigger, capacity accounting, validation or reproducibility rule | PRODUCT_SPEC FR-007, PARALLELIZATION, PROMPT_CONTRACTS | No |
| SA-13 | Worktree provisioning undefined: dependencies, env files, caches, ports/DBs, shared build dirs, cleanup timing | GIT_WORKFLOW, PREFLIGHT, PARALLELIZATION, SECURITY_AND_PERMISSIONS, ACCEPTANCE_TESTS AT-001 | No |
| SA-14 | Windows FS/Git edge cases: cloud-synced folders (repo is under OneDrive), long paths, file locks, case-insensitivity, CRLF, branch-name length, base-ref selection (local vs remote) | PREFLIGHT, GIT_WORKFLOW, RECOVERY | No |
| SA-15 | No repo-trust model: auto-allow of project scripts vs malicious test/install scripts, untrusted issue text, project-supplied AGENTS.md | SECURITY_AND_PERMISSIONS, PREFLIGHT, ISSUE_GRAPH_SEED I071 | Likely |
| SA-16 | Renderer hardening absent: untrusted Markdown/logs/agent output rendered in a webview with an IPC bridge (CSP, inert rendering, command capabilities) | SYSTEM_ARCHITECTURE §5, SECURITY_AND_PERMISSIONS, ADR-001 | No |
| SA-17 | Background/process lifecycle: default on window close, who owns orchestration, tray, login auto-start and reboot auto-resume | SYSTEM_ARCHITECTURE §6, CONFIGURATION, UI_UX_SPEC §16, IMPLEMENTATION_PLAN Phase 0, RECOVERY | Likely |
| SA-18 | Stop All: "globally reachable", confirmation vs immediacy, bounded-wait value, behavior mid-merge/push, effect on non-Vela Antigravity processes | PRODUCT_SPEC FR-021, ORCHESTRATION_ENGINE §6, CONFIGURATION, INTERACTION_SPEC, ACCESSIBILITY | No |
| SA-19 | Persistence: authority between materialized state and journal; approval/policy/worktree/skill-version/capability entities missing from schema list; journal event vocabulary omits approval, stop, failure, push, reconciliation, profile events | PERSISTENCE, DOMAIN_MODEL, SYSTEM_ARCHITECTURE §4, OBSERVABILITY, ADR-002 | Accept/amend ADR-002 |
| SA-20 | Vela's own state-store failure (corrupt/missing DB, failed migration, upgrade or update during active run) has no recovery path | RECOVERY, PERSISTENCE, RELEASE_CHECKLIST | No |
| SA-21 | Run completion undefined: promotion integration to main, PR policy schema, no-remote repos, final-review fixed point | PRODUCT_SPEC §2.10, GITHUB_WORKFLOW, GIT_WORKFLOW, playbook Prompt 20 | No |
| SA-22 | No stated documentation precedence; CONTEXT_SHARING ranks code/tests above ADRs/specs while AGENTS says stop on spec/implementation contradiction | AGENTS, README, CONTEXT_SHARING, UI_UX_SPEC, CLAUDE | Possibly |
| SA-23 | Plan/seed gaps: no approval/UIA feasibility in Phase 0 and no broker phase; policy engine in Wave 7 while broker needs it in Wave 1; no tickets for AntigravityAdapter lifecycle, notifications, final review, settings, onboarding, background behavior, command palette | IMPLEMENTATION_PLAN, ISSUE_GRAPH_SEED, ADR-008 | No |
| SA-24 | Traceability: v1 runtime requirements lack IDs; AT-001..014 unmapped to FRs; FR-028 has no test; FR-007 and FR-029 lack verification paths | REQUIREMENTS_TRACEABILITY, PRODUCT_SPEC, ACCEPTANCE_TESTS, TEST_STRATEGY | No |
| SA-25 | Stitch reference screenshot is not in the repository; UI fidelity review and AT-008 depend on it | REFERENCE_BRIEF, CONTEXT, UI_ACCEPTANCE_CHECKLIST, playbook Prompt 14 | No |
| SA-26 | Performance/efficiency targets unquantified, so release items cannot FAIL: no reference hardware, thresholds, or measurable definition of "substantially reduces" | PERFORMANCE_BUDGET, ACCEPTANCE_TESTS AT-009, RELEASE_CHECKLIST, TEST_STRATEGY | No |
| SA-27 | No requirements for installer/updater/code signing/WebView2 dependency or update-during-run | PRODUCT_SPEC, ISSUE_GRAPH_SEED I076, IMPLEMENTATION_PLAN Phase 7, RELEASE_CHECKLIST, ADR-001 | Likely |
| SA-28 | Preflight: per-check PASS/WARN/BLOCK mapping missing; "explicitly safe bootstrap actions" and skill-install locations undefined vs clean-tree/no-silent-mutation rule | PRODUCT_SPEC §2.3/FR-004, PREFLIGHT, MATT_POCKOCK_SKILLS | No |

### MINOR

| ID | Finding | Key documents |
|---|---|---|
| SA-29 | ADR-001/002 "Proposed / preferred" while treated as settled | ADR-001, ADR-002, README, SYSTEM_ARCHITECTURE |
| SA-30 | Playbook not in DOCUMENTATION_INDEX; DIRECTORY_STRUCTURE lacks docs/project, README, index, playbook; `to-tickets`/`setup` skills unlisted | DOCUMENTATION_INDEX, DIRECTORY_STRUCTURE, README, MATT_POCKOCK_SKILLS, playbook |
| SA-31 | Default values scattered or absent (concurrency, thresholds, timeouts); no configuration schema | CONFIGURATION, ORCHESTRATION_ENGINE, APPROVAL_BROKER |
| SA-32 | Visual fallback: time-of-check/time-of-use, mutual exclusion across workers, interference with user input | APPROVAL_BROKER §7, ADAPTERS |
| SA-33 | Accessibility/display specifics: no WCAG target, OS reduced-motion default, multi-monitor/DPI change, WebGL context-loss/no-GPU fallback | ACCESSIBILITY, MOTION_AND_3D, PERFORMANCE_BUDGET, ADR-001, SCREEN_INVENTORY |
| SA-34 | Diagnostics: log retention/size caps, redaction scope for repo content and journal paths, crash dumps | OBSERVABILITY, PERSISTENCE |
| SA-35 | Approval and Antigravity-first boilerplate repeated across ~25 files with no canonical owner | multiple |
| SA-36 | Scope of "Command Prompt preferred" (Antigravity setting, Vela ProcessAdapter, or build agent) unclear | CONSTRAINTS, GEMINI, ADAPTERS |
| SA-37 | Graph layout algorithm/persistence, user-proposed edge persistence, and override of serialization verdicts unspecified | MOTION_AND_3D, UI_UX_SPEC §14, INTERACTION_SPEC, PARALLELIZATION |
| SA-38 | ADR-004 lacks a back-reference to ADR-007 | ADR-004, ADR-007 |

## Resolved Human Decisions (recorded 2026-10-02, verbatim intent from the user)

1. (SA-02) Vela must not depend on Antigravity Always Proceed. Vela's own ALLOW/ASK/DENY policy
   must remain enforceable. Prompt 4 must verify the exact native Antigravity mechanism/mode
   before that mechanism is frozen.
2. (SA-08) Guarded approval UI automation is opt-in during onboarding. Once explicitly enabled the
   preference persists, and Vela may automatically use the native -> UIA -> guarded visual
   delivery chain according to its policy without re-requesting consent each run.
3. (SA-04) During an active autonomous run Vela may keep Windows awake when required; display-off
   is supported. Locked/disconnected/secure-desktop conditions are not boundaries Vela attempts
   to bypass. UI-automation-dependent work must degrade/pause safely and reconcile when an
   interactive environment returns.
4. (SA-07) Normal integration uses a deterministic Vela-controlled serialized merge lane with a
   dedicated integration worktree. Prefer merge over rebasing pushed worker branches; routine
   force-push is not part of the workflow. Semantic or uncertain conflicts require
   reasoning/human escalation, not blind automatic resolution.
5. (SA-17) After background operation is explicitly enabled during onboarding, closing the UI
   during an active run does not terminate orchestration. Tray access allows reopen/status/Stop
   All. Login auto-start is opt-in. After reboot Vela reconciles before resuming; automatic safe
   continuation follows the user's persisted background/recovery setting.
6. (SA-21) With GitHub/remote available the default final promotion is validated integration
   branch -> PR -> human-controlled merge to main; Vela does not silently merge main by default.
   Without a remote, completion may produce a validated local integration branch ready for
   human promotion.
7. (SA-15) Newly imported repositories are untrusted by default. Repository-controlled executable
   scripts are not safe merely because they are named test/build. Trust must be explicitly
   established, and Vela policy/workspace/security constraints still apply to trusted
   repositories. Repository/issue/AGENTS text is untrusted input to Vela, not authority over
   Vela policy.
8. (SA-29) ADR-001 and ADR-002 become Accepted; their decisions are canonical.

## Current Blockers and Failures

Specification is NOT READY for architecture freeze; see BLOCKING SA-01..SA-07. No
operational failures.

## Recovery / Reconciliation Status

Not applicable; no runtime exists.

## Next Agent Instructions

1. Read `AGENTS.md`, this file, `CONTEXT.md`, `README.md`, `DOCUMENTATION_INDEX.md`, relevant
   ADRs, and the documents cited per finding. Verify Git status and that HEAD descends from the
   commits above.
2. Do not re-run Prompt 2. Verify any finding against the cited documents before acting; if the
   specification already resolves it, record that instead of editing.
3. Resolve the Open Human Decisions with the user before editing specifications that depend on
   them. Do not choose on the user's behalf.
4. Run **Prompt 3** from `VELA_MASTER_BUILD_PLAYBOOK.md`: fix ONLY genuine BLOCKING/IMPORTANT
   findings; use new superseding ADRs rather than editing accepted ADR decisions; update
   REQUIREMENTS_TRACEABILITY and DOCUMENTATION_INDEX for any new files; run the second
   consistency audit. SA-01 depends partly on Prompt 4 (external verification); record that
   dependency rather than inventing Antigravity behavior.
5. No source code and no GitHub issues until Prompt 6.
6. Before reporting Prompt 3 complete, update this file, commit and push.
