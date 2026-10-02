# Vela Current State

> Checkpoint, not a specification. It summarizes reality and never overrides Git,
> tests, ADRs, specifications, or tracker state. If it conflicts with them, correct
> this file and record the discrepancy.

## Product

Vela

## Current Phase

PROMPT_4_COMPLETE — External assumptions verified against current primary sources (2026-10-02).
Decision gate open before Prompt 5.

## Last Completed Phase

Prompt 4 — Verify volatile external assumptions (research documents and state only; no
specification, ADR, architecture, issue, or source changes).

## Next Phase

**Decision gate, then Prompt 5.** The user must answer DR-1 and DR-3 below (DR-2 and DR-4 are
specification or spike work). Then run a limited specification-update pass for the queued items in
`docs/research/EXTERNAL_VERIFICATION_2026-10-02.md` section L, then Prompt 5 (architecture freeze).
Do not freeze the `AntigravityAdapter` before DR-1 is decided.

## Canonical Branch and Commit

- Branch: `main` (remote `origin`: `https://github.com/Jaweria-Abdul-Hameed/vela.git`)
- Specification baseline: `bbcecf72b5efe842a48330cd080d4a2180546e97`
- Prompt 3 specification fixes: `2d9a8fd8ccb468d6c12bfc9caad61139b7b87a4c`
- This checkpoint is the commit that updates this file; a file cannot contain its own SHA. Take
  current Git HEAD as canonical and verify it descends from the commits above.

## Status Summary

| Area | Status |
|---|---|
| Specification | FIXED_AFTER_AUDIT; external facts now verified; queued updates pending decisions (see below) |
| External verification | DONE 2026-10-02; probes P1..P10 still required in the real environment |
| Architecture | NOT_FROZEN |
| Issue graph | NOT_CREATED |
| Implementation | NOT_STARTED |
| Open human decisions | DR-1, DR-3 (and carry-over: Stitch reference image) |

## Runtime Scope

Vela v1 runtime is **Google Antigravity on Windows**; strategy **Antigravity-first,
provider-extensible**. Claude, Gemini and Codex are build-time tools only.

## External Integration Readiness (Prompt 4 conclusions; evidence in `docs/research/EXTERNAL_VERIFICATION_2026-10-02.md`)

**Verdict: the specification is internally consistent but NOT READY to freeze the Antigravity
adapter**, because current reality has three programmatic/UI surfaces and the spec assumes only one.

1. **Antigravity 2.0 has three surfaces**: the Desktop app (v2.19.1 on 2026-09-30; no documented
   programmatic API), the `agy` CLI with a documented headless mode (`-p`, `stream-json`,
   `--conversation`/`--continue`, `--json-schema`, hooks, skills; v1.2.12), and a Python SDK (Research
   Preview, Apache-2.0, API-key or Vertex authentication, per-tool approval callbacks). The spec's UI
   Automation fallback assumes the Desktop GUI.
