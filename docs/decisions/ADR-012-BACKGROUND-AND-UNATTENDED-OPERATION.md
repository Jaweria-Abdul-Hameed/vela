# ADR-012: Background Operation and Unattended Session Conditions

## Status

Accepted (2026-10-02).

## Context

Prompt 2 findings SA-04, SA-17 and SA-18: "walk away" conflicts with Windows lock, sleep, and
display behavior; the default behavior when the window closes was undefined; reboot continuation
and tray behavior were unspecified.

## Decision

### Background operation

1. **Opt-in.** Background operation is enabled only by an explicit onboarding choice (changeable
   in Settings).
2. **When enabled**, closing the UI during an active run does not terminate orchestration. A
   system tray presence provides: reopen window, run status, and Stop All.
3. **When not enabled**, closing the window during an active run offers a safe pause (the default
   action) or cancel of the close; Vela never silently abandons running work.
4. **Login auto-start is opt-in** and independent of background operation.

### Reboot and restart

5. After a reboot or restart Vela reconciles first (see `RECOVERY.md`). It resumes only after
   successful reconciliation, and automatic continuation follows the persisted setting
   `recovery_continuation` (`auto_safe` or `ask`). The default is `ask`, offered during
   onboarding.

### Unattended session conditions

6. **Keep awake.** During an active autonomous run, Vela may hold a system-awake power request
   when required, releasing it when the run pauses, stops, or completes. Display-off is supported
   and is not a failure condition. Vela does not persistently alter the user's lock or power
   policy.
7. **No bypass.** A locked, disconnected, or secure desktop is not a boundary Vela attempts to
   circumvent. Vela does not try to unlock, switch desktops, or elevate to defeat these
   conditions.
8. **Degrade safely.** Work that depends on UI automation and cannot reach an interactive desktop
   pauses safely (worker `PAUSED`, reason `INTERACTIVE_SESSION_UNAVAILABLE`); unaffected workers
   continue. When an interactive environment returns, Vela reconciles: re-detects the prompt,
   re-verifies correlation and freshness, and re-evaluates policy before any delivery. A stale
   click is never replayed.
9. **Lid close and sleep** remain recoverable pauses (see `RECOVERY.md`); Vela cannot override
   OS power actions.
10. **Readiness reporting.** Preflight reports which of these conditions currently limit
    unattended operation.

### Process model

11. Orchestration lives in the Vela core process, independent of the renderer window. Whether it
    is a separate OS process or a Windows service is an architecture decision for Prompt 5, which
    must satisfy items 2, 3 and 5.

## Consequences

- Onboarding is mandatory first-run UI (background, auto-start, recovery continuation, approval
  UI automation consent).
- The "leave the computer" promise is bounded by the conditions above and is stated that way in
  user-facing copy.
