# ADR-007: Native Approval First, Guarded UI-Automation Fallback

## Status

Accepted; supersedes the absolute interpretation of ADR-004.

## Context

The user's actual Antigravity 2.0 Windows environment continues to
surface approval prompts despite permissive native configuration.
Vela's core promise requires unattended routine execution.

## Decision

Vela separates **approval policy** from **approval delivery**.

1.  Use native Antigravity permission/approval mechanisms when
    effective.
2.  If a policy-allowed request still requires a UI approval, use
    Windows accessibility/UI Automation to locate and activate the exact
    correlated Antigravity control.
3.  Use visual/pixel automation only as a last-resort compatibility
    adapter.
4.  Unknown/sensitive/destructive requests remain human-gated.
5.  Repeated equivalent prompts trigger a bounded loop guard.

## Safety invariants

-   Vela approves an operation, not a button.
-   The adapter verifies Antigravity process/window/session identity.
-   UI changes fail closed.
-   No password/2FA/CAPTCHA automation.
-   No unbounded auto-click loops.
-   Every delivery attempt is auditable.
