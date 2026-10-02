# Vela Current State (authoritative cross-model checkpoint)

> Checkpoint, not a specification. It summarizes reality and never overrides Git, tests, ADRs, specifications, or
> tracker state. If it conflicts with them, correct this file and record the discrepancy. Any model (Claude, Gemini,
> Codex, Antigravity) can continue from the repository plus this file.

## Product

Vela: a Windows-first native desktop control plane that autonomously orchestrates coding-agent work over a dependency-aware
issue graph. **v1 runtime: Google Antigravity on Windows. Strategy: Antigravity-first, provider-extensible** (ADR-006, ADR-008).

## Current Phase

PROMPT_5_COMPLETE — **Architecture frozen** (2026-10-02).

## Last Completed Phase

Prompt 5 — Architecture freeze (documentation only: three ADRs, two architecture documents, a rewritten directory
structure, and the Prompt 4 queued specification corrections that Prompt 5 authorizes). No source code, no GitHub issues.

## Next Phase

**Prompt 6 — Generate the complete GitHub issue graph** (see "Instructions for the next model"). Do not begin it in this session.

## Canonical Branch and Commit

- Branch: `main`; remote `origin`: `https://github.com/Jaweria-Abdul-Hameed/vela.git`.
- Prior checkpoints: baseline `bbcecf7`; Prompt 3 `2d9a8fd`; Prompt 4 `1bbc38b`; Section S probes `27d2408`, `cd50b5a`, `f5dac49`.
- **This checkpoint is the commit that updates this file** (a file cannot contain its own SHA). Take current Git `HEAD` as canonical
  (`git log -1`) and verify it descends from the commits above and that `main` equals `origin/main`.

## Status Summary

| Area | Status |
|---|---|
| Specification | READY (audit findings resolved; Prompt 4/5 corrections applied; items below remain open but non-blocking) |
| External verification | Documentation verified; Section S probes done; several capabilities remain unverified (see register) |
| Architecture | **FROZEN** (`docs/architecture/IMPLEMENTATION_ARCHITECTURE.md`, `COMPONENT_SPECIFICATIONS.md`, ADR-017/018/019) |
| Issue graph | NOT_CREATED |
| Implementation | NOT_STARTED |
| Ready for ticket generation | **YES**, with the gating rules below |

## Frozen Architecture (summary; the documents are authoritative)

- **Stack:** Tauri 2.12 shell; Rust core with `tokio`; React + TypeScript (strict) renderer; `rusqlite` (bundled SQLite, WAL, single writer thread,
  embedded SQL migration ladder keyed by `user_version`); `ts-rs` generated contracts plus a command registry enforced by a contract test
  (`tauri-specta` rejected: still a release candidate); `petgraph`; the `git` binary under a hardened invocation; `gh` CLI for GitHub; `tracing` with
  one redaction layer; Vite, Tailwind with CSS-variable tokens, Motion, Zustand; React Three Fiber confined to `packages/ui/src/canvas`; Vitest and
  Playwright. Cargo workspace plus npm workspaces. **No Python sidecar. No second process** unless spike S-TRAY fails (ADR-017).
- **Layout:** `apps/desktop` (renderer + `src-tauri` composition root), `crates/` (`vela-domain`, `vela-persistence`, `vela-process`, `vela-git`,
  `vela-adapters`, `vela-uia`, `vela-hook`, `vela-core`, `vela-testkit`), `packages/` (`contracts`, `ui`, `test-fixtures`), `tools/fake-approval-window`,
  `tests/` (including the gated real-environment `antigravity-compat` suite). Dependency rules are CI-enforced (`IMPLEMENTATION_ARCHITECTURE.md` section 3).
- **Process model:** one OS process; Orchestrator actor owns all orchestration state; workers are tasks; deterministic serialized merge lane in a dedicated
  integration worktree; journal-after-commit events to the UI over one sequenced channel with snapshot resync; background operation via hide-to-tray
  with `prevent_exit` (**[U]**); Job-Object process-tree kill (**[U]** mechanism, tree kill itself **[V]**); keep-awake thread.
