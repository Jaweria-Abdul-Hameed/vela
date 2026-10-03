# Vela Current State (authoritative cross-model checkpoint)

> Checkpoint, not a specification. It summarizes reality and never overrides Git, tests, ADRs, specifications, or
> tracker state. If it conflicts with them, correct this file and record the discrepancy. Any model (Claude, Gemini,
> Codex, Antigravity) can continue from the repository plus this file.

## Product

Vela: a Windows-first native desktop control plane that autonomously orchestrates coding-agent work over a dependency-aware
issue graph. **v1 runtime: Google Antigravity on Windows. Strategy: Antigravity-first, provider-extensible** (ADR-006, ADR-008).

## Current Phase

**PROMPT_9_F01_BOOTSTRAP: NEEDS_HUMAN (blocked by an environment limitation, see "Prompt 9 / F01 Status" below).** Prompt 8 is complete and checkpointed at `532cf32890e6dd26169213390ce8a0c8a45caf70` (the corrected graph runs from the initial frontier to Z06 in 25 simulated rounds with no deadlock) — **Architecture frozen** (2026-10-02); the issue graph was audited (Prompt 7, checkpointed at `0f976d1bb67d50c094e729538643f7f7fb847958`) and then executed end to end in a scheduler dry-run (Prompt 8, 2026-10-03), which made small contract-safety corrections: **135 tickets, 356 authored edges, 277 published edges, critical path 18 tickets, initial frontier 14**. **The planning graph is READY FOR IMPLEMENTATION.** GitHub publication is still deferred pending the user's explicit confirmation. See `docs/issues/graph/SCHEDULER_DRY_RUN.md`, `AUDIT_PROMPT7.md`, and `SHARED_SURFACE_PROTOCOL.md`.

## Last Completed Phase

Prompt 7 — independent issue-graph audit (checkpointed at `0f976d1bb67d50c094e729538643f7f7fb847958`, `main` equal to `origin/main`, 75 final checks passed: 135 tickets, 347 authored and 274 published edges, frontier 14, critical path 17). Prompt 8 (scheduler dry-run) then verified that checkpoint from Git, rebuilt the DAG from the ticket Markdown (identical), simulated all 135 tickets to completion, and corrected six ordering defects (listed below). Planning artifacts only: no source code, no GitHub issues, no H05 provisioning.

## Next Phase

**Close Prompt 8** (the user reviews the dry-run report and approves a commit and push), then **Prompt 9 — bootstrap the repository (ticket F01, the only code ticket in the initial frontier)**. Until GitHub publication, the canonical assigned ticket is `docs/issues/graph/tickets/<KEY>.md`. The playbook's "GitHub issue" references the corresponding canonical ticket. Publication remains a separate, user-confirmed step (plan in `ISSUE_GRAPH.md`). Do not begin Prompt 9 in the Prompt 8 session.

## Canonical Branch and Commit

- Branch: `main`; remote `origin`: `https://github.com/Jaweria-Abdul-Hameed/vela.git`.
- Prior checkpoints: baseline `bbcecf7`; **Prompt 7 `0f976d1bb67d50c094e729538643f7f7fb847958`**; Prompt 3 `2d9a8fd`; Prompt 4 `1bbc38b`; Section S probes `27d2408`, `cd50b5a`, `f5dac49`; Prompt 5 `8b46b25`;
  ADR-020 `3cc25f7`, scope clarification `4e9f657`.
- Current working location on this machine: `C:\Users\Jaweria\Projects\Vela`, a local directory outside any cloud-sync service (the project was moved out of OneDrive on 2026-10-03). This is this machine's current location only, not a portable architectural requirement; see the setup guidance in `VELA_MASTER_BUILD_PLAYBOOK.md`.
- **This checkpoint is the commit that updates this file** (a file cannot contain its own SHA). Take current Git `HEAD` as canonical
  (`git log -1`) and verify it descends from the commits above and that `main` equals `origin/main`.

