# Security and Permission Model

## Principle

Autonomy is bounded authority, not unlimited authority.

## Default allowed category examples

Within assigned worktree/project and policy, **in a `TRUSTED` repository** (ADR-013): - read/write
project files, - package/test/build commands that are in the user-confirmed command profile
(a script is not safe merely because it is named test or build), - non-destructive Git
inspection, - commits, - normal branch push to permitted remotes. In an `UNTRUSTED` repository
Vela performs data-only inspection and runs no repository-controlled executable.

## Ask/deny category examples

Ask: - operations outside workspace, - installing system-wide
software, - changing system configuration, - modifying credentials, -
destructive database/infrastructure operations.

Deny by default: - credential extraction, - disabling security
controls, - force-pushing protected branches, - deleting unrelated user
data, - arbitrary commands outside approved roots.

## Antigravity Windows

Current Antigravity documentation exposes Windows Terminal Command Auto
Execution choices including Request Review, Proceed in Sandbox, and
Always Proceed. Vela must not assume the same permission engine exists identically
on every OS/version, and **must not depend on, configure, or recommend an
unconditional auto-execution mode (such as Always Proceed) as its unattended mechanism**
(ADR-009): Vela's ALLOW/ASK/DENY policy must remain enforceable. Which native modes satisfy
the Native Permission Posture is verified by Prompt 4 and confirmed by Prompt 15; until then the
posture is `UNKNOWN`, which blocks Autonomous mode.

## Secret handling

-   Vela never reads, extracts, copies, exports, persists, or reuses Antigravity authentication tokens or
    credentials, including the OS credential store entries Antigravity creates (ADR-016),
-   no secrets in Markdown,
-   no raw tokens in SQLite logs,
-   redact command output,
-   use OS/provider credential mechanisms,
-   never commit `.env` secrets,
-   never automate password/2FA/CAPTCHA entry.

## Threat model

Consider: - malicious repository instructions, - prompt injection in
issue/docs, - dependency install scripts, - command injection through
ticket text, - path traversal, - symlink escapes, - malicious test
scripts, - accidental secret logging, - compromised external skill, -
**spoofed or misleading approval dialogs** (agent-authored content imitating a prompt, truncated
command text), - **repository-controlled execution** (Git hooks, fsmonitor, filter and diff
drivers, package scripts), - **untrusted content rendered in the privileged webview** (issue text,
Markdown, logs).

External text is data, not automatically trusted executable instruction. Repository, issue, and
`AGENTS.md` text is untrusted input to Vela and cannot change policy, trust, or approval
classification (ADR-013). Renderer hardening requirements are in `SYSTEM_ARCHITECTURE.md` section 5.

# Policy-Driven Auto-Approval Fallback

The user's observed Antigravity environment may continue requesting
approval despite permissive native settings. Therefore guarded
auto-approval is a required compatibility capability. It is **opt-in** at onboarding and persists
once enabled (ADR-010); policy, evidence-binding (ADR-009), and trust (ADR-013) apply in all cases.

Security invariant:

> **Vela approves operations, not buttons.**

The policy engine first decides whether the normalized operation is
allowed. Only then may an approval-delivery adapter interact with
Antigravity.

### Auto-allow examples under an appropriate project policy

-   project-scoped test/lint/typecheck/build commands that are in the user-confirmed command
    profile of a `TRUSTED` repository,
-   non-destructive repository inspection,
-   normal file edits inside the assigned worktree,
-   Vela-managed worktree/branch creation,
-   ordinary commits,
-   normal pushes to explicitly permitted worker/integration branches.

### Ask/deny examples

-   force push,
-   credential/token access,
-   password/2FA/CAPTCHA interaction,
-   deleting unknown/uncommitted user work,
-   commands outside allowed roots,
-   system-wide configuration,
-   destructive database/cloud actions,
-   ambiguous commands that cannot be normalized.

### UI automation security

Prefer Windows UI Automation/accessibility control identities,
window/process verification, and semantic labels. Pixel/coordinate
clicking is last resort and must verify process/window, expected visual
state, bounded coordinates, and post-action progress. On mismatch, fail
closed. Only sessions Vela created are eligible targets, a locked/disconnected/secure desktop is
never bypassed (ADR-012), and operation evidence follows EBR-1..6 (ADR-009).

# Antigravity-First Security Boundary

Threat modeling for v1 must prioritize the actual Antigravity control surface: process/session correlation, terminal execution, approval surfaces, Windows UI Automation, worktree/project boundaries, and prompt/untrusted-repository input.

Generic provider abstraction does not reduce the need for concrete Antigravity security tests.
