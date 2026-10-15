# Antigravity release gate evidence (Z04)

**Status: PARTIAL. One complete real workflow ran (AT-001). The rest of the gate is NOT VERIFIED here and needs a person with the Desktop app.**

A fake adapter or another agent cannot substitute: the real suite (`tests/e2e/tests/real_ticket.rs`) only runs with
`VELA_REAL_ANTIGRAVITY=1`, drives the installed `agy` through the real driver, the real `vela-hook` and an isolated worker profile, and
takes the machine-wide `RK-AGY-REAL-ACCOUNT` lock. Its results are version-stamped in `target/real-suite-results.jsonl`.

## Environment of the recorded run

| Item | Value |
|---|---|
| Date | 2026-10-15 |
| Antigravity CLI | `agy` 1.3.5 (as reported by `agy --version`) |
| Antigravity Desktop | not exercised |
| Windows | Windows 11 Home, 10.0.26300 (Smart App Control on: freshly built test programs needed retries) |
| Vela | branch `mega/m31` |
| Account | the owner's own signed-in account; Vela read no credentials (ADR-016) |

## Results per acceptance test

| AT | Requirement | Result | Evidence |
|---|---|---|---|
| AT-001 | one ticket unattended, start to merged and completed | **PASS (real)** | `one_real_ticket` passed in 48 s on `agy` 1.3.5: import, trust, plan, build, a real `agy` worker in an isolated profile under the real hook, a Vela checkpoint, the merge lane, finalize. The file the ticket asked for is on the integration branch and the primary checkout is untouched. An earlier real run the same day (162 s) showed the agent trying `git status`, which the hook denied; the worker prompt now says Vela commits. |
| AT-011 | a routine Desktop approval prompt is delivered by UI automation without the user | **NOT VERIFIED** | The UIA adapter is proven against a fake approval window (SP12) and against one real delivery observed on Desktop 2.17.0 (`EXTERNAL_VERIFICATION_2026-10-02.md` section V). No real end-to-end run with a Desktop-hosted session was made: the headless path produces no Desktop card. |
| AT-013 | the same approval repeating is stopped and surfaced as `APPROVAL_STALLED` | **NOT VERIFIED on the real path** | Proven with the broker and scripted agents (K01 tests). A real repeating prompt was not produced. |
| AT-015 | posture `MEETS`: a `DENY` class operation is classified before execution and not run; otherwise Autonomous is blocked | **PARTIAL** | The preflight readiness table and the policy engine are tested (K10, D05, X04). On the real installation the hook denied the agent's own `git status` in the first AT-001 run, which is the deny path observed for real. The full posture probe matrix on the real `agy` was run during the A06 spikes and not repeated here. |
| AT-017 | a locked session pauses only UI-automation workers; nothing stale is replayed | **NOT VERIFIED on a real lock** | Proven with a fake session probe (K07). A real lock and unlock was not exercised. |
| AT-025 | unauthenticated: `AUTH_REQUIRED`, no credential access, an API key does not silently change the runtime | **PARTIAL** | The `AUTH_REQUIRED` mapping and the credential-access audit are tested (A13, X04: the source tree has no credential-store access outside denial code). A real signed-out installation was not produced, and is not to be provoked on the owner's account. |
| AT-026 | every in-scope conversation is recorded, shown, and can be found and opened in Desktop 2.x | **FAILS honestly (not verified)** | The worker's conversation id is recorded and listed by a read model (A12), but whether Desktop shows a headless conversation, and how to open one by id, is spike SP08 and has not been verified, so the open action reports itself unavailable. AT-026 is specified to fail the requirement rather than pass on Vela's own rendering. |

## What this means for a release

- The core claim holds on a real installation: Vela can run a real ticket to a merged completion through the real Antigravity path.
- A release cannot claim AT-011, AT-013 (real path), AT-017 (real lock), AT-026, or a real signed-out check until a person runs them with
  the Desktop app present. Each is listed above as what it is; none is reported as passed.
- `docs/research/H01_google_position.md` remains open: nothing here changes the Terms position.

## How to re-run the recorded part

```powershell
$env:VELA_REAL_ANTIGRAVITY = '1'
cargo build -p vela-hook
cargo test -p vela-e2e-tests --test real_ticket -- --test-threads=1 --nocapture
Get-Content target\real-suite-results.jsonl -Tail 1
```
