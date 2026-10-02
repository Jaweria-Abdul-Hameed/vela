# Vela Current State

> Checkpoint, not a specification. It summarizes reality and never overrides Git,
> tests, ADRs, specifications, or tracker state. If it conflicts with them, correct
> this file and record the discrepancy.

## Product

Vela

## Current Phase

PROMPT_4_COMPLETE — External verification and real-environment probes done as far as this environment
allows (2026-10-02). Human decisions DR-1 and DR-3 recorded (ADR-016).

## Last Completed Phase

Prompt 4 — Verify volatile external assumptions (research documents, ADR-016, and limited factual
specification updates only; no architecture, issue, or source changes).

## Next Phase

Prompt 5 — Architecture freeze, **with the gating rules below**. The user asked to stop before Prompt 5.
Before or during Prompt 5 the user should run the probes in `docs/research/EXTERNAL_VERIFICATION_2026-10-02.md`
section S (they need an installed, authenticated `agy`, which Vela and this agent must not handle).

## Canonical Branch and Commit

- Branch: `main` (remote `origin`: `https://github.com/Jaweria-Abdul-Hameed/vela.git`)
- Specification baseline: `bbcecf72b5efe842a48330cd080d4a2180546e97`
- Prompt 3 specification fixes: `2d9a8fd8ccb468d6c12bfc9caad61139b7b87a4c`; Prompt 4 first checkpoint: `0f1480a`
- This checkpoint is the commit that updates this file; a file cannot contain its own SHA. Take current Git
  HEAD as canonical and verify it descends from the commits above.

## Status Summary

| Area | Status |
|---|---|
| Specification | FIXED_AFTER_AUDIT; DR-1/DR-3 recorded; remaining queued items listed below |
| External verification | Documentation verified; real-environment probes: Git, keep-awake API and UIA (proxy) done; CLI probes UNVERIFIED |
| Antigravity capability contract | Defined; headless CLI probes now VERIFIED for CAP-01..03, 08, 10 and the native posture candidate; CAP-04 (approval events, `WAITING`), 05, 07, 09 and UIA delivery still not fully verified |
| Architecture | NOT_FROZEN |
| Issue graph | NOT_CREATED |
| Implementation | NOT_STARTED |
| Open human decisions | None blocking; carry-over: Stitch reference image |

## Runtime Scope and Surface Priority (ADR-016)

Vela v1 runtime is **Google Antigravity on Windows**; strategy **Antigravity-first, provider-extensible**.

- **Primary surface:** the official `agy` CLI headless interface, **conditional on real-environment probes
  confirming the Required Capability Contract**. Use supported hooks, session, and conversation mechanisms where
  verified.
- **Secondary:** Windows UI Automation for capabilities the primary surface cannot provide, particularly approval
  delivery for Desktop GUI prompts (opt-in, ADR-010). **Last resort:** guarded visual adapter (ADR-007).
- **Not a v1 dependency:** the Python SDK (official but Alpha/Research Preview, API-key/Vertex authentication
  only, resume and cancellation undocumented; fails DR-3).
- **Authentication (DR-3):** official binary with the user's normally authenticated Antigravity session;
  authentication is owned by Antigravity; Vela never reads, extracts, copies, exports, persists, or reuses tokens
  or credentials (including Windows Credential Manager entries); `AUTH_REQUIRED` asks the user to authenticate
  through Antigravity's flow; API-key/Vertex is not required and never silently replaces the runtime. The vendor
  standing of external orchestration of the headless CLI is **not confirmed by a vendor source**; confirming it is
  a release prerequisite.
- The user's persistent approval prompts were observed in the **Antigravity Desktop GUI on Windows**, including under
  permissive/Always Proceed-style settings. This remains a requirement.

## External Integration Readiness (evidence: `docs/research/EXTERNAL_VERIFICATION_2026-10-02.md`)

**Verdict: the contract and priorities are defined; no Antigravity capability is yet verified as ready.**

Verified in this environment (Windows 11 Home 10.0.26200; Git 2.45.1; Node 24.19.0; WebView2 154; Desktop 2.17.0;
IDE running; `agy` not installed):

- **Git hardening (P10) VERIFIED:** repo-local config executed external diff, textconv, filter clean/smudge,
  fsmonitor and hooks; the hardened invocation neutralized all of them.
- **Keep-awake API (P9) VERIFIED; effect on sleep PARTIALLY VERIFIED** (not measured). No secure-screensaver or
  inactivity-lock policy is set on this machine; Modern Standby is in use.
