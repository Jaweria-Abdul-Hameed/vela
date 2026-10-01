# Security and Permission Model

## Principle

Autonomy is bounded authority, not unlimited authority.

## Default allowed category examples

Within assigned worktree/project and policy: - read/write project
files, - package/test/build commands, - non-destructive Git
inspection, - commits, - normal branch push to permitted remotes.

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
Always Proceed. Vela may guide users to an appropriate project-level
setup, but must not assume the same permission engine exists identically
on every OS/version.

## Secret handling

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
scripts, - accidental secret logging, - compromised external skill.

External text is data, not automatically trusted executable instruction.

# Policy-Driven Auto-Approval Fallback

The user's observed Antigravity environment may continue requesting
approval despite permissive native settings. Therefore guarded
auto-approval is a required compatibility feature.

Security invariant:

> **Vela approves operations, not buttons.**

The policy engine first decides whether the normalized operation is
allowed. Only then may an approval-delivery adapter interact with
Antigravity.

### Auto-allow examples under an appropriate project policy

-   project-scoped test/lint/typecheck/build commands,
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
closed.

# Antigravity-First Security Boundary

Threat modeling for v1 must prioritize the actual Antigravity control surface: process/session correlation, terminal execution, approval surfaces, Windows UI Automation, worktree/project boundaries, and prompt/untrusted-repository input.

Generic provider abstraction does not reduce the need for concrete Antigravity security tests.