2. **Native policy mechanisms exist and are documented**: allow/deny/ask rules with Deny > Ask > Allow
   precedence, and `PreToolUse` hooks (command handlers receiving tool name/args, conversation id, and
   workspace paths; decisions allow, deny, ask, force_ask, deny_unless_prior_grant). These are the
   candidate mechanisms for the Native Permission Posture (ADR-009). **They are not proven**:
   hook `allow` does not satisfy the grant gate in headless mode (CLI issue #1053), hook `ask`
   overrides are ignored (#1059), headless runs ignore `permissions.allow` and can hang (#548), and the
   docs do not say whether hooks fail open or closed.
3. **The observed-environment requirement stands**: documentation says permissive modes remove prompts;
   open bugs and the user's own observation say prompts or soft-denials persist. Vela stays defensive and
   capability-detected; the Approval Broker remains required.
4. **Documentation conflicts**: Windows sandbox support (docs say no; changelog v2.15.1, 2026-09-19, says
   file and network sandboxing on Windows are supported); skill install locations (docs versus the
   `npx skills` installer); the headless soft-deny documentation versus the reported hangs.
5. **Matt Pocock skills**: `implement`, `implement-spec`, `code-review` and `to-tickets` exist.
   `implement` commits after its own `/code-review`; `code-review` requires a fixed point, uses
   `git diff <fp>...HEAD`, verifies a non-empty diff, and emits `## Standards`/`## Spec` text with **no
   severity and no structured output** (the reviewer-severity assumption in `REVIEW_PROTOCOL.md` is
   unsupported). The setup skill edits `CLAUDE.md`/`AGENTS.md` and writes `docs/agents/*.md`.
6. **Consequence of merge-based updates (ADR-011)**: `git diff <fp>...HEAD` includes sibling tickets'
   changes once the integration tip is merged into a worker branch, so a review after a conflict-
   resolution attempt needs an explicit fixed point.
7. **Account policy**: Google prohibits third-party tools from using Antigravity/Gemini CLI OAuth to
   reach backend services (maintainer statement 2026-02-27). A forum answer says running the official
   `agy` binary as a child process with cached credentials is supported (official standing of that
   answer unconfirmed). Vela must never read or reuse agent OAuth tokens.
8. **Platform facts confirmed**: Git branch exclusivity, shared `refs/stash` (ADR-003 caveat holds),
   per-worktree HEAD/index; Tauri 2.12 with tray, autostart, signed updater (app exits on Windows
   install), NSIS/MSI installers and WebView2 modes; UI Automation cannot reach elevated or logon-screen
   UI without UIAccess (not appropriate for Vela); `SetThreadExecutionState` keeps the system awake but
   cannot prevent user-initiated sleep or lid close. These support ADR-012. Not documented: Tauri
   prevent-exit with a tray, and whether display-off leads to a locked session.
9. **Workspace trust**: the CLI requires each new workspace to be trusted (recorded in user-global
   settings); every Vela worktree is a new workspace path. Headless behavior is undocumented.
10. **Capacity**: quota exhaustion is human-readable text (`RESOURCE_EXHAUSTED`, "resets in ..."), and a
    headless bug can retry silently to the timeout and exit 0 (#1018). Exit code 0 is not success.

## Decision Requests (not resolved; do not guess)

- **DR-1 (blocking Prompt 5): Antigravity surface.** Which surface(s) does `AntigravityAdapter` drive in
  v1: (a) CLI headless with `stream-json`, hooks, and conversation resume; (b) the Desktop GUI via UI
  Automation (the current spec assumption; no documented API); (c) the SDK (API-key/Vertex billing,
  Research Preview); or a combination. Also record which surface showed the persistent approval prompts.
- **DR-3: Account and credential policy.** Official binary with the user's cached account credentials
  versus API-key/Vertex; confirmation of Google's stance for external orchestration.
- DR-2 (specification/spike, not a user decision): prove or reject hooks and native rules as the NPP
  mechanism through probes P1-P3.
- DR-4 (specification): obtain review severity and structure through Vela's own prompt/`--json-schema`
  or classify in Vela.
- Carry-over: the Stitch reference image is still missing from `docs/ui/reference/`.

## Queued Specification Updates (apply after DR-1/DR-3; list in verification record section L)

Surface decision into `ADAPTERS.md`, ADR-009/ADR-010 scope, `APPROVAL_BROKER.md`; NPP mechanism candidates;
review contract (`REVIEW_PROTOCOL.md`, `PROMPT_CONTRACTS.md`); explicit fixed point after merge-based
updates; per-worktree workspace trust and consent; account policy; Vela-only execution workspaces versus
Antigravity-created worktrees; preflight additions (submodules, Git version, project-level skills, WebView2
refresh); skills-setup bootstrap handling.

## Probes Required in the Real Environment (Phase 0; see record section N)

P1 headless permissions and hooks; P2 hook decision semantics and failure mode; P3 agent-writability of
hooks/settings; P4 skill invocation in print mode and in a worktree; P5 workspace trust for a new worktree;
P6 `WAITING` and cancellation; P7 Desktop approval-card UIA tree (if Desktop is chosen); P8 parallel `agy`
processes and quota; P9 Tauri prevent-exit and keep-awake under display-off/lock; P10 hardened Git set.

## Accepted ADRs

ADR-001..ADR-015 (all Accepted). No ADR was changed by Prompt 4; DR-1 may require a superseding or refining
ADR.

## External Research Last Verified

2026-10-02 (`docs/research/EXTERNAL_VERIFICATION_2026-10-02.md`). Vendor documentation changes quickly
(desktop v2.19.1 and CLI v1.2.12 were current); revalidate before architecture freeze and again before release.

## Known Compatibility Constraint

In the user's observed Antigravity 2.0 Windows environment, approval prompts persist despite permissive
native settings. The Approval Broker is required (ADR-007); Vela does not depend on unconditional native
auto-execution (ADR-009); guarded UI automation is opt-in (ADR-010). No password, 2FA or CAPTCHA
automation; no quota circumvention; no reuse of agent OAuth tokens.

## Remaining Items from Earlier Phases

Untouched MINOR findings SA-30, SA-31 (schema), SA-33..SA-37; judgment calls listed in the Prompt 3
checkpoint (`git log`, commit `2d9a8fd`) remain revisitable; performance reference hardware unrecorded.

## Recovery / Reconciliation Status

Not applicable; no runtime exists.

## Next Agent Instructions

1. Read `AGENTS.md`, this file, `DOCUMENTATION_INDEX.md` (precedence section), ADR-007 and ADR-009..015,
   and `docs/research/EXTERNAL_VERIFICATION_2026-10-02.md`. Verify Git status and that HEAD descends from the
   commits above.
2. Do not re-run Prompt 4. Do not start Prompt 5 until DR-1 is answered; ask the user.
3. After DR-1/DR-3, apply the queued specification updates (limited pass, ADRs for decisions with
   alternatives), then update this file, commit, push, and proceed to Prompt 5.
4. Treat any vendor claim as time-stamped; capability-detect at runtime; keep the observed-environment
   requirement.
5. No source code and no GitHub issues until Prompt 6.