- **UI Automation on Electron/Chromium (P7) PARTIALLY VERIFIED on a proxy (the IDE):** the app was not elevated; the
  first UIA query returns a near-empty tree (13 elements) and Chromium then populates about 600 elements and keeps
  them, so the adapter must warm up and retry. The Desktop approval card itself was **not observed**. This probe
  switched on Chromium accessibility in the running IDE until it restarts.
- **CLI release integrity:** latest manifest 1.2.14 (`windows_amd64`) with SHA-512; per-user install to
  `%LOCALAPPDATA%\agy\bin`; PATH modification unless `--skip-path`; the CLI **self-updates in the background**
  (disable/pin undocumented); installer script unsigned. **Integrity VERIFIED without executing:** the 1.2.14 binary's SHA-512 matches the manifest and its Authenticode
  signature is Valid (signer Google LLC). Runtime behavior and update control remain UNVERIFIED.
- **Documentation-only, UNVERIFIED in the real environment:** headless permissions and soft-denial (P1), hook
  `allow`/`deny`/`ask` semantics and failure mode (P2), agent-writability of hooks/settings (P3), skill invocation
  in print mode (P4), workspace trust for a new worktree (P5), `WAITING` and cancellation (P6), parallel sessions
  and quota (P8), Desktop approval-card accessibility tree and correlation, Tauri `prevent_exit` at runtime.
- **Open vendor bugs contradicting documentation:** #548, #1053, #1054, #1059, #1114, #1018, #1048.
- **UNSUPPORTED for v1:** the SDK as a dependency (above).

**Compatibility consequence (per `ADAPTERS.md` and ADR-016):** an unverified Required capability is treated as
unavailable for release claims. This checkpoint makes **no adapter workaround**: if a probe shows a Required
capability unavailable, the outcome is a specification-change decision (ADR). Autonomous mode stays blocked while the
Native Permission Posture is `UNKNOWN` (ADR-009).

## Section S Probe Results (executed after Prompt 4; evidence in the verification record, sections T and U)

Installed `agy` 1.2.14 (user ran the installer path through me), probed in the throwaway repo `C:\vela-probe`
(outside OneDrive; the Vela repository was untouched). **VERIFIED:** headless soft-denial of commands (exit 0,
`status SUCCESS`, stderr only); user-level `permissions.allow` rules (exact, regex, path-scoped `write_file`) work and
unlisted/outside targets are denied; settings can be isolated per run via `USERPROFILE`/`HOME` with authentication
still working; `PreToolUse` hooks load from workspace `.agents/hooks.json`, `deny` is a hard block (even under
skip-permissions), crash/garbage/timeout/empty responses are **fail-closed**, and hook `allow` **cannot grant**
(bug #1053 reproduced); skills resolve in print mode; new worktrees run headless without a trust prompt; kill leaves no
orphans and `--conversation` resume preserves context; the agent's shell is `powershell.exe`; the Desktop 2.17.0
approval card is readable through UI Automation after warm-up (inside the window; five options with `Invoke`
patterns including persistent "always allow" options; Skip and Submit; "Waiting for user input"). **PARTIALLY
VERIFIED:** policy files inside an allowed worktree would be agent-writable; two parallel sessions. **UNVERIFIED:**
UIA delivery (invoke and verify progress), `WAITING` status, global-path hooks in an isolated profile, interactive
mode, self-update control, Desktop permission presets (the user reports only Plan Review Policy exists in 2.17.0
and prompts persist under Always Proceed).

**UIA delivery (executed with user consent, verification record section V):** on the pending Desktop card, Vela's
probe selected only option 1 ("Yes, allow this time") with `SelectionItemPattern` and invoked `Submit` with
`InvokePattern` (no coordinates), after a fail-closed correlation check (title, command fragments, working directory,
status text, exactly one matching control each). The card cleared, the agent reported the result, and the transcript
shows the correlated request (`git status --short`, `Cwd c:\vela-probe\wt1`) with the agent resuming at the Submit
second. A before/after hash of `~/.gemini/config` showed **no persistent permission created** (zero file differences).
**VERIFIED** for one single-use allow; broader reliability (variants, versions, concurrent cards, the refusal
path) is PARTIALLY VERIFIED. The earlier "UIA delivery UNVERIFIED" items above are superseded. Antigravity's
"worked for 19 minutes" is the agent's wait on the unattended card (request `11:44:17Z`, resumed `12:03:18Z`, the
Submit second), not execution time; an unanswered Desktop prompt blocks indefinitely, which supports the Approval
Watchdog requirement.

