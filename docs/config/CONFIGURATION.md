# Configuration

Settings layers: 1. application defaults, 2. user global settings, 3.
project settings, 4. run snapshot.

A running build uses a snapshot so changing settings mid-run does not
silently rewrite policy unless the setting is explicitly live.

Categories: - appearance, - motion/graphics, - orchestration
concurrency, - review policy, - test commands, - Git behavior, -
tracker, - agent adapter, - autonomy/security, - notifications,
- diagnostics, - background and recovery, - promotion, - trust.

Recommended defaults: - graphics: Balanced, - review: Engineering, -
parallelism: conservative automatic, - destructive operations: Ask, -
Stop All confirmation: minimal but clear (inline, non-modal, may be turned off).

## Onboarding choices (first run)

Persisted user-global settings, each changeable later in Settings:

-   `background_operation`: off by default; enabled only by explicit choice (ADR-012);
-   `login_autostart`: off by default; opt-in (ADR-012);
-   `recovery_continuation`: `ask` (default) or `auto_safe` (ADR-012);
-   `approval_ui_automation`: off by default; opt-in with a recorded consent record (ADR-010).

## Project settings

Project settings, trust, command profiles, and policy rules are stored in Vela's own store, **never in the repository**, so
repository, issue, or `AGENTS.md` text cannot alter them (ADR-013, `IMPLEMENTATION_ARCHITECTURE.md` section 9).

-   trust state (`UNTRUSTED` default; ADR-013),
-   user-confirmed command profile (exact commands and working directories),
-   provisioning commands, explicit file-copy allowlist, shared-cache declarations, resource keys
    (`GIT_WORKFLOW.md`),
-   `promotion_mode`: `pr` (default), `local_only`, or opt-in auto-merge with warning (ADR-014).

## Initial defaults

These are initial defaults, configurable, to be validated by Prompt 4 (external verification) and
Prompt 15 (real-environment validation). They are not permanent product constants.

| Setting | Initial default |
|---|---|
| Maximum concurrent workers | 2, never above the Antigravity-declared limit (CAP-07) |
| Review maximum iterations | 3 |
| Approval repeat threshold | 3 consecutive, bounded 2..10 |
| Watchdog no-progress window (agent-driven states) | 120 seconds; suppressed while a tracked command runs |
| Stop All graceful wait | 30 seconds |
| State-store backups retained | 3 |
| Command timeout | per project command profile; no global default, a command without a timeout is not run unattended |

# Default Runtime Configuration

The default production runtime/provider for v1 is Antigravity. Configuration schemas may be extensible, but documentation and defaults must not present unsupported providers as selectable working alternatives.

Future provider values should remain hidden/unsupported until their adapter passes the required acceptance contract.
