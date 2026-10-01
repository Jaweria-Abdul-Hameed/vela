# ADR-010: Guarded UI Automation Is Opt-In at Onboarding

## Status

Accepted (2026-10-02). Refines ADR-007; fulfils the opt-in condition stated in ADR-004's
exception.

## Context

ADR-004 permits UI automation only if "isolated, observable, opt-in, and fails closed".
ADR-007 requires the fallback but is silent on consent, and other documents disagree on whether
it is default-on (Prompt 2 finding SA-08).

## Decision

1. Guarded approval UI automation (Windows UI Automation tier and guarded visual tier) is
   **disabled until the user explicitly enables it during onboarding** (or later in Settings).
2. The enabling action presents what the feature does, its safety limits (policy first, correlated
   window/session only, bounded retries, fail closed), and that it covers the whole delivery chain
   (native, then UIA, then guarded visual).
3. Once enabled, the preference **persists**. Vela may automatically use the chain
   native -> UIA -> guarded visual according to policy without asking for feature consent on every
   run. Enabling does not relax policy: `ASK` and `DENY` classes remain enforced.
4. The preference is revocable at any time in Settings; revoking takes effect for new deliveries
   immediately and is journaled with the consent text version.
5. While not enabled, reading the accessibility tree and screen capture are part of the disabled
   capability, so the Approval Broker detects prompts only from non-UI sources (provider
   events, process/session state, and the progress watchdog) and classifies them; native
   delivery may be used; the watchdog may report a no-progress worker as a suspected approval
   block; remaining prompts surface as interventions, and preflight reports unattended readiness
   accordingly.
6. Consent is recorded in persistence (setting plus consent record) and is part of the run
   policy snapshot.

## Consequences

- Fresh installs are conservative. Unattended routine operation on Antigravity builds that still
  prompt requires one explicit onboarding choice.
- Onboarding becomes a required first-run surface (see `SCREEN_INVENTORY.md`).