- **Antigravity adapter (ADR-018):** one `agy -p ... --output-format stream-json --print-timeout N` per turn with `--conversation` resume; isolated
  per-worker profile via `USERPROFILE`/`HOME`; ALLOW via generated allow rules (from the confirmed command profile and a path-scoped write rule);
  DENY/ASK via a fail-closed `vela-hook` `PreToolUse` hook; ASK resolved by a one-time allow rule plus resume (**[U]**); results interpreted from events,
  stderr, and Git state, never from exit code or `SUCCESS`. Desktop UI Automation (`vela-uia`) is the secondary, opt-in approval path; the visual tier is a
  feature-gated last resort.
- **UI (ADR-019):** DOM for all text and controls; one WebGL canvas for dots, ambient field, and graph (instanced); layout computed in Rust and stored in the
  graph snapshot; always-mounted accessible tree; Full/Balanced/Efficiency modes; WebGL-failure fallback to CSS/SVG plus the list view.
- **Every component** has inputs, outputs, state owned, dependencies, failure modes, and test strategy in `COMPONENT_SPECIFICATIONS.md`, which also maps
  **all 50 functional requirements (FR-050 added post-freeze)** to components and tests.

## Decisions and ADRs

ADR-001..ADR-020 are all Accepted (ADR-020 added post-freeze: conversation visibility in Desktop; it makes the isolated-profile design in ADR-018 provisional). Resolved human decisions: the eight Prompt 2 decisions (ADR-009..014 and ADR-001/002 acceptance), DR-1 and DR-3
(ADR-016). Architecture decisions: ADR-017 (stack, process model, contracts), ADR-018 (Antigravity adapter and policy enforcement), ADR-019 (UI rendering).
Documentation precedence is in `DOCUMENTATION_INDEX.md`.

## Verified / Partially Verified / Unverified Register

**VERIFIED in the real environment (Windows 11 Home, `agy` 1.2.14, Desktop 2.17.0, Git 2.45.1):** headless soft-denial of commands (exit 0, `SUCCESS`, stderr
only); user-level `permissions.allow` rules (exact, regex, path-scoped write) with unlisted targets denied; repo-level `.gemini/config.json` ignored;
per-process settings isolation via `USERPROFILE`/`HOME` with working authentication (no credentials read); workspace `.agents/hooks.json` loaded headless, hook
input fields, `deny` hard block, fail-closed on crash/garbage/timeout/empty, hook `allow` cannot grant; skills in print mode; no trust prompt for new worktrees
(headless); process-tree kill with no orphans (`taskkill /T`); `--conversation` resume; the agent's shell is `powershell.exe`; `--print-timeout` default `0s`;
official `agy` 1.2.14 binary checksum and Google signature; Git hardened invocation neutralizing repo-local code execution; `SetThreadExecutionState` call;
Desktop 2.17.0 approval card readable by UIA after warm-up; **one** correlated single-use allow delivered by `SelectionItemPattern` + `InvokePattern`, the agent
proceeded, and a before/after hash of `~/.gemini/config` showed no persistent permission. Antigravity's "worked for 19 minutes" was the wait on the unattended card.

**PARTIALLY VERIFIED:** policy files placed inside an allowed worktree are agent-writable; two parallel headless sessions; UIA reliability beyond that single
delivery (other versions, minimized windows, concurrent cards, other prompt variants, the refusal path); keep-awake effect on sleep; UIA on the IDE as a proxy.

**UNVERIFIED (each has a Phase 0 spike in `IMPLEMENTATION_ARCHITECTURE.md` section 14):** **CLI-created conversation visible/openable in Antigravity Desktop
(S-DESKTOP-VISIBILITY, required)**; allow rule combined with hook `allow` (S-NATIVE-POSTURE); ASK resume
after a refusal (S-ASK-RESUME); global-path hooks in an isolated profile and agent-write protection (S-HOOK-GLOBAL); `--json-schema` reviewer output
(S-SCHEMA-OUTPUT); interactive trust (S-INTERACTIVE-TRUST); `prevent_exit` with tray (S-TRAY); `windows` crate UIA bindings (S-UIA-BINDINGS); UIA reliability
(S-UIA-RELIABILITY); visual tier (S-VISUAL); Job Object kill (S-PROC-JOB); `WAITING` status, concurrency ceiling, background self-update control, interactive TUI
(S-CAP-REMAINING); Matt Pocock skills running inside Antigravity; Google's stance on external orchestration of the headless CLI (release prerequisite, ADR-016).