## Status Summary

| Area | Status |
|---|---|
| Specification | READY (audit findings resolved; Prompt 4/5 corrections applied; items below remain open but non-blocking) |
| External verification | Documentation verified; Section S probes done; several capabilities remain unverified (see register) |
| Architecture | **FROZEN** (`docs/architecture/IMPLEMENTATION_ARCHITECTURE.md`, `COMPONENT_SPECIFICATIONS.md`, ADR-017/018/019) |
| Issue graph | **DRAFTED_NOT_PUBLISHED, READY FOR IMPLEMENTATION**: Prompt 7 audited it; the Prompt 8 dry-run (uncommitted until approved) scheduled all 135 tickets in 25 rounds and corrected it to 356 authored and 277 published edges (symbolic keys in `docs/issues/graph/`; no GitHub issues exist) |
| Implementation | **F01 BLOCKED, NOT COMPLETE**: written in the working tree, uncommitted, Rust gates unverified (Smart App Control blocks Cargo build scripts); no ticket is complete |
| Next gate | The user's decision on how Rust gates will run (see "Prompt 9 / F01 Status"); then finish F01 (rerun the gates, checkpoint, `/code-review`), still from the canonical local ticket `docs/issues/graph/tickets/F01.md`. Prompt 10 has not begun |

## Prompt 9 / F01 Status (2026-10-03): IN PROGRESS on branch `f01/bootstrap` (not merged to `main`)

- **Update:** the user chose CI for the Rust gates. Branch `f01/bootstrap` at `71bf92a` (pushed, parent `532cf32`) passed CI run 37143358949 (Rust: fmt, clippy `-D warnings`, build `--all-targets`, test, structure checks; TypeScript: `npm ci`, verify). Local Rust remains **BLOCKED/NOT VERIFIED** (Smart App Control); this is a host limitation, not an implementation defect. Authoritative review and final merge to `main` are pending the user's approval. The bullets below describe the state before this update.

- **Base:** Prompt 8 checkpoint `532cf32890e6dd26169213390ce8a0c8a45caf70`; at the start `main` equalled `origin/main` with a clean tree. **No F01 commit exists; no push occurred.**
- **Written (uncommitted working tree):** Cargo workspace (all 14 members) and npm workspaces, pinned toolchains, `Cargo.lock` (550 packages) and `package-lock.json`, per-crate dependency declarations with the `windows` feature sets,
  every module skeleton, the dependency-direction checker and module-skeleton checker with tests, CI workflow, `.editorconfig`/`.gitattributes`/`.gitignore`, README "Development Commands",
  `docs/architecture/TOOLCHAIN_AND_DEPENDENCIES.md` (pins, assignment, interpretations), `docs/project/F01_EVIDENCE.md` (commands and results). One protocol correction: `wiring/settings` added to the section 1 table of `SHARED_SURFACE_PROTOCOL.md` (S02 owns it per section 5).
- **Green locally:** `npm run typecheck`, `test:ts`, `lint:ts`, `fmt:check:ts`, `cargo fmt --check`, both structure checks (39 checker tests, including the illegal `vela-core -> vela-git` edge failing against a real Cargo workspace).
- **BLOCKER:** Smart App Control is in enforcement on this machine and blocks the unsigned build scripts that `cargo` compiles (`os error 4551`), so `cargo build`, `clippy` and `test` do not run here, for F01 and for every later Rust ticket. Nothing in the policy was changed. Details: `docs/project/F01_EVIDENCE.md`.
- **Not verified:** `cargo build --locked`, clippy, `cargo test`, clean-clone `npm ci` and build transcript, CI on a clean checkout. The authoritative `/code-review` and the checkpoint commit were not started because the gates are not green. `/implement` and the Matt Pocock skills are not installed; the `AGENTS.md` loop was followed manually.
- **Human decision needed (smallest):** choose how Rust gates run: (a) the user turns Smart App Control off (one-way in Windows; their decision alone); (b) run the Rust gates on a clean Windows machine or the CI runner, which needs the user's approval to push a work branch (and H05 stays unprovisioned until Z07); (c) another route the user prefers. Do not weaken Smart App Control without the user's explicit instruction.
- **Unchanged:** architecture FROZEN; graph READY FOR IMPLEMENTATION; GitHub issues NOT published; H01, H04, H06 OPEN; H05 NOT provisioned.
- **Environment:** Rust 1.99.0 was installed per-user via `rustup` (`PATH` not modified).

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

