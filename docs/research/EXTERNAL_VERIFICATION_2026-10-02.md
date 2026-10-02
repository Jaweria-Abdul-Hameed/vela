# External Verification Record — 2026-10-02 (Prompt 4)

This is a dated snapshot, not a permanent truth. It verifies the volatile external assumptions in the
Vela specification against primary documentation fetched on 2026-10-02. It supersedes the unverified
statements in `EXTERNAL_INTEGRATIONS_2026-10.md` where they differ; that file's "Observed User
Environment" section remains binding.

## Method and source quality

-   Pages were fetched with a summarizing web fetcher, so quotes below are as reported by it. One
    summarizer error was caught and corrected against verbatim text (Git stash sharing, section R).
    Items marked "secondary" come from search results or community pages, not vendor documentation.
-   Vendor documentation (antigravity.google/docs) is internally inconsistent in places (section A);
    GitHub issues for the official CLI repository record behavior that contradicts the documentation
    (section K). Where documentation and observed behavior differ, Vela stays defensive.
-   The user's observed environment (approval prompts persist despite permissive settings) is
    evidence and is **not** erased by anything below.

> **Later in this file:** the sections headed "Update after Human Decisions DR-1 and DR-3 and Real-Environment
> Probes" (sections O to S) record what was actually probed on this machine and supersede the "UNDOCUMENTED" or
> "RUNTIME" labels for rows D3 (Desktop is Electron/Chromium, UIA tree populates lazily), I6 (hardened Git set
> verified), J3 (`prevent_exit` documented), and C1 (SDK is Alpha with Windows wheels).

## Classification legend

`VERIFIED` matches current primary sources. `CHANGED` current reality differs from the specification
or snapshot. `UNDOCUMENTED` no primary source states it. `UNSUPPORTED` a primary source shows it
does not work as assumed. `RUNTIME` requires runtime capability detection (docs conflict, are
incomplete, or open bugs affect it).

## Primary sources

-   Antigravity agent settings: https://antigravity.google/docs/agent-settings
-   Permissions: https://antigravity.google/docs/permissions
-   Settings: https://antigravity.google/docs/settings
-   Terminal sandbox: https://antigravity.google/docs/sandbox
-   Hooks: https://antigravity.google/docs/hooks/
-   Skills: https://antigravity.google/docs/skills/
-   CLI overview, headless, reference, modes, conversations, features:
    https://antigravity.google/docs/cli/overview (and `/headless/`, `/reference/`, `/modes/`,
    `/conversations/`, `/features/`)
-   SDK overview and policies: https://antigravity.google/docs/sdk/overview/ ,
    https://antigravity.google/docs/sdk/policies/ ; SDK blog:
    https://antigravity.google/blog/introducing-google-antigravity-sdk/ ; repository:
    https://github.com/google-antigravity/antigravity-sdk-python