**UNSUPPORTED for v1:** the Antigravity SDK as a dependency (Alpha, API-key/Vertex authentication, resume/cancel undocumented).

## REQUIRED CAPABILITY ADDED AFTER THE FREEZE (do not lose): conversation visibility in Antigravity Desktop (ADR-020, FR-050, CAP-12, AT-026)

**Product requirement (user clarification, 2026-10-02):** every Vela issue/worker (and reviewer and analyst session) runs in a **fresh
Antigravity conversation**, and that conversation must be **discoverable and openable in the Antigravity 2.0 Desktop GUI** so the user can inspect
the real conversation, not only Vela's rendering. Vela still orchestrates through the supported headless `agy` surface; it does **not** need to
automate the Desktop GUI to create conversations. Prefer a CLI-created conversation that Desktop can show; if that relationship is unverified,
it is a required capability and spike, not an assumption.

- **Status: UNVERIFIED** (spike **S-DESKTOP-VISIBILITY**, seed ticket I100). No mechanism is chosen.
- **Conflict with the freeze, found and recorded:** CLI conversations are stored in `~/.gemini/antigravity-cli/` (`brain`, `conversations`,
  `annotations`, `presence`), separate from the Desktop store `~/.gemini/antigravity/conversations/`; none of the probe conversation ids from CLI runs are in
  the Desktop store. ADR-018 section 2 (isolated per-worker `USERPROFILE`) would relocate conversations away from the user's profile, so **that design is
  provisional** and stands only if the spike shows visibility, or a verified mechanism restores it. Alternatives (real profile, a Desktop-read config location,
  another store) are not chosen; any that touches the user's global Antigravity settings needs a new decision and explicit consent.
- **Observed but not enough:** the CLI created a `CLI Project` entry the user saw in Desktop; whether it lists CLI conversations and whether they open is unobserved.
  The spike needs a user-assisted observation in the Desktop app for conversations from (i) the real profile and (ii) an isolated profile.
- **Obligations:** record each conversation id on workers, reviewer cycles, and the analyst (`PERSISTENCE.md`); show it with an "Open in Antigravity" action
  (`SCREEN_INVENTORY.md`) limited to what is verified; AT-026 fails if any conversation cannot be shown, rather than passing on Vela's own rendering.
- **For Prompt 6:** the profile-isolation/adapter tickets depend on I100; FR-050/AT-026 tickets cannot be marked done on an unverified mechanism.

## Unresolved Compatibility Constraints