**Product requirement (user clarification, 2026-10-02, scope refined same day):** every Vela issue/worker, authoritative reviewer session, and
substantive analyst session whose output enters the audit trail runs in a **fresh Antigravity conversation**. **Scope limit:** it does not apply to
deterministic Vela computation or incidental/diagnostic agent invocations (capability/posture probes, version checks, smoke tests); ADR-020 decision 1a.
Each in-scope conversation and that conversation must be **discoverable and openable in the Antigravity 2.0 Desktop GUI** so the user can inspect
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

## Canonical Reference Hardware (H03, resolved 2026-10-03)

The user's current development laptop is Vela's canonical machine for performance measurement. The authoritative record is the **"Reference Hardware Profile" section of `docs/ui/PERFORMANCE_BUDGET.md`, profile `REF-HW-1`**. Performance agents (PF1, U17 measurement runs, the release validation report) must read that section and must capture the actual environment per run; do not copy numbers from this summary.

- **Verified on the machine (read-only queries, 2026-10-03):** Acer Nitro ANV16S-71; Intel Core 9 270H (14 cores, 20 threads); one 16 GB Kingston DDR5-5600 module (single-channel); NVIDIA GeForce RTX 5070 Laptop GPU (8 GiB, driver 616.92) plus an Intel integrated GPU (driver 32.0.101.7079) that reports the display mode; 2560 x 1600 at 180 Hz, about 15.9 in, Windows scaling 150% (device pixel ratio 1.5); Windows 11 Home 25H2 build 10.0.26200.9550; power scheme "Acer"; WebView2 154.0.4258.53; 1 TB NVMe.
- **Supplied by the user, not verified:** the name "Acer Nitro 5" (Windows reports Nitro ANV16S-71), the development-machine role, the intent about future RAM, and any GPU power limit or vendor performance mode (not queryable here).
- **Policy:** 16 GB is the *current* reference configuration. A change to CPU, GPU, RAM (capacity, module count, or channel layout), the panel, or the machine creates a new profile (`REF-HW-2`, and so on) and never rewrites REF-HW-1 or results already recorded against it. Every benchmark records the profile ID and a per-run environment capture (power source and scheme, GPU that renders WebView2, graphics mode, reduced motion, window state, DPR and its cap, driver and WebView2 versions, display mode, machine exclusivity). Hybrid graphics means the rendering GPU must be captured per run.
- **Reference-run conditions (approved 2026-10-03):** a result counts as release evidence only for a **reference run**: REF-HW-1 hardware as recorded, AC power connected, a stable Windows power scheme with its exact name and GUID recorded, no deliberate heavy competing workload, and the full environment capture (rendering GPU, Windows version and build, GPU driver versions, WebView2 runtime version, graphics mode, reduced-motion state, window state, effective DPR and its cap, display resolution and refresh rate, and any Acer or Nitro performance mode that can be reliably determined). **No Acer or Nitro performance mode is mandated**, because there is no reliable programmatic fact about which modes exist or which should be canonical; that is a future decision. Any other run is a **non-reference exploratory run**: it may use different conditions but must be labelled non-reference and must never be release evidence. The conditions live in `docs/ui/PERFORMANCE_BUDGET.md` (kept there, not in a separate document).
- **Budgets are unchanged and not weakened.** Full, Balanced (default), and Efficiency are each measured; reduced motion, minimized and hidden, unfocused, the device-pixel-ratio cap, and adaptive suggestions remain in scope. No benchmark numbers exist yet and none were invented.

