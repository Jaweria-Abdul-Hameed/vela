## K08 (guarded visual fallback module)

K08 is **closed without building**: SP13 recommends deferring the visual tier, so there is nothing to implement. The `visual` feature and `vela-uia::visual` module stay as the empty, disabled, consent-gated skeleton. No code path selects the visual tier. Reason recorded: no evidence that UI Automation fails in a practical case, and no real-app captures from which a layout fingerprint could be validated.