1. **Autonomous mode stays blocked** until the Native Permission Posture probe passes against the installed version (ADR-009); Supervised needs acknowledgement.
2. Vendor bugs contradict documentation: hook `allow` cannot grant (#1053 reproduced); `permissionOverrides` ignored (#1059); `--print-timeout` default differs
   from docs; workspace writes are denied in headless mode without a rule although docs say auto-allowed; the Windows sandbox docs conflict with the changelog.
3. Persistent prompts occur in the Desktop GUI under Always Proceed-style settings; Desktop 2.17.0 appears to expose only Plan Review Policy (user report).
4. The CLI self-updates in the background; version drift mid-run is a divergence handled by reconciliation.
5. Agent commands run in PowerShell, not `cmd`; the policy normalizer accepts only a small grammar and sends the rest to `ASK`.

## Remaining Human Decisions and Actions

1. **Does v1 create Desktop-hosted Antigravity sessions?** On the primary headless path no GUI card exists, so `vela-uia` is dormant unless such sessions exist
   (ADR-018 section 5). Decide before tickets for the UIA adapter are prioritized (they are still specified and gated).
2. Add the **Stitch reference image** under `docs/ui/reference/` (still missing).
3. Confirm Google's position on external orchestration of the headless `agy` with the user's own account (release prerequisite).
4. Record the **reference hardware** for the performance budgets before the performance pass.
5. Untouched MINOR findings: SA-31 (full configuration schema), SA-33 (WCAG target, multi-monitor/DPI), SA-34 (log retention), SA-35 (boilerplate duplication),
   SA-37 (manual edge persistence, serialization override).

## Environment Side Effects of the Probe Work (outside the repository)

`agy` 1.2.14 is installed per-user at `%LOCALAPPDATA%\agy\bin` and the user PATH was updated by its installer; the CLI created the Antigravity project entries
"CLI Project" and "wt1"; `C:\vela-probe` (throwaway repo, worktrees, isolated profile, logs) exists outside OneDrive; the IDE's Chromium accessibility was switched
on by a UIA probe until it restarts. None of these are part of the Vela repository.

## Gating Rules for Ticket Generation (Prompt 6)

- Generate **concrete Antigravity production tickets and fake/test-adapter tickets only**; no Claude/Codex/other runtime-adapter tickets (ADR-008).
- Every **[U]** element in the register becomes a **timeboxed spike ticket** with a recorded-result acceptance criterion; no dependent ticket may be marked
  ready/done on an unverified element until that spike closes with evidence. Spikes precede dependents in the DAG.
- Honor the crate dependency rules and the seed additions in `docs/issues/ISSUE_GRAPH_SEED.md` ("Architecture-Driven Seed Additions" and I100); cite
  requirement IDs FR-001..FR-050 and acceptance tests AT-001..AT-026; reference the relevant ADRs and component ids (C01..C22).
- Tracer-bullet vertical slices; real blockers only; conservative parallelism (shared contracts, the migration ladder, `Cargo.toml`/`package.json`,
  `tauri.conf.json`, and generated bindings are serialization hazards).

## Instructions for the Next Model (beginning Prompt 6)

1. Run the **universal handoff prompt** in `VELA_MASTER_BUILD_PLAYBOOK.md` section 2 first. Read, in order: `AGENTS.md`, this file, `CONTEXT.md`, `README.md`,
   `DOCUMENTATION_INDEX.md` (precedence), `docs/architecture/IMPLEMENTATION_ARCHITECTURE.md`, `docs/architecture/COMPONENT_SPECIFICATIONS.md`, ADR-007 and
   ADR-009..019, `docs/product/PRODUCT_SPEC.md`, `docs/issues/ISSUE_AUTHORING.md`, `docs/issues/ISSUE_GRAPH_SEED.md`, `docs/roadmap/IMPLEMENTATION_PLAN.md`,
   `docs/testing/ACCEPTANCE_TESTS.md`, and `docs/research/EXTERNAL_VERIFICATION_2026-10-02.md` (sections T-V). Verify `git status`, that `HEAD` descends from the
   commits above, and that `main` equals `origin/main`.
2. Do **not** redo Prompts 1-5 and do not reopen accepted ADRs without discovering an actual contradiction; surface contradictions as decision requests.
3. Run **Prompt 6** from the playbook. First inspect installed Matt Pocock skills (`to-tickets`, `setup-matt-pocock-skills`); the verified facts are in
   `docs/agents/MATT_POCKOCK_SKILLS.md`. Note that `/setup-matt-pocock-skills` edits `CLAUDE.md` or `AGENTS.md` and writes `docs/agents/*.md`: do that only on a
   separate branch/commit under the preflight bootstrap rule, and tell the user.
4. Creating GitHub issues is an external, hard-to-undo action: confirm `gh auth status`, the target repository (`Jaweria-Abdul-Hameed/vela`), and ask the user
   before bulk creation; if `gh` is unavailable, draft the issues as local Markdown under a clearly named directory and stop for instructions.
5. After issue creation, build the real dependency DAG from the created issue ids and record the issue-graph status here. Then stop (Prompt 7 is an independent
   audit, preferably by a different model).
6. Real-environment note: `agy` is installed on the user's machine; any probe or spike must use a throwaway repository outside OneDrive, must not read or copy
   Antigravity credentials, and must not modify the user's global Antigravity settings (use the isolated-profile technique).
7. Before reporting Prompt 6 complete, update this file, commit and push, and verify `main` equals `origin/main` with a clean working tree.