**Impact:** nothing blocks Prompt 5. The CLI headless path has a verified candidate native posture (generated allow
rules plus a fail-closed deny hook) that needs no UI automation; UIA is only for Desktop-hosted prompts. Queued
specification changes (not applied): ADR-009 mechanism text; EBR-1 via hook input; error handling (exit 0 and
`SUCCESS` are not proof of execution; set explicit `--print-timeout`); UIA adapter rules (warm-up, role/label
matching, never select persistent "always allow" options); PowerShell normalization versus the "Command Prompt
preferred" statements (SA-36); documentation conflicts. The `CLI Project` registry entry was created by the CLI;
why the Desktop opened during the probes is unexplained.

## Gating Rules for Prompt 5

1. Freeze the `AgentAdapter` seam, the capability contract, and the surface priority; **do not freeze
   `AntigravityAdapter` internals** that depend on unverified behavior (approval delivery path, hook semantics,
   workspace trust, skill invocation, cancellation).
2. Treat probes P1-P8 and the Desktop approval-card probe as Phase 0 work and release-gating.
3. Resolve the open design question (record section R): how approvals reach Vela for headless primary-surface
   sessions (rules, `PreToolUse` hooks, or a Desktop-hosted session) versus the UI-automation surface that only
   acts on Desktop-hosted prompts. This is not decided.
4. Keep the Approval Broker as specified (ADR-007, 009, 010).

## Remaining Queued Specification Items (record section L; not applied)

Fixed-point rule for reviews after an integration merge (`git diff <fp>...HEAD` is polluted once the integration
tip is merged; `REVIEW_PROTOCOL.md` carries a marker); per-worktree workspace trust and its consent; Vela-only
execution workspaces versus Antigravity-created worktrees; preflight additions (submodules, Git version,
project-level skills, WebView2 refresh for a long-lived process, Desktop version drift); skills-setup bootstrap
handling. Applied in this phase: ADR-016, FR-049, AT-025, `ADAPTERS.md` surface note, `PREFLIGHT.md`
authentication/installation checks, `CAPACITY_AND_PROFILES.md` authentication ownership, `SECURITY_AND_PERMISSIONS.md`
credential rule, `REVIEW_PROTOCOL.md`/`PROMPT_CONTRACTS.md` corrections (the skill emits no severity), `RECOVERY.md`
background-update divergence.

## Accepted ADRs

ADR-001..ADR-016 (all Accepted). ADR-016 refines ADR-008 and records DR-1/DR-3.

## External Research Last Verified

2026-10-02 (`EXTERNAL_VERIFICATION_2026-10-02.md`). Vendor documentation and the CLI (manifest 1.2.14; changelog
shows 1.2.12; Desktop 2.19.1 latest, 2.17.0 installed) change quickly; revalidate before architecture freeze,
before implementation of the adapter, and before release.

## Known Compatibility Constraint

Approval prompts persist in the user's Antigravity Desktop GUI on Windows despite permissive settings. The Approval
Broker is required (ADR-007); Vela does not depend on unconditional native auto-execution (ADR-009); guarded UI
automation is opt-in (ADR-010). No password, 2FA or CAPTCHA automation; no quota circumvention; no handling of
Antigravity credentials.

## Remaining Items from Earlier Phases

Stitch reference image still missing (`docs/ui/reference/`); untouched MINOR findings SA-30, SA-31 (schema),
SA-33..SA-37; Prompt 3 judgment calls remain revisitable; performance reference hardware unrecorded.

## Recovery / Reconciliation Status

Not applicable; no runtime exists.

## Next Agent Instructions

1. Read `AGENTS.md`, this file, `DOCUMENTATION_INDEX.md` (precedence section), ADR-007, ADR-009..016, and
   `docs/research/EXTERNAL_VERIFICATION_2026-10-02.md` (especially sections O-S). Verify Git status and that HEAD
   descends from the commits above.
2. Do not re-run Prompt 4. Ask the user whether the section S probes have been run; incorporate their results into the
   research record before relying on any CLI behavior.
3. Run Prompt 5 under the gating rules above. Do not invent adapter behavior for unverified capabilities and do not
   substitute another provider or the SDK.
4. No source code and no GitHub issues until Prompt 6.
5. Before reporting Prompt 5 complete, update this file, commit and push.