-   Changelog: https://antigravity.google/docs/changelog
-   Official CLI issue tracker: https://github.com/google-antigravity/antigravity-cli/issues
    (#548, #1053, #1054, #1059, #1018, #1048, #1114 cited below)
-   Google statement on third-party OAuth use (2026-02-27):
    https://github.com/google-gemini/gemini-cli/discussions/20632
-   Forum thread on external orchestration of headless CLI:
    https://discuss.ai.google.dev/t/is-external-orchestration-of-antigravity-cli-headless-mode-supported-with-account-based-usage/183051
-   Matt Pocock skills: https://github.com/mattpocock/skills (README and `skills/engineering/*/SKILL.md`)
-   Skills installer: https://github.com/vercel-labs/skills
-   Git: https://git-scm.com/docs/git-worktree , `/githooks` , `/git-config`
-   Tauri: https://v2.tauri.app/learn/system-tray/ , `/plugin/autostart/` , `/plugin/updater/` ,
    `/distribute/windows-installer/` , `/security/csp/` ; https://docs.rs/tauri/latest/tauri/enum.RunEvent.html
-   Microsoft: UI Automation security
    https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-securityoverview ;
    SetThreadExecutionState
    https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-setthreadexecutionstate ;
    WebView2 distribution
    https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution

## A. Antigravity permissions and auto-execution

| # | Assumption (spec location) | Finding | Class |
|---|---|---|---|
| A1 | Windows exposes Terminal Command Auto Execution modes Request Review, Proceed in Sandbox, Always Proceed (`SECURITY_AND_PERMISSIONS.md`, research snapshot) | Confirmed on the agent-settings page, with a configurable Allow list (Request Review) and Deny list (Always Proceed). | VERIFIED |
| A2 | A newer unified permission engine differs by OS | macOS/Linux use presets Default / Request Review / Turbo; Windows "uses the previous system" where commands default to Ask. | VERIFIED |
| A3 | (new) Native allow/deny/ask rules exist | Rules `action(target)` with precedence **Deny > Ask > Allow**; actions include `command`, `read_file`, `write_file`, `read_url`, `execute_url`, `unsandboxed` (Windows/CLI), `mcp`; regex form `command(regex:...)` shown in CLI docs. Layered over any mode and "always take precedence". Interactive approval is "a card in your editor". | VERIFIED |
| A4 | Windows sandbox availability | **Docs conflict.** Sandbox and permissions pages say Windows uses previous behavior with no unified sandbox; settings page says Windows lacks native sandboxing; the changelog says "File and network sandboxing on Windows are now supported" (desktop v2.15.1, 2026-09-19). | CHANGED / RUNTIME |
| A5 | Settings key names | `toolPermission`: `request-review` (default), `proceed-in-sandbox`, `always-proceed`, `strict`; also `artifactReviewPolicy`, `enableTerminalSandbox`, `allowNonWorkspaceAccess` (default off). Spec names modes only as prose. | VERIFIED |
| A6 | Permissive native mode removes prompts | Documentation says `Always Proceed` executes without prompting except Deny list. The user's observed Antigravity 2.0 Windows environment still prompts. Changelog also shows permission-prompt fixes through v2.17.0 (for example `.git`, `.env`, `.vscode` writes unprompted under Default/Request Review). **Requirement retained.** | RUNTIME |
| A7 | Project-level permission scoping | Projects can inherit or override global permissions; "outside of folder file access policy" is configurable. | VERIFIED |

## B. Antigravity CLI / headless (spec: "CLI, SDK, daemon" unknown, `ADAPTERS.md`)

| # | Assumption | Finding | Class |
|---|---|---|---|
| B1 | A CLI exists | `agy`, a TUI plus headless mode; Windows install via PowerShell or CMD scripts; shares agent infrastructure and settings with Antigravity 2.0. Secondary sources say it replaced Gemini CLI for individual accounts on 2026-06-18 (not confirmed from a vendor page). | VERIFIED (exists) |
| B2 | Programmatic start with a task and context | `agy -p`/`--print`/`--prompt`; `--output-format text\|json\|stream-json`; `--input-format stream-json`; `--json-schema`; `--model`, `--effort`, `--agent`, `--print-timeout` (5m default), `--sandbox`. | VERIFIED |
| B3 | Structured events (CAP-04) | `stream-json` NDJSON: `init` (conversation_id, cwd, tools, `permission_mode`), `step_update` (step type, `tool_name`, `tool_info` with parameters/output/error, subagent info, usage), `result` (status `SUCCESS\|ERROR\|CANCELED\|INTERRUPTED\|INVALID\|WAITING\|RUNNING`). No dedicated approval-request event is documented. Meaning of `WAITING` is not defined. | VERIFIED (events) / UNDOCUMENTED (approval event, `WAITING`) |
| B4 | Skills and slash commands run headless (`/implement`) | Secondary/search summary: slash-command and skill expansion in print mode ("`-p "/my-skill ..."` resolves"). Not found on the fetched primary headless page, which says slash commands abort a `stream-json` stdin session (exit code 2). | RUNTIME |
| B5 | Cancel a session (CAP-05) | Statuses `CANCELED`/`INTERRUPTED` exist; closing stdin ends a stream session; no dedicated cancel command documented. Process termination is the only documented-by-implication route. | UNDOCUMENTED |
| B6 | Query status (CAP-06) | Only through events and the on-disk transcript; no separate status command documented. | UNDOCUMENTED |
| B7 | Headless permissions | Documented: shell commands default to Ask and are **soft-denied** (run continues, exit 0) unless granted via `permissions.allow` in `~/.gemini/antigravity-cli/settings.json` or `--dangerously-skip-permissions`; workspace reads/writes auto-allowed. Reported behavior contradicts this (section K). | RUNTIME |
| B8 | Concurrency limit (CAP-07) | Not documented. Antigravity has its own subagent parallelism and subagents inherit the main agent's permissions. | UNDOCUMENTED |
| B9 | Account use for external orchestration | A forum answer (author not identified as an official Google employee on the page) says launching the official `agy` binary as a child process with cached credentials is supported and consumes the same entitlements; extracting OAuth tokens or calling backends directly is not. Google's maintainer statement (2026-02-27) prohibits third-party tools from using Antigravity/Gemini CLI OAuth to access backend services and points to Vertex/API-key billing. | VERIFIED (prohibition) / UNDOCUMENTED (official standing of the orchestration answer) |

## C. Antigravity SDK (spec: "SDK if available")

| # | Finding | Class |
|---|---|---|
| C1 | A Python SDK exists (`pip install google-antigravity`, Apache-2.0, positioned as a Research Preview). It gives "the same agent runtime that powers Antigravity 2.0 and the Antigravity CLI". Platform-specific compiled binaries are bundled; Windows support is not stated. | VERIFIED (exists) / UNDOCUMENTED (Windows) |
| C2 | Policies `allow()`, `deny()`, `ask_user(handler)`, `enforce()`; a host callback `handler(tool_call) -> bool` can approve or deny each tool call programmatically; default `confirm_run_command()`. Hooks span nine lifecycle points. | VERIFIED |
| C3 | Authentication: API key or Vertex AI / ADC. The SDK pages do not describe use of a consumer Google account. | VERIFIED |
| C4 | Conversation resume by id, cancellation, concurrency: not documented on the official overview; only third-party summaries mention resume. | UNDOCUMENTED |

## D. Antigravity Desktop app (the surface the spec's UI automation assumes)

| # | Finding | Class |
|---|---|---|
| D1 | No documented programmatic API, CLI, or automation interface for driving Desktop conversations. Remote Control (`/remote-control`) lets the Desktop UI interact with CLI sessions via a tunnel, signed in to the same account; it is a viewing/control channel, not an orchestration API. | UNDOCUMENTED |
| D2 | Desktop 2.x (latest v2.19.1, 2026-09-30) has native Git worktree support, subagent worktrees, scheduled tasks, plugins, hooks. Antigravity creating its own worktrees can overlap with Vela's worktree management. | VERIFIED |
| D3 | UI framework and UI Automation accessibility tree of the Desktop approval card: not documented by the vendor. Secondary sources describe the earlier Antigravity IDE as an Electron/Chromium VS Code fork. Third-party tools drive it through Chrome DevTools Protocol; these are unofficial and carry the policy risk noted in B9. | UNDOCUMENTED / RUNTIME |
| D4 | Approval prompts: "interactive card appears in your editor"; terminal commands in permission requests are syntax highlighted (v2.18.1); prompts describe the specific action (v2.12.0). Scope editing is not available for terminal commands. | VERIFIED |

## E. Hooks (a native policy interposition point)

| # | Finding | Class |
|---|---|---|
| E1 | `hooks.json` at `.agents/hooks.json` (workspace), `~/.gemini/config/hooks.json` (global), or plugin. Events `PreToolUse`, `PostToolUse`, `PreInvocation`, `PostInvocation`, `Stop`. Only handler `type` is `command` (shell command; JSON on stdin, JSON on stdout; 30 s default timeout). Available in Antigravity 2.0, CLI, and IDE. | VERIFIED |
| E2 | `PreToolUse` input: `conversationId`, `workspacePaths`, `transcriptPath`, `modelName`, `toolCall {name,args}`, `stepIdx`. Output: `decision` in `allow\|deny\|ask\|force_ask\|deny_unless_prior_grant`, optional `reason`, optional `permissionOverrides`. This supplies a pre-execution structured operation and a correlation identity (conversation, workspace). | VERIFIED |
| E3 | Whether a hook `allow` bypasses a permission prompt, whether `deny` always overrides grants, and fail-open versus fail-closed on hook crash or timeout are **not documented**. | UNDOCUMENTED |
| E4 | Open bugs: hook `allow` does not allow the call in headless mode (#1053, Windows 11, CLI 1.2.7); `ask` with `permissionOverrides` is ignored (#1059, macOS, 2026-09-19). Reporter analysis: hooks act as a restriction layer (deny overrides) but `allow` does not satisfy the grant requirement. | RUNTIME |
| E5 | Workspace hook and settings files live in the workspace, which the agent can write. A hook that enforces policy there is agent-modifiable unless protected; the global file is outside the workspace (`allowNonWorkspaceAccess` defaults off) but also affects the user's own sessions. | RUNTIME (design consequence) |

## F. Sessions, recovery, workspace trust (CAP-10, `RECOVERY.md`)

| # | Finding | Class |
|---|---|---|
| F1 | CLI conversations are persisted, **workspace-scoped** (bound to the working directory), and survive process termination and reboots; resume with `--continue`/`-c` or `--conversation <id>`; `/resume`, `/fork`. | VERIFIED |
| F2 | Transcript on disk: `<app_data_dir>/brain/<conversationId>/.system_generated/logs/transcript.jsonl` (CLI app data dir `~/.gemini/antigravity-cli`). | VERIFIED |
| F3 | Headless runs are stateless unless resumed by id. Open bug: headless resume can return a historical quota error after a successful turn (#1048). | VERIFIED / RUNTIME |
| F4 | Each new workspace must be explicitly trusted in the CLI; trusted workspaces are recorded in `~/.gemini/antigravity-cli/settings.json` (codelab). Each Vela-created worktree is a new workspace path; how headless runs treat an untrusted workspace is not documented. | UNDOCUMENTED |
| F5 | Subagents inherit tool configuration and permissions; subagent worktrees are handled by Antigravity (v1.2.10, v2.18.1 fixes). | VERIFIED |

## G. Capacity and rate limits (`CAPACITY_AND_PROFILES.md`, FR-024)

| # | Finding | Class |
|---|---|---|
| G1 | Quota exhaustion surfaces as `RESOURCE_EXHAUSTED (code 429): Individual quota reached ... Resets in Xh Ym Zs` (human text). The reset time is not a documented machine-readable field. | VERIFIED (text) / UNDOCUMENTED (structured) |
| G2 | Open bug #1018 (Windows 11, CLI 1.2.2): headless runs retry a multi-hour quota error until `--print-timeout` and then exit with code 0. Exit code 0 does not mean success. | RUNTIME |
| G3 | Plans differ: Enterprise/Business have no per-model quotas (v2.14.0); a "Use G1 Credits" fallback exists; `GEMINI_API_KEY` sessions stop on exhausted daily quota (v1.2.12). Do not hard-code a window. | VERIFIED |

## H. Matt Pocock skills (`MATT_POCKOCK_SKILLS.md`)

| # | Assumption | Finding | Class |
|---|---|---|---|
| H1 | `implement`, `code-review`, `implement-spec` exist | All present under `skills/engineering/`; also `to-tickets`, `to-spec`, `setup-matt-pocock-skills`, `triage`, `tdd`, `pr`. `to-tickets` exists (A6 resolved). | VERIFIED |
| H2 | `implement` flow | Implement, TDD at pre-agreed seams, typecheck and single-file tests regularly, full suite once, invoke `/code-review`, **then commit to the current branch**. Input is "a spec or set of tickets", not specifically an issue URL. How it chooses the review fixed point is not stated. | VERIFIED / UNDOCUMENTED (inputs, fixed point) |
| H3 | `code-review` fixed-point behavior | Fixed point is required (SHA/branch/tag); validates `git rev-parse` and a **non-empty** diff; diff is `git diff <fp>...HEAD` (three-dot) and `git log <fp>..HEAD`; spawns parallel Standards and Spec sub-agents; presents `## Standards` and `## Spec` verbatim plus a one-line summary. | VERIFIED |
| H4 | Reviewer reports severity and structured output (`REVIEW_PROTOCOL.md`, `PROMPT_CONTRACTS.md`) | **Not produced.** Standards findings are "hard" vs "judgement calls"; Spec findings are missing/partial/scope-creep/incorrect. There is no severity scale and no machine-readable output. | UNSUPPORTED |
| H5 | Review sees uncommitted work (ADR-005 premise) | Confirmed premise: three-dot diff cannot see uncommitted changes; `implement` reviews before it commits. | VERIFIED |
| H6 | `implement-spec` behavior | Task graph from tickets, frontier, implementer subagents in their own worktrees/branches, integration branch, merger subagents, optional draft PR, final `code-review`, worktree cleanup. Described as a Claude Code skill using subagents; implementers "reset" to the integration branch if needed. Vela does not use it at runtime. | VERIFIED |
| H7 | Installation and setup | `npx skills@latest add mattpocock/skills`, Claude plugin, or `/setup-matt-pocock-skills`. The setup skill edits `CLAUDE.md` if present (else `AGENTS.md`) and writes `docs/agents/issue-tracker.md`, `docs/agents/domain.md`, and optionally `docs/agents/triage-labels.md`. Default domain layout expects `GLOSSARY.md` and `docs/adr/`; Vela uses `docs/decisions/`. | CHANGED |
| H8 | Agent support | The repository README names Claude Code and Codex and "any model"; Antigravity is not named. Antigravity reads `SKILL.md` skills from `<workspace>/.agents/skills/` and treats skills as slash commands; the `npx skills` installer supports `--agent antigravity`. | VERIFIED (formats compatible) / RUNTIME (skill behavior) |
| H9 | Skill install paths | Antigravity docs: global `~/.gemini/config/skills/` (2.0 and IDE), `~/.gemini/antigravity-cli/skills/` (CLI); installer reports global `~/.gemini/antigravity/skills/`; open installer issue #1851 (global install not populated for the CLI). Project-level `.agents/skills/` is common to all. | CHANGED / RUNTIME |
| H10 | Claude-Code-specific features | `implement-spec` and `code-review` rely on subagents; Antigravity has subagents, but whether these skills run correctly there is unproven. | RUNTIME |
| H11 | Version pinning | No release tags noted; the repository uses changesets and a changelog. Pin by commit SHA. | UNDOCUMENTED |

## I. Git (`GIT_WORKFLOW.md`, ADR-003, ADR-013)

| # | Finding | Class |
|---|---|---|
| I1 | A branch cannot be checked out in two worktrees unless forced (`--force`). | VERIFIED |
| I2 | Verbatim: "all refs starting with `refs/` are shared" except `refs/bisect`, `refs/worktree`, `refs/rewritten`; pseudo refs (HEAD) and the index are per-worktree. Therefore `refs/stash` is shared, confirming the ADR-003 caveat. (An intermediate summary claimed a per-worktree stash; the verbatim text refutes it.) | VERIFIED |
| I3 | `worktree remove` refuses unclean trees unless forced; `lock` prevents pruning/moving; `prune` and `repair` exist. | VERIFIED |
| I4 | Git warns that "multiple checkout in general is still experimental, and the support for submodules is incomplete" and that checking out a superproject multiple times is not recommended. Preflight does not check for submodules. | CHANGED (new constraint) |
| I5 | Hooks run from `$GIT_DIR/hooks` or `core.hooksPath`; `--no-verify` bypasses pre-commit, commit-msg, and pre-merge-commit but not `prepare-commit-msg`. Whether linked worktrees share the hooks directory is not stated on that page. | VERIFIED / UNDOCUMENTED |
| I6 | Git documents "protected configuration" (system, global, command scopes): some options that execute programs are honored only from protected scopes. The full list of repository-local keys that can execute code is not enumerated there, so hardened invocation for untrusted repositories needs a pinned-Git-version review. | UNDOCUMENTED (list) |

## J. Tauri, Windows platform

| # | Assumption | Finding | Class |
|---|---|---|---|
| J1 | Tauri stable | Tauri 2.12 announced 2026-09-26; crate 2.12.1 shown 2026-09-30. | VERIFIED |
| J2 | Tray (reopen/status/Stop All) | `tray-icon` feature, `TrayIconBuilder`, menu and click events. | VERIFIED |
| J3 | Keep running after last window closes | `RunEvent::ExitRequested` exists (code `None` for user-initiated exit) with `ExitRequestApi`; the fetched pages did not document the prevent-exit call or its emission trigger. | UNDOCUMENTED (verify in a Phase 0 spike) |
| J4 | Login auto-start | `autostart` plugin: enable/disable/isEnabled; Windows supported; Rust 1.77.2 or newer. | VERIFIED |
| J5 | Signed updates, never mid-run | Updater requires signature keys; "on Windows the application is automatically exited when the install step is executed", with passive/basicUi/quiet modes. Consistent with FR-047. | VERIFIED |
| J6 | Installer and WebView2 | NSIS (`-setup.exe`, per-user, per-machine, or user choice) and WiX MSI; WebView2 modes: download bootstrapper (default), embed bootstrapper, offline installer (~127 MB), fixed version (~180 MB), skip. Evergreen runtime is included in Windows 11; Windows 10 may lack it. A long-running app keeps using the old runtime until restarted (`NewBrowserVersionAvailable`). | VERIFIED |
| J7 | CSP and capabilities | CSP must be explicitly configured in `tauri.conf.json`; remote content such as CDN scripts is an attack vector; a capabilities/permissions model exists (not read in detail). | VERIFIED |
| J8 | UI Automation limits | A client without the UIAccess manifest flag runs at medium integrity and "cannot access elevated process UI"; the logon screen and UAC dialogs run at higher integrity to prevent access; UIAccess requires Authenticode signing, a secure install location, and is not meant for non-assistive-technology applications. Consistent with ADR-012 (no bypass of lock or secure desktop, no elevation). | VERIFIED |
| J9 | Keep-awake | `SetThreadExecutionState` with `ES_CONTINUOUS` plus `ES_SYSTEM_REQUIRED` keeps the system out of idle sleep; it "cannot be used to prevent the user from putting the computer to sleep" (lid, power button); it does not stop the screen saver. Whether a display-off timeout leads to a locked session depends on the user's policy and is not documented here. | VERIFIED / RUNTIME |

## K. Open issues that contradict documentation (official CLI repository)

-   #548 (open, Windows 11): `--print` mode ignores `permissions.allow`; the process stalls indefinitely
    and does not honor `--print-timeout`; "persist to settings.json" writes nothing.
-   #1053 (open, Windows 11, CLI 1.2.7): hook `allow` is auto-denied in headless mode.
-   #1054: `autoExecutionPolicy: AUTO` has no effect in headless mode.
-   #1059 (open, duplicate, macOS): hook `ask` with `permissionOverrides` is ignored.
-   #1114: on Windows 11, `sudo` hangs headless because `run_command` lacks ConPTY allocation.
-   #1018 / #1048: headless quota retry loop and a historical quota error on resume.
-   Changelog fixes still landing for headless: exit codes (v1.2.8, v1.2.10), leaked daemon processes after
    exit (v1.2.9), background tasks cancelled about 5 seconds after idle (v1.2.9).

Interpretation: the headless surface is documented and moving fast but not yet reliable for unattended
policy-gated runs. Vela must treat it as RUNTIME and probe, exactly as the specification already requires.

## L. Consequences for the specification (queued; not applied in Prompt 4)

Prompt 4 changes research documents and state only. These need decisions or a specification pass before
Prompt 5 freezes the adapter:

1.  **[RESOLVED by human decision DR-1, recorded in ADR-016; capability claims still gated by probes.]
    Antigravity surface decision.** The specification's UI-automation design
    assumes the Desktop GUI, which has no documented programmatic interface. The CLI headless mode and
    the SDK are documented programmatic surfaces. Which surface(s) `AntigravityAdapter` targets, and
    which surface the user's observed persistent prompts occurred in, must be decided.
2.  **Native Permission Posture mechanism candidates (ADR-009).** Native allow/deny/ask rules with
    Deny > Ask > Allow precedence and `PreToolUse` hooks are documented, supported ways to place Vela's
    policy before execution. Neither is proven (E3, E4, K). A Phase 0 spike must test them on the
    installed version before any claim of `MEETS`.
3.  **Review contract (`REVIEW_PROTOCOL.md`, `PROMPT_CONTRACTS.md`).** `/code-review` produces no severity or
    structured output (H4). Vela must obtain structure through its own prompt or `--json-schema`, or
    classify itself.
4.  **Fixed-point effect of merge-based updates (ADR-011).** `git diff <fp>...HEAD` shows everything reachable
    from HEAD and not from the fixed point. After the integration tip is merged into a worker branch,
    sibling tickets' changes appear in that diff. The review that follows a conflict-resolution attempt
    needs an explicit fixed point (for example the pre-merge head).
5.  **Workspace trust per worktree (F4).** New Vela worktrees are new workspaces for the CLI. How trust
    is established without prompting, and whether that edits user-global settings, needs a decision under
    the consent principles of ADR-010 and ADR-013.
6.  **[RESOLVED by human decision DR-3, recorded in ADR-016 and FR-049; the official standing of external
    orchestration remains unconfirmed by a vendor source.] Account and credential policy (B9).** Use of the official binary with the user's cached credentials
    versus API-key/Vertex billing needs a user decision; Vela must never read or reuse agent OAuth tokens.
7.  **Worktree overlap (D2).** Antigravity can create worktrees itself; Vela's worktree contract must
    state that Vela-created worktrees are the only execution workspaces.
8.  **Preflight additions:** submodule detection (I4), Git-version check for protected-config behavior (I6),
    skills installed at project level versus global (H9), WebView2 runtime refresh for a long-lived background
    process (J6).
9.  **Skills setup collisions (H7).** `/setup-matt-pocock-skills` edits `CLAUDE.md`/`AGENTS.md` and writes
    `docs/agents/*.md`; under `PREFLIGHT.md` this is a bootstrap action on a separate branch.

## M. Resolution of the eight verification items in `EXTERNAL_INTEGRATIONS_2026-10.md`

1.  Native modes that satisfy NPP: **not yet determinable**; candidates A3 and E; probes required.
2.  Capability contract (CAP-01..11) availability: CAP-01..03 VERIFIED for the CLI (B1-B4 with RUNTIME on
    skills); CAP-04 partially (B3); CAP-05, CAP-06, CAP-07 UNDOCUMENTED; CAP-08 supplied by hook input and
    `init` event; CAP-09 text only (G1); CAP-10 VERIFIED for the CLI (F1); CAP-11 inspect via settings and
    `init.permission_mode`. Desktop: none documented (D1).
3.  Skills invocation: formats compatible (H8); programmatic invocation in print mode is secondary-source
    only (B4).
4.  Structured events for EBR-1: tool steps and PreToolUse input exist (B3, E2); approval-request events do
    not (B3).
5.  UI Automation surface of the Desktop approval card: UNDOCUMENTED (D3); platform limits VERIFIED (J8).
6.  Rate-limit surfacing: G1, G2.
7.  Session resume: VERIFIED for the CLI (F1); UNDOCUMENTED for the SDK (C4).
8.  Tauri tray, autostart, updater, installer: VERIFIED (J2, J4-J6); prevent-exit (J3) and keep-awake via Win32 (J9)
    need a spike.

## N. Probes required (Phase 0, real installed environment)

P1 Headless permission behavior with and without `permissions.allow` and hooks; P2 hook `allow`/`deny`/`ask`
semantics and fail-open versus fail-closed on crash and timeout, in interactive and headless; P3 whether
workspace and global settings and hooks are agent-writable; P4 skill invocation in print mode and in a
Vela-created worktree; P5 workspace-trust behavior for a new worktree in headless mode; P6 `WAITING` status
meaning and cancellation behavior; P7 Desktop approval-card UIA tree and process/window identity (if the
Desktop surface is chosen); P8 parallel `agy` processes and quota behavior; P9 Tauri prevent-exit with tray,
and `SetThreadExecutionState` under display-off and a lock policy; P10 Git hardened-invocation set against the
pinned Git version.

# Update after Human Decisions DR-1 and DR-3 and Real-Environment Probes (2026-10-02)

The product decisions in ADR-016 (primary surface: official `agy` CLI headless, conditional on probes; UI
Automation secondary for approval delivery; guarded visual last resort; SDK not a v1 dependency;
authentication owned by Antigravity) are **decisions, not evidence**. This section records what could be
probed in the real environment and keeps documentation claims separate from observed facts. It updates rows
D3, I6, J3, and C1 above.

Labels used here: `VERIFIED` observed in this environment (or confirmed by primary documentation **and**
observation); `PARTIALLY VERIFIED` observed in part or on a proxy; `UNVERIFIED` not observed and not
documented well enough to rely on; `UNSUPPORTED` evidence shows it does not satisfy the requirement.

## O. Environment under test

Windows 11 Home 10.0.26200; Git 2.45.1.windows.1; Node 24.19.0; WebView2 Runtime 154.0.4258.48; no Rust
toolchain; **Antigravity Desktop 2.17.0** installed (Electron/Chromium: bundled Electron licence file,
`language_server.exe`; latest published is 2.19.1) and an **Antigravity IDE** running (window class
`Chrome_WidgetWin_1`, not elevated). The `agy` CLI is **not installed** (no `agy` on PATH, no
`~/.gemini/antigravity-cli`, no CLI `settings.json`, no global `hooks.json`). No credential or conversation
files were opened. Installing and authenticating `agy` changes the machine and requires the user's
interactive sign-in, which Vela must never handle (ADR-016), so those probes were not run.

## P. Probe results

| Probe | Result | Label |
|---|---|---|
| P10 Git hardened invocation (Git 2.45.1) | Repo-local configuration executed code through `diff.external`, `diff.<driver>.textconv`, `filter.<driver>.clean/smudge`, `core.fsmonitor`, and hooks (`pre-commit`, `post-checkout`, `reference-transaction`). The invocation `-c core.fsmonitor=false -c core.hooksPath=NUL -c core.pager=cat -c diff.external= -c filter.<driver>.clean= -c filter.<driver>.smudge= -c diff.<driver>.textconv=` with `--no-pager --no-ext-diff --no-textconv` and commit `--no-verify` neutralized all of them. `core.pager` did not fire because output was not a terminal (not conclusive); `post-merge` was not exercised. A normal clone does not deliver a repository-local `.git/config`, so the exposure applies when a `.git` directory itself comes from an untrusted source. | VERIFIED (set above) |
| P9 keep-awake call | `SetThreadExecutionState` with `ES_CONTINUOUS` and `ES_SYSTEM_REQUIRED` succeeded and the clear call returned the prior state `0x80000001`. Actual prevention of idle sleep was not measured (`powercfg /requests` needs administrator rights). This machine uses Modern Standby (S0 low power idle); display-off is 300 s on AC and 180 s on battery; no secure screensaver or inactivity-lock policy is set (read-only registry). Whether "require sign-in" locks the session after display-off was not queried. | API VERIFIED; effect PARTIALLY VERIFIED |
| P7 UI Automation on an Electron/Chromium Antigravity window | Target was the **IDE, a proxy** (the Desktop app was not running and no approval prompt was showing). The process was not elevated. The first UIA query returned only 13 elements; once a UIA client had queried, Chromium populated the tree to about 600 elements (text, buttons, list items, tabs, tree items, 6 documents) and kept it populated for later clients. An adapter must therefore tolerate and retry an initially near-empty tree. No approval-keyword buttons existed to test `Invoke`. The Desktop approval card, its control identity, and session correlation inside the tree were not observed. (Side effect: this switched on Chromium accessibility in the running IDE until it restarts.) | PARTIALLY VERIFIED |
| P1 headless permission behavior | Needs installed and authenticated `agy`. | UNVERIFIED |
| P2 hook decision semantics and failure mode | Needs `agy`. Vendor documentation is silent on fail-open versus fail-closed and on `allow` versus prompts; open issues #1053 and #1059 report that `allow` and `permissionOverrides` do not work. | UNVERIFIED |
| P3 agent-writability of hooks and settings | Needs `agy`. | UNVERIFIED |
| P4 skill invocation in print mode and in a worktree | Needs `agy`; only a secondary source claims print-mode skill expansion. | UNVERIFIED |
| P5 workspace trust for a new worktree | Needs `agy`; only the codelab describes trust. | UNVERIFIED |
| P6 `WAITING` status and cancellation | Needs `agy`. | UNVERIFIED |
| P8 parallel `agy` processes and quota | Needs `agy`; quota is account-level. | UNVERIFIED |
| Desktop approval card (UIA tree, control identity, process/window/session correlation) | Needs the Desktop app with an agent action that requests approval, in the user's session. | UNVERIFIED |
| Tauri prevent-exit with tray | Documentation confirms `ExitRequestApi::prevent_exit` ("Prevents the app from exiting", ignored with `AppHandle::restart`); the emission condition is not documented; no Rust toolchain to run it. | Documentation VERIFIED; runtime UNVERIFIED |
| Official CLI release integrity | Latest manifest 1.2.14 (`windows_amd64`), served from Google storage with a SHA-512; the installer verifies it, installs per-user to `%LOCALAPPDATA%\agy\bin`, modifies PATH unless `--skip-path`, and states that the CLI self-updates in the background (disable or pin is not documented). The installer script itself is unsigned. **Integrity check performed:** the 1.2.14 `windows_amd64` binary (about 200 MB) was downloaded to a scratch directory and **not executed**; its SHA-512 **matches the manifest** and its Authenticode signature is **Valid**, signer `CN=Google LLC` (Mountain View). Version resource fields are empty (a Go binary), so the version comes from the manifest. | Release integrity VERIFIED; install, update behavior, and runtime UNVERIFIED |
| SDK as a v1 dependency | PyPI 0.1.20 (2026-09-27), "Development Status 3 - Alpha", Windows wheels exist, Python 3.10 or newer; official docs call it a Research Preview; authentication is API key or Vertex only; resume and cancellation are undocumented. Does not fit DR-3 or DR-1. | UNSUPPORTED for v1 |

## Q. Capability contract status on the primary surface (`ADAPTERS.md`, CAP-01..CAP-11)

| ID | Documentation | Real environment | Consequence if it stays unverified |
|---|---|---|---|
| CAP-01 discover installation and version | `agy` binary, manifest version 1.2.14 | PARTIALLY VERIFIED (Desktop 2.17.0 and IDE found; CLI not installed) | No claim of CLI readiness; preflight `BLOCK` if absent. |
| CAP-02 fresh isolated session in a worktree | cwd-bound `agy -p`, workspace-scoped conversations | UNVERIFIED | `BLOCK`; Antigravity end-to-end acceptance cannot pass. |
| CAP-03 deliver task and invoke skills | `-p`; skills as slash commands (print-mode expansion only secondary) | UNVERIFIED | `BLOCK`. |
| CAP-04 observe lifecycle | `stream-json` `init`/`step_update`/`result`; no approval event; `WAITING` undefined | UNVERIFIED | `BLOCK` without completion or error signal; approvals default to `ASK`. |
| CAP-05 cancel | statuses exist; no documented command | UNVERIFIED | Autonomous blocked; Supervised needs acknowledgement. |
| CAP-06 query status | events and transcript only | UNVERIFIED | Derived from CAP-04 or `BLOCK`. |
| CAP-07 concurrency limit | undocumented | UNVERIFIED | Vela runs one session at a time and reports it. |
| CAP-08 correlation identity | hook input (`conversationId`, `workspacePaths`), `init.conversation_id`/`cwd` | UNVERIFIED (CLI); window/process identity PARTIALLY VERIFIED (UIA proxy) | UI-automation delivery unavailable; approvals become interventions. |
| CAP-09 capacity signals | human-readable `RESOURCE_EXHAUSTED` text; open bug #1018 | UNVERIFIED | Pause on unclassified failure. |
| CAP-10 resume | `--conversation`, `--continue`; persisted across reboot | UNVERIFIED | Recovery uses a new session at the last checkpoint. |
| CAP-11 native permission posture | rules and `PreToolUse` hooks; semantics undocumented; bugs #548, #1053, #1059 | UNVERIFIED | Posture `UNKNOWN`: Autonomous blocked (ADR-009). |

Compatibility consequence (per `ADAPTERS.md` and ADR-016 section 5): no Required capability is verified on the
primary surface in the real environment. This record therefore declares **no capability ready**, supports
**no adapter workaround**, and makes the Phase 0 probes P1-P8 and the Desktop approval-card probe release-gating.
If a probe shows a Required capability unavailable, the outcome is a specification-change decision (ADR), not
a substitution.

## R. Open design question exposed by verification

For a **headless** session there is no GUI approval card: documented behavior is a soft-denial of tools that
need approval. The UI-automation tiers act only on a Desktop-hosted prompt. How approvals reach Vela for
Vela-created primary-surface sessions (native rules, `PreToolUse` hooks, or a Desktop-hosted session) must be
settled by probes P1, P2, and the Desktop approval-card probe, then by Prompt 5. The `/remote-control`
mechanism (Desktop UI controlling a CLI session through a tunnel, same Google account) exists but is only
documented for an interactive session.

## S. Procedure for the probes only the user can run (non-destructive)

Run in a throwaway Git repository outside OneDrive (for example `C:\vela-probe`), after installing `agy` with
the official installer and signing in through its browser flow yourself. Record results in this document.

1.  `agy --version` and `agy --help`; note the version and whether a background updater process remains after exit.
2.  **P1:** `agy -p "Run the shell command: git status" --output-format stream-json --print-timeout 2m`, with
    no allow rules; record behavior (soft-deny, hang, prompt), exit code, and `result.status`. Repeat after
    adding an allow rule for that command in the CLI settings.
3.  **P2:** add a workspace `.agents/hooks.json` with a `PreToolUse` command hook that appends its stdin JSON to a
    file and returns `allow`, then `deny`, then `ask`; repeat with a hook that exits non-zero and one that
    exceeds its timeout. Record whether the tool ran in each case, headless and interactive.
4.  **P3:** ask the agent to edit `.agents/hooks.json` and the workspace settings; record whether a prompt,
    soft-denial, or silent write occurs.
5.  **P4:** create `.agents/skills/probe/SKILL.md` (a one-line instruction) and run `agy -p "/probe hello"`.
6.  **P5:** `git worktree add ..\vela-probe-wt1`, run `agy -p "echo ok"` there for the first time; record any trust
    prompt or hang.
7.  **P6:** start a long command, terminate the process, and resume with `--conversation <id>`; record statuses
    (`WAITING`, `INTERRUPTED`) and what causes `WAITING`.
8.  **P8:** run two headless sessions in two worktrees at once; record behavior and quota effects.
9.  **Desktop approval card (Desktop 2.17.0 or newer):** trigger a command that prompts in the Desktop app and,
    with a read-only UIA inspection, record the card's control types, names, and whether it exposes an `Invoke`
    pattern. Do not click anything for the probe.

# Section S Probe Results (executed 2026-10-02, after the user installed and used Antigravity)

Environment: `agy` 1.2.14 installed per-user by the official installer (PATH updated; the installer's
setup step logs to stderr and is not an error); Antigravity Desktop 2.17.0; Windows 11 Home 10.0.26200. All
headless probes ran in a throwaway repository `C:\vela-probe` (outside OneDrive; the Vela repository was not
touched). The CLI authenticated by itself from the existing Antigravity sign-in (the OS credential store); no
credential or token material was read, and a few thousand tokens of the user's quota were used. The CLI created
the registry entry `CLI Project` (`~/.gemini/config/projects/default-cli-project.json`, written at the time of the
first headless run). Why the Desktop app opened by itself during the probes is **unexplained**.

## T. Results

| Probe | Evidence | Label |
|---|---|---|
| S1 version and flags | `agy --version` = 1.2.14. Flags include `--print`, `--output-format text\|json\|stream-json`, `--input-format`, `--json-schema`, `--conversation`, `--continue`, `--dangerously-skip-permissions`, `--sandbox`, `--add-dir`, `--project`, `--mode accept-edits\|plan`, `--disable-slash-commands`, `--remote-control`, `--print-timeout` (**default 0s, not 5m as documented**). Subcommands: `update`, `remote-control` (daemon "not registered" after install), `models`, `mcp`, `plugin`. No `agy` background process remained after headless runs. | VERIFIED (CHANGED vs docs for the timeout default) |
| P1 headless permissions, no rules | Command tool is **soft-denied**: exit code 0, `result.status` `SUCCESS`, empty response, denial text only on stderr ("a tool required the 'command' permission that headless mode cannot prompt for"). The tool step still reports state `DONE`. No hang. `init.permission_mode` = `request-review`. | VERIFIED |
| P1b user-level allow rules | With `permissions.allow` in the user-level CLI `settings.json`: exact `command(git log --oneline -1)` ran with real output; `command(regex:git (status\|log\|diff)( .*)?)` allowed `git diff` and denied `git branch`; `write_file(C:\vela-probe\wt2)` allowed a write inside and denied a write outside. Bug #548 (rules ignored in headless) did **not** reproduce on 1.2.14. A repo-level `.gemini/config.json` allow rule was **not honored**. Settings could be isolated by pointing `USERPROFILE` and `HOME` at another directory for the child process; authentication kept working (credential store), so Vela can give each run its own settings without editing the user's files. Documentation said workspace writes are auto-allowed in headless mode; observed, even ordinary workspace writes were denied without a rule. | VERIFIED (docs CHANGED) |
| P2 hooks | Workspace `.agents/hooks.json` is loaded in headless mode. `PreToolUse` input carried `toolCall.args.CommandLine`, `Cwd`, `conversationId`, `stepIdx`, `modelName`, `artifactDirectoryPath`. `deny` = hard block, **even under `--dangerously-skip-permissions`**. An empty `{}` response is treated as deny. A crashing hook (exit 1), non-JSON output, and a hook that exceeded its 8 s timeout each stopped the tool from running (**fail-closed**). `allow` **cannot grant permission**: without a matching permission rule the command is still denied (bug #1053 reproduced on 1.2.14, Windows); `ask` is denied because headless cannot prompt. Under skip-permissions an `allow` hook let the command run. | VERIFIED |
| P3 agent edits to policy files | In headless mode without a write rule the agent could not write `.agents/hooks.json` or any ordinary workspace file (`write_file` denied); `.gemini/config.json` was not changed. A path-scoped `write_file` allow rule covering a worktree allowed writes inside it, so policy files placed **inside** an allowed worktree would be writable (not tested on `hooks.json` directly). The agent can read `hooks.json`. | PARTIALLY VERIFIED |
| P4 skills in print mode | `.agents/skills/probe/SKILL.md` was resolved by `-p "/probe hello"` and the reply was `SKILL-PROBE-OK`; with `--disable-slash-commands` the model still found the skill by itself. Behavior of the Matt Pocock skills themselves was not tested. | VERIFIED (mechanism) |
| P5 workspace trust, new worktrees | Brand-new worktrees (and the repo itself) ran headless with no trust prompt and no hang, for tool-free and tool-using prompts. Interactive and Desktop trust behavior not tested. | VERIFIED (headless) |
| P6 kill and resume | A long command was killed mid-run (process tree terminated; exit code 1; no orphans). `run_command` executes through a child **`powershell.exe`**. Resuming with `--conversation <id>` kept the same conversation id and context (`turns=2`). The `WAITING` status was never observed in headless runs. | VERIFIED (kill, resume); `WAITING` UNVERIFIED |
| P8 parallel sessions | Two headless sessions in two new worktrees ran concurrently and both succeeded (about 16 s wall time for 3 s of model time, so startup overhead is large). No quota error, no leftover processes. The concurrency ceiling and quota effects at scale are unknown. | PARTIALLY VERIFIED |
| Desktop approval card (P7 on the real Desktop 2.17.0; delivery tested later, see section V) | The Desktop window is `Chrome_WidgetWin_1` (Electron), not elevated. UIA returned 13-14 elements until the window was a normal visible foreground window (a minimized window reports an empty rectangle), then about 200 after a few seconds. The card is **inside the window** (a DOM card, not a separate window). It exposes: title "Allow checking git status?", the working directory (`...\vela-probe\wt1`), the command split into text fragments (`git`, ` status `, `--short`), the status text "Waiting for user input", an "Edit permission target" edit box, five radio options with `InvokePattern` and `SelectionItemPattern` (`ask-opt-:<session-id>:-1` "Yes, allow this time"; `-2` always allow in this conversation; `-3` always allow in this project; `-4` always allow (no scope); `-__write_in__` "No (tell the agent what to do instead)"), a write-in edit box, and `Skip` and `Submit` buttons. Option 1 is selected by default. Clicking or invoking was **not** tested. The user reports that the only policy setting visible in Desktop 2.17.0 is Plan Review Policy, set to Always Proceed, and that the card appears anyway. | VERIFIED (structure, read-only); delivery UNVERIFIED |

## U. Does anything change the specification or block Prompt 5?

**No result blocks Prompt 5.** Several results change what the specification should say; they are recorded here
and queued, not applied, because this phase authorizes research-record and state updates only.

1.  **Native permission posture (ADR-009) now has a verified candidate for the primary surface.** Headless
    `permissions.allow` rules (exact, regex, path-scoped) work, and a `PreToolUse` command hook gives a
    pre-execution, fail-closed `deny` layer. A hook cannot grant `allow`, so ALLOW must come from allow rules
    that Vela generates from the confirmed command profile and worktree path (ADR-013), placed in an isolated
    per-run profile outside the worktree; everything else is blocked and visible (hook input, step errors,
    stderr). Whether this satisfies NPP-1..3 in full is a Prompt 5 design question; Autonomous mode remains blocked
    until the posture check actually runs against it.
2.  **EBR-1(a) is available for the CLI** through hook input (command line and working directory before execution).
    For the Desktop card the evidence is the UIA subtree only, with the command text split across fragments that an
    adapter must reassemble and treat as incomplete if anything is truncated (EBR-3).
3.  **Error handling:** exit code 0 and `result.status = SUCCESS` do not mean a tool ran or that a task succeeded;
    a tool step can be `DONE` after a soft denial. Vela must inspect stderr and step errors. (Affects
    `ERROR_HANDLING.md`, `ORCHESTRATION_ENGINE.md`.)
3a. **Quota and timeouts:** `--print-timeout` defaults to 0s (wait for the turn); Vela must set explicit timeouts.
4.  **UIA adapter requirements (queued for `APPROVAL_BROKER.md`):** warm-up and retry while the tree populates; the
    window must be a visible, normal window; option identifiers contain a session-unique part, so match by role and
    label, not by id; the card offers **persistent "always allow" options (2-4)**. Vela must only ever select the
    single-use allow (1) or the refusal; selecting 2-4 would create persistent permissions and conflicts with the
    rule that Vela never infers permanent allow rules.
5.  **Shell:** the agent's commands run through PowerShell, which conflicts with the "Command Prompt preferred"
    statements (`CONSTRAINTS.md`, `GEMINI.md`; finding SA-36). Policy normalization must handle PowerShell syntax.
6.  **Documentation conflicts to record in the research snapshot:** `--print-timeout` default; workspace writes in
    headless; repo-level settings not honored; Desktop 2.17.0 exposes only Plan Review Policy (user report) while the
    permissions page describes allow/deny/ask rules for Desktop.
7.  **DR-1 is consistent with the evidence:** the CLI headless path can enforce policy without UI automation; UIA applies
    to Desktop-hosted prompts. The design question in section R is narrowed, not closed: how approvals for
    Desktop-hosted sessions are reached when Vela creates sessions through the CLI is not needed, because CLI sessions
    do not raise Desktop cards.

## V. UIA delivery test on the pending Desktop card (executed 2026-10-02 17:03 +05:00, with user consent)

Target: the card for the harmless request `git status --short` in `C:\vela-probe\wt1` (Antigravity Desktop 2.17.0,
process 61044, not elevated, visible foreground window). Method: Windows UI Automation patterns only (no
coordinate or pixel clicking, no keystrokes).

1.  **Correlation before acting (all required, fail-closed):** exactly one control named "1 Yes, allow this time" and
    exactly one "Submit" button in the verified window; card title "Allow checking git status?"; command fragments
    `git`, ` status `, `--short`; working directory `…\vela-probe\wt1`; status text "Waiting for user input";
    exactly five options, with option 1 the selected one and no other selected.
2.  **Actions:** `SelectionItemPattern.Select()` on option 1 (read back `IsSelected = True`, others unselected), then
    `InvokePattern.Invoke()` on `Submit` (17:03:18.158 +05:00). Options 2-4 (persistent "always allow") and the "No"
    option were never selected.
3.  **Result:** the card and the "Waiting for user input" status disappeared within six seconds; the Desktop
    conversation showed the agent's report "The working tree is clean (no output returned from git status --short)".
    **Independent evidence:** the conversation transcript (`.../brain/<id>/.system_generated/logs/transcript.jsonl`)
    records the request as step 1 `run_command` with `CommandLine "git status --short"` and
    `Cwd "c:\vela-probe\wt1"`, and the agent's next step (step 3) is created at `12:03:18Z`, the second Submit was
    invoked. The transcript does not record an exit code, so execution is inferred from the agent's report and the
    resumption timing.
4.  **No persistent permission created:** a SHA-256 snapshot of every file under `~/.gemini/config` (except plugin
    and sidecar caches) before and after showed **zero differences**; the new `wt1` project registry file contains
    only its name and folder URI (no permission keys); no CLI `settings.json`, no global `hooks.json`, and no
    workspace policy files were created.
4a. **Observed blocked time (user observation, confirmed by timestamps):** Antigravity displayed "worked for 19
    minutes". The transcript shows the request created at `11:44:17Z` (tool step at `11:44:20Z`) and the agent's next
    step at `12:03:18Z`, i.e. **19 min 1 s of the agent waiting on the unattended card**, ending in the same second as the
    `Invoke()` call; the user was only observing and did not interact. The 19 minutes is therefore approval wait time, not
    command execution time. An unattended Desktop prompt blocks the agent indefinitely with no visible timeout, which
    supports the Approval Watchdog and `APPROVAL_STALLED` requirements (`APPROVAL_BROKER.md` sections 8-9). The
    "card disappeared within six seconds" in item 3 is the probe's own re-read delay, not a card duration.
5.  **Limits:** one successful delivery of a single-use allow does not establish reliability across prompt variants,
    versions, window states, or multiple concurrent cards; the request text was a harmless command; the check that the
    command ran relies on the transcript and the agent report; a denial path (option 5) was not exercised.

Label: **UIA approval delivery to a Desktop card: VERIFIED for one correlated single-use allow on Desktop 2.17.0**
(structure, selection, invocation, progress, and no persistent side effect); broader reliability PARTIALLY VERIFIED.

Still unverified: `WAITING`
status; hooks and settings loaded from an isolated global location; interactive (TUI) behavior; background self-update
control; Desktop project permission presets; the vendor's stance on external orchestration.
