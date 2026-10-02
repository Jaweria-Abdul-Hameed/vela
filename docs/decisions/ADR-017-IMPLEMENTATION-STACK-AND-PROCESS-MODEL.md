# ADR-017: Implementation Stack, Process Model, and Contract Layer

## Status

Accepted (2026-10-02). Implements ADR-001 (Tauri) and ADR-002 (SQLite journal); does not reopen them.

## Context

Prompt 5 must freeze concrete technologies and boundaries. Facts verified on 2026-10-02: Tauri 2.12 is stable
(tray, autostart, signed updater, NSIS/MSI, `ExitRequestApi::prevent_exit` documented); `tauri-specta` v2 is still a
release candidate; `ts-rs` 12.x is stable; `rusqlite` 0.40 bundles SQLite; the Antigravity SDK is Alpha and excluded
(ADR-016). The `windows` crate's UI Automation bindings are expected but their exact API surface was **not confirmed
from documentation**; a Phase 0 spike confirms them.

## Decision

1.  **Languages and shell:** Rust for all core logic; Tauri 2 as the Windows shell; React with TypeScript (strict)
    for the renderer. **No Python sidecar** (the SDK is not a v1 dependency, ADR-016).
2.  **Single OS process.** The orchestration core runs inside the Tauri application process. `vela-core` and every
    crate below it have **no Tauri dependency**, so the same core can be hosted by a headless binary later. Background
    operation (ADR-012) is implemented by hiding the window and preventing exit (`prevent_exit`), with a tray menu.
    The runtime behavior of `prevent_exit` is unverified; Phase 0 spike S-TRAY must confirm it. If it fails, the
    fallback is a separate `vela-daemon` binary hosting the same core with the UI as a client; that would amend this
    ADR, not ADR-001.
3.  **Concurrency:** `tokio` runtime. One **Orchestrator actor** owns run and worker state; workers, the merge lane,
    the Approval Broker, and watchers communicate with it by typed messages.
4.  **Persistence:** `rusqlite` with bundled SQLite, WAL mode, foreign keys on. **One writer thread** receives write
    commands over a channel; readers use separate read-only connections. Migrations are an embedded, ordered SQL ladder
    keyed by `PRAGMA user_version` with a pre-migration backup (`RECOVERY.md`); no migration framework dependency.
5.  **Journal:** one append-only `event_journal` table; every state transition is one transaction that updates state and
    appends events with a monotonically increasing `seq` (`PERSISTENCE.md`). UI events are emitted **only after
    commit**, so the UI is never ahead of durable state.
6.  **Contract layer:** DTOs derive `serde` and `ts-rs`; TypeScript types are generated into `packages/contracts`.
    Commands are a hand-maintained registry (name, request type, response type) enforced by a contract test that fails
    if a Rust command lacks a TypeScript wrapper or the reverse. `tauri-specta` is rejected while it is a release
    candidate.
7.  **Git:** shell out to the `git` binary under a hardened invocation (verified set, `vela-git`); no `libgit2`.
8.  **GitHub:** the `gh` CLI behind `IssueTrackerAdapter`, so Vela never stores or handles tokens; a local Markdown
    tracker is the other implementation. If `gh` is absent the remote features degrade to local-only.
9.  **Frontend tooling:** Vite, Tailwind CSS with CSS-variable design tokens, the Motion library for DOM transitions,
    a small external store (Zustand) for UI state, Vitest and Testing Library for unit/interaction tests, Playwright
    for visual regression. Rendering architecture is ADR-019.
10. **Monorepo:** one Cargo workspace plus npm workspaces. No Nx, Turborepo, or other orchestration layer.
11. **Logging:** `tracing` with structured JSON log files and a single central redaction layer.
12. **Packaging:** Tauri bundler, NSIS per-user installer by default, signed; WebView2 download bootstrapper by default
    (embedded bootstrapper or offline installer as release options); the Tauri updater with signature keys; updates are
    deferred while a run is active (FR-047).

## Alternatives rejected

Separate daemon now (extra IPC and lifecycle complexity before it is shown necessary); `libgit2` (hook, config, and
Windows edge-case behavior differs from the `git` binary that agents and users rely on, and the hardened-invocation
facts were verified against the binary); `tauri-specta` (release candidate); Redux or a data-fetching framework (no
server state); Nx or Turborepo (not needed for two languages); the Python SDK sidecar (ADR-016).

## Consequences

-   `docs/architecture/IMPLEMENTATION_ARCHITECTURE.md` and `COMPONENT_SPECIFICATIONS.md` carry the detail.
-   Phase 0 gains spikes S-TRAY and S-UIA-BINDINGS.