## Unresolved Compatibility Constraints

1. **Autonomous mode stays blocked** until the Native Permission Posture probe passes against the installed version (ADR-009); Supervised needs acknowledgement.
2. Vendor bugs contradict documentation: hook `allow` cannot grant (#1053 reproduced); `permissionOverrides` ignored (#1059); `--print-timeout` default differs
   from docs; workspace writes are denied in headless mode without a rule although docs say auto-allowed; the Windows sandbox docs conflict with the changelog.
3. Persistent prompts occur in the Desktop GUI under Always Proceed-style settings; Desktop 2.17.0 appears to expose only Plan Review Policy (user report).
4. The CLI self-updates in the background; version drift mid-run is a divergence handled by reconciliation.
5. Agent commands run in PowerShell, not `cmd`; the policy normalizer accepts only a small grammar and sends the rest to `ASK`.

## Remaining Human Decisions and Actions

1. **H06: does v1 create Desktop-hosted Antigravity sessions for approval delivery (UI Automation path scope)?** On the primary headless path no GUI card exists, so `vela-uia` is dormant unless such sessions exist (ADR-018 section 5). This is **independent of ADR-020**: it does not decide FR-050, CAP-12, or AT-026, does not reinterpret ADR-020 decision 1a, and does not authorize Desktop GUI automation to create or host conversations (ADR-020 decision 2: last resort, separate explicit decision, only if SP08 proves no supported route satisfies ADR-020). Desktop-visible or openable does not mean Desktop-created or Desktop-hosted. Open until shortly before K07; blocks K07 and the real-Desktop approval-delivery scope of Z04 (AT-011, AT-017) only; depends on SP09, not SP08. **Provisional posture:** headless or programmatic `agy` is the primary orchestration path; the UIA approval adapter may be implemented and tested against the controlled fake approval window; real-Desktop approval-delivery acceptance scope is unresolved; none of this weakens or substitutes for ADR-020, FR-050, CAP-12, or AT-026.
2. ~~Add the Stitch reference image~~ **RESOLVED (H02, 2026-10-02):** the actual user-supplied Stitch reference is in the repository at **`docs/ui/reference/vela-stitch-reference.png`** and was inspected directly (see `docs/ui/REFERENCE_BRIEF.md` for what it shows and the recorded differences from the written direction). **Future UI implementation and UI fidelity-review agents must inspect the actual image, not rely only on textual reconstruction or earlier conversation.** The image is a visual reference only; it never overrides the written specifications for non-visual behavior. H02 was removed from the issue graph because a resolved human action is not published as an issue.
3. **H01 (OPEN, not resolved):** Google's position on external orchestration of the headless `agy` with the user's own account. Researched 2026-10-03 from primary sources; evidence state **PARTIALLY-DOCUMENTED** (the CLI and headless automation are officially documented; orchestration at Vela's level is not addressed by the Terms, FAQ, or docs; the one affirmative statement is an informal Google-flagged forum reply that is in tension with the literal Terms and FAQ wording). Full record: `docs/research/H01_google_position.md`. **Decisions of record (user, 2026-10-03):** (1) **release gate only**: H01 gates Z06 and nothing earlier, there is no H01 to A03 edge, and development, local integration, spikes, and validation of the Antigravity adapter proceed under documented uncertainty, which is **not** a conclusion that Google permits the pattern, and Vela v1 must not be represented as release-ready while H01 is open; (2) an **authoritative written answer from Google is wanted** (draft question in the record; nothing was sent; the forum reply does not resolve H01), and obtaining it is the remaining action; (3) the **Gemini API-key route is out of scope for v1** (possible future path only; ADR-016 unchanged; v1 stays Antigravity-first with Antigravity-owned authentication); (4) **provider-policy-block handling is required** and is specified by extending FR-024 and AT-007: a provider-neutral `PROVIDER_POLICY_BLOCK` is non-retryable, never worked around (no retry, account or profile switching, credential change, or API-key fallback), stops provider work safely with durable state preserved, is surfaced as a `NEEDS_HUMAN` intervention, survives restart without automatic retry, and clears only by an explicit user decision after a user-initiated access re-check (`docs/orchestration/CAPACITY_AND_PROFILES.md`). The Antigravity signal mapping is unverified `[U]`; never provoke a block to obtain evidence. Do not treat the forum reply or Vela's successful probes as policy approval.
4. ~~Record the reference hardware~~ **RESOLVED (H03, 2026-10-03):** the user's development laptop is the canonical performance reference machine, recorded as profile **`REF-HW-1`** in the "Reference Hardware Profile" section of `docs/ui/PERFORMANCE_BUDGET.md` (see "Canonical Reference Hardware" below). H03 was removed from the issue graph because a resolved human action is not published as an issue.
5. **H04 (OPEN; production-release prerequisite only, decisions of 2026-10-03):** production release signing. Development and testing use unsigned local builds, throwaway self-signed development certificates, and disposable test updater keypairs (Z01, Z02), which are never production trust material. **Tauri updater signing** (its own keypair, required by the updater) is separate from **Windows Authenticode** signing (the installer and Windows binaries; production route undecided). Deferred to the release-signing phase and the user's: the production Authenticode provider or route, any purchase or spend, the identity-validation route, production credentials, production updater private-key generation and custody and backup (a lost key can prevent updating installed copies), and the final updater hosting location. No vendor is chosen, nothing is purchased, no key has been generated. H04 gates Z06 directly (the production release-signing gate); it does not block Z01 or Z02. Research and cost labels (verified, unverified or illustrative, unknown): `docs/research/H04_signing_research_2026-10-03.md`. Smart App Control was observed ON on the current development machine and may affect unsigned or self-signed development artifacts; it was not changed and changing it is not a prerequisite for development (surface an actual blocker instead of weakening it).
6. **H05 (OPEN; route DECIDED, environment NOT YET PROVISIONED; decisions of 2026-10-03):** the canonical clean-machine validation environment is a local **Windows 11 virtual machine under VMware Workstation Pro** on the development machine (not the only technically valid hypervisor; VirtualBox remains a possible alternative). The guest is the official Microsoft **Windows 11 Enterprise 90-day Evaluation**: an Enterprise guest, not Windows Home; time-limited; disposable; not a licensing route for permanent infrastructure; testing on Windows Home or another edition must be added deliberately. A spare physical machine is optional supplementary validation, not a v1 prerequisite. Initial configuration (configurable, not a performance requirement): 4 vCPU, about 4 to 6 GB RAM, a dynamic 60 to 100 GB disk, virtual TPM 2.0 and Secure Boot, no nested virtualization, no GPU passthrough; a quiet-machine workload that must not run concurrently with heavy Antigravity worker activity, performance benchmarking, or large WSL workloads on this 16 GB host. States (created only when a scenario needs them): S0 pristine baseline (no Rust, Node, Git, Antigravity, or Vela; VMware guest tools recorded as part of the baseline), S1 N-1 installed, S2 Git installed. WebView2: removal is not a prerequisite; a missing-runtime path that cannot be obtained by a supported method is recorded **NOT VERIFIED**, never passed. Smart App Control: not mandated on or off; record its state for every relevant run; a rejection of a development or self-signed artifact under an enforcing posture is expected evidence. Artifacts enter the guest by SHA-256 manifest and a read-only virtual disk or ISO, verified with built-in Windows facilities. Remaining human action: provision it when Z07 (the first environment-dependent ticket) is about to start; do not provision it during Prompt 7 (the evaluation guest is time-limited). H05 no longer blocks Z01; it gates Z07, which feeds Z05 and Z06. Z07 owns the install, first-launch, login auto-start, uninstall, development-signature, and N-1 to N evidence and waits only for the capabilities that evidence exercises (Z02, W01, K10, L03, L01); the full-product lifecycle checks (tray and background, reboot continuation of a real run, notifications) and the production-signed run belong to Z06. Nothing was installed, downloaded, or changed on the host. Research: `docs/research/H05_clean_machine_2026-10-03.md`.
7. Untouched MINOR findings: SA-31 (full configuration schema), SA-33 (WCAG target, multi-monitor/DPI), SA-34 (log retention), SA-35 (boilerplate duplication),
   SA-37 (manual edge persistence, serialization override).

## Environment Side Effects of the Probe Work (outside the repository)

`agy` 1.2.14 is installed per-user at `%LOCALAPPDATA%\agy\bin` and the user PATH was updated by its installer; the CLI created the Antigravity project entries
"CLI Project" and "wt1"; `C:\vela-probe` (throwaway repo, worktrees, isolated profile, logs) exists outside OneDrive; the IDE's Chromium accessibility was switched
on by a UIA probe until it restarts. None of these are part of the Vela repository.

## Issue Graph (Prompt 6 draft, Prompt 7 audit and Prompt 8 dry-run applied; GitHub publication deferred)

The proposed implementation graph is persisted in `docs/issues/graph/`; the Prompt 7 audit is committed (`0f976d1`), and the Prompt 8 corrections below are in the working tree until the user approves the checkpoint:

- `ISSUE_GRAPH.md`: counts, validation, the DAG table, critical path, initial frontier, layers, spike gates, FR/AT traceability, publication plan.
- `ISSUE_GRAPH.json`: machine-readable graph. `tickets/<KEY>.md`: 135 full ticket bodies.
- `SHARED_SURFACE_PROTOCOL.md` (normative): module skeleton, lockfile rules, reserved migration blocks and table ownership, command registry, composition root and plugin ownership, fault points, resource keys, shared documents, Orchestrator extension points.
- `AUDIT_PROMPT7.md`: the Prompt 7 audit record (historical counts; the current ones are below and in `ISSUE_GRAPH.md`).
- `SCHEDULER_DRY_RUN.md` (new, Prompt 8): rules, graph re-verification, contract-consumption audit, corrections C1 to C6, watch items W1 to W10, the complete 25-round schedule with every ticket accounted for, resource-key lanes, simulated human gates, integration-lane notes, failure-state analysis, and validation.
- **135 tickets; 277 published edges** (356 authored before reduction; 347 and 274 at the Prompt 7 checkpoint); no cycles; every FR-001..FR-050 and AT-001..AT-026 has a ticket. Prompt 8 corrections: R01 to K01; S15 to R02 (S15 to S16 became implied); P02, G04 (implies G03), and Z01 to W01; S13, S14, and R08 to R03 (S12 to R03 and the S13, S14, R08 edges into Z03 became implied); text-only F09 and Z01 edits and a FaultPoint-declaration line in S13, S14, and R08. R03, Z03, Z05, and Z06 each moved one layer deeper.
- **Critical path (18 tickets):** F01 → F02 → F04 → F05 → F08 → S01 → S03 → S04 → S08 → S09 → S10 → S11 → S12 → S14 → R03 → Z03 → Z05 → Z06.
- **Initial ready frontier (14, unchanged):** F01, H01, SP01, SP02, SP03, SP04, SP06, SP07, SP08, SP09, SP10, SP11, H04, H05. Everything except F01 is a spike or a human action. H06 is not in the frontier (it depends on SP09).
- **Scheduler dry-run result:** all 135 tickets scheduled exactly once in 25 rounds (maximum 12 workers in one round), no deadlock; the real-account key (21 holders) and the human-attention key (22 holders), not graph depth, set the schedule length. Simulated human gates (labelled SIMULATED, nothing fabricated): H06 before K07, H05 before Z07 (armed several rounds earlier; provision only when Z07 approaches), H01 and H04 before Z06. **H05 is NOT provisioned. H01, H04, H05, and H06 remain OPEN.**
- **Spike gating:** SP08 (S-DESKTOP-VISIBILITY, ADR-020) directly gates A05 and A12 and transitively the isolated-profile and policy-enforcement path; it does not gate H06. H06 depends on SP09 only.
- **GitHub publication is intentionally NOT done.** `gh` is authenticated with admin access to `Jaweria-Abdul-Hameed/vela`, which has no issues.
- The `to-tickets` skill was followed manually and not invoked. `/setup-matt-pocock-skills` has not been run.
- **Graph tooling is disposable.** The scripts that generated, patched, audited, and simulated the graph (Prompts 6 to 8) were temporary scratch files kept outside the repository; they are not tracked, were not retained, and some hard-coded a former machine path. They must not be reused. The Markdown and JSON files in `docs/issues/graph/` are the source of truth: edit them directly and re-validate independently (rebuild the DAG from the ticket files and compare it with `ISSUE_GRAPH.json`). Do not make any machine-specific absolute path canonical.

## Instructions for the Next Model (Prompt 8 closure, then Prompt 9)

1. Run the **universal handoff prompt** in `VELA_MASTER_BUILD_PLAYBOOK.md` section 2. Read, in order: `AGENTS.md`, this file, `CONTEXT.md`, `README.md`, `DOCUMENTATION_INDEX.md` (precedence), then for Prompt 9 only `docs/issues/graph/tickets/F01.md`, `SHARED_SURFACE_PROTOCOL.md`, `docs/architecture/IMPLEMENTATION_ARCHITECTURE.md`, `DIRECTORY_STRUCTURE.md`, and the ADRs F01 cites. Verify `git status`, that `HEAD` descends from `0f976d1bb67d50c094e729538643f7f7fb847958`, and that `main` equals `origin/main`.
2. **Prompt 8 is complete when the user approves its checkpoint.** Do not redo Prompts 1 to 8 and do not re-simulate unless a ticket, edge, or protocol rule changes; if the graph changes, update the Markdown and JSON together, recompute the reduction, depths, frontier, and critical path from the ticket files, and rerun the scheduler dry-run from the initial frontier.
3. **Prompt 9 is the F01 bootstrap only.** The initial frontier is F01 plus spikes and human actions; the real-account spikes run one at a time in the protocol order (SP08, SP09, SP04, SP06, SP05, SP07, SP10, SP11) and need the user. Follow the round order in `SCHEDULER_DRY_RUN.md`. Each ticket gets a fresh context, its own branch and worktree from the recorded integration tip, the `/implement` flow, a checkpoint, `/code-review` against the recorded fixed point, and a worker-branch push; workers never merge; the serialized lane merges and validates; the frontier advances only after healthy integration.
4. **Human actions stay open and unfabricated:** H01 (release gate on Z06, authoritative Google answer wanted), H04 (production signing, gates Z06), H05 (clean-machine VM not provisioned; provision only when Z07 approaches, never earlier), H06 (Desktop-hosted sessions for approval delivery, gates K07; separate from ADR-020, FR-050, CAP-12, AT-026). Resolve none of them to make progress.
5. GitHub publication needs the user's explicit confirmation; do not publish issues on your own.
6. Honor the standing constraints: concrete Antigravity production tickets plus fake and test adapters only (ADR-008); unverified [U] elements stay gated by their spikes; the conversation-visibility requirement (ADR-020, FR-050) stays an explicit required capability; never claim Antigravity capabilities beyond the verification record; real-account probes use a throwaway repository outside any synced folder, never read or copy credentials, and never modify global Antigravity settings (isolated profile).
7. Before reporting any phase complete, update this file; commit and push only with the user's approval; verify `main` equals `origin/main` with a clean working tree.
