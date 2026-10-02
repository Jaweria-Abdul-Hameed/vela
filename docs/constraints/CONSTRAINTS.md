# Constraints

-   Windows is first release target.
-   Finished app must install/run natively; no user-managed localhost
    server.
-   Ordinary autonomous commands should not require constant approval
    once the user has selected an appropriate autonomy policy.
-   Command Prompt is preferred for Antigravity project terminal flows
    where PowerShell introduces avoidable permission friction. (Verified 2026-10-02: the Antigravity CLI agent's
    `run_command` executes through `powershell.exe`, and no setting to change that was found. Vela's policy normalizer
    therefore supports both PowerShell and `cmd` syntax, within a deliberately small grammar, and treats anything else as
    `ASK`; this preference is advisory for Antigravity terminal settings, not a requirement on Vela's own `ProcessRunner`.)
-   Parallel workers require worktree isolation.
-   Fresh context per issue.
-   Git and test evidence are mandatory.
-   UI must follow Stitch-inspired spatial design docs.
-   No provider password/2FA automation.
-   No quota-evasion design.
-   No hidden destructive operations.
-   External tool assumptions must be capability-detected and
    documented.

# Approval Constraint Update

The application must work in the user's observed case where Antigravity
continues to request approvals despite permissive settings. Native
permissions remain preferred but are not considered a complete solution
until effective behavior is verified. A guarded UI-automation fallback
is therefore in scope as a required capability for unattended execution. It is enabled only by an
explicit, persisted onboarding choice (ADR-010), and Vela does not depend on unconditional native
auto-execution (ADR-009).

# Runtime Scope Constraint

Vela v1 is constrained to an Antigravity-first production runtime strategy on Windows. Provider-extensible architecture is required; provider parity is not.

Any implementation plan that materially delays Antigravity reliability to implement unrequested Claude/Codex/other runtime adapters violates this constraint.
