# Spike S-VISUAL (SP13): is the guarded visual tier ever needed on the primary path?

**Recommendation: DEFER the visual module. Do not build it for v1; keep the feature-gated, consent-gated, disabled `vela-uia::visual` skeleton as a placeholder only.**

## Evidence

- The primary path is headless: `agy -p` with native permission rules and a `PreToolUse` hook. No Desktop card exists there (ADR-018 section 1), so no delivery tier is involved at all.
- The secondary path is UI Automation, and the one real observation (Desktop 2.17.0, `EXTERNAL_VERIFICATION_2026-10-02.md` sections P7 and V) found every control the adapter needs exposed with the right patterns: five radio options with `SelectionItem` and `Invoke`, `Submit` and `Skip` buttons, the command as text fragments, the status text. UIA was sufficient; nothing required pixels.
- The one failure mode seen on the real app was an initially near-empty tree and a minimized window reporting no rectangle. Both are handled in UIA terms (warm-up and retry; reporting `ApprovalSurfaceUnavailable` rather than restoring the window), exercised in `crates/vela-uia/tests/fake_approval.rs` (see `SPIKE_S-UIA-RELIABILITY.md`).
- A visual tier would act on coordinates against a layout fingerprint: exactly what ADR-018 forbids as a first resort, with the highest wrong-click risk (a persistent option sits a few pixels from the single-use one). Its fingerprints would need real-app captures, which do not exist, so it could not be validated.

## Experiments

Controlled captures on the fake window were not run: the fake is native Win32 and its pixels say nothing about the real card's layout, so such captures would produce fingerprints that look precise and mean nothing. That is the wrong kind of evidence to build a click-by-position tier on.

## Decision

- v1 ships UIA only. If a future Desktop version breaks UIA discovery, the response is `ApprovalSurfaceUnavailable` and a human answers the card; there is no pixel fallback.
- Re-open this spike only if real-app runs (with the user present) show UIA failing in a practical, repeatable way. The trigger and the evidence required are recorded here so the decision is not re-litigated without data.
