# GEMINI.md

Gemini/Antigravity must treat `AGENTS.md` as canonical.

-   Prefer supported Antigravity permissions/settings/CLI/SDK interfaces
    over UI coordinate automation.
-   On Windows, project bootstrap should detect the actual Antigravity
    permission capabilities available at runtime.
-   Use Command Prompt (`cmd.exe`) for project terminal operations when
    the configured environment requires it; do not silently switch to
    PowerShell if doing so introduces additional approval/policy
    friction.
-   Every parallel ticket must run in an isolated worktree/branch.
-   Fresh context per ticket is mandatory.
-   Persist material exploration and research so later sessions do not
    depend on hidden chat history.
-   Do not automate sign-in/password/2FA flows or use account rotation
    to evade service limits. Capacity/account handling must use
    supported authenticated profiles and provider policy.
-   Follow `AGENTS.md` for implementation, review, testing, Git, and
    completion.

# Antigravity Approval Compatibility

Do not assume `Always Proceed` eliminates all prompts. Vela must verify
behavior during preflight and runtime. When a prompt remains, route it
through the Vela Approval Broker. Supported native mechanisms are
preferred; guarded UI automation is a compatibility fallback. Never
blindly approve every visible prompt and never create an infinite
approve loop.

# Antigravity-First Scope

When working on Vela, treat Google Antigravity on Windows as the required v1 runtime environment. Provider-neutral interfaces must remain clean, but do not spend v1 implementation effort on Claude, Codex, or other runtime adapters unless an approved issue explicitly requires one.

Gemini/Antigravity, Claude, and Codex may all be used as development agents for this repository; that is separate from Vela's shipped runtime scope.
