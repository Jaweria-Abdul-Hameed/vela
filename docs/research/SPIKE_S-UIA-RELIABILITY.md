# Spike S-UIA-RELIABILITY (SP12): repeat and vary UI Automation approval delivery

**Result: MATRIX RECORDED against the fake approval window (executed 2026-10-15, Windows 11, real UI Automation binding). The real Desktop app was NOT exercised in this spike, so nothing below is a reliability claim about Antigravity Desktop.** The only delivery ever observed on the real Desktop (2.17.0) remains the single correlated single-use allow in `EXTERNAL_VERIFICATION_2026-10-02.md` section V.

## Method

`crates/vela-uia/tests/fake_approval.rs` drives the real `vela-uia` binding (`WinSurface`, `IUIAutomation`) against `crates/vela-uia/tests/fake_window`, a Win32 window that reproduces the verified card: title, working directory, command fragments, status text, five radio options (first selected by default, three persistent), write-in box, `Skip` and `Submit`, and an unrelated `Approve` button. Selection uses `SelectionItemPattern`, submission `InvokePattern`; there are no coordinates and no key presses. The fake records which options were checked when `Submit` was pressed.

The harness is native Win32, not WebView2 (a deliberate deviation from K04's wording, recorded there): the controls expose the same roles and patterns without a browser runtime, which keeps it deterministic. What it cannot reproduce is Chromium's lazy tree population, accessibility switching and DOM-derived structure; those remain `[U]` for the real app.

## Matrix (fake window)

| Condition                                     | Outcome                                                                                                              | Result             |
| --------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- | ------------------ |
| Single card, single-use allow                 | Option 1 selected, `Submit` invoked, card cleared, recorded selection is exactly the single-use option               | PASS               |
| Single card, refusal                          | The `No (tell the agent ...)` option selected and submitted, nothing else                                            | PASS               |
| Tree populated 1.5 s after the window appears | Found by warm-up and retry                                                                                           | PASS               |
| Window minimized                              | `ApprovalSurfaceUnavailable`; the window is neither restored nor touched, card still showing                         | PASS (fail closed) |
| Two identical correlated cards                | `ApprovalTargetAmbiguous`, nothing submitted                                                                         | PASS (fail closed) |
| Card for a different command                  | `ApprovalWindowMismatch`, nothing submitted                                                                          | PASS (fail closed) |
| Only an unrelated `Approve` button            | `ApprovalSurfaceNotFound`; the button is never matched                                                               | PASS (fail closed) |
| Consent absent or revoked between reads       | `ApprovalUndeliverable` before any pattern is used (in-memory surface)                                               | PASS               |
| Persistent option label permutations          | All "always ..." / "don't ask again" / scope-of-standing labels classified persistent and never chosen (label table) | PASS               |

## Not measured (and why it matters)

- The real Desktop app on any version other than the single probe of 2.17.0; the card layout may differ across versions.
- Real Chromium behaviour for obscured (not minimized) windows, DPI changes, window moves, and concurrent cards in different windows.
- The real refusal path: it is `[U]` whether `No` plus `Submit` with an empty write-in is accepted by the Desktop app.
- Latency and failure rates over repeated real deliveries. No rate is claimed.

## Conclusion

The adapter's decisions (what to select, what to refuse, when to stop) are tested and fail closed. Delivery reliability on the real app is **unmeasured beyond one delivery**; any release claim must come from real-app runs with the user present (tracked under the manual evidence items), never from this matrix.
