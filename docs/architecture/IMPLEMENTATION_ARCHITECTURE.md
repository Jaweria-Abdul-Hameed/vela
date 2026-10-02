# Implementation Architecture (Frozen, Prompt 5)

Status: **frozen 2026-10-02** for ticket generation. It implements ADR-001..ADR-016 and adds ADR-017 (stack and process
model), ADR-018 (Antigravity adapter and policy enforcement), and ADR-019 (UI rendering). Component-level contracts
(inputs, outputs, state, dependencies, failure modes, tests) and the requirements coverage matrix are in
`COMPONENT_SPECIFICATIONS.md`.

**Verification tags** (from `docs/research/EXTERNAL_VERIFICATION_2026-10-02.md`): **[V]** verified in the real
environment, **[P]** partially verified, **[U]** unverified and gated by a Phase 0 spike (section 14). A design element
tagged **[U]** may be built behind its seam but may not be claimed ready, and its dependents may not pass release gates,
until its spike passes. A single successful UIA delivery is **[V]** for that case only; it is never generalized.

## 1. Technology inventory (the complete list)

| Area | Choice | Why this and nothing more |
|---|---|---|
| Shell | Tauri 2.12 (stable), Windows-first | ADR-001 |
| Core language | Rust, `tokio` | one runtime; actor model for orchestration |
| Persistence | `rusqlite` (bundled SQLite), WAL, embedded SQL migration ladder | ADR-002; no ORM, no migration framework |
| Serialization | `serde`, `serde_json` | |
| Contracts | `ts-rs` generated TypeScript + command registry + contract test | `tauri-specta` v2 is still a release candidate |
| Graph | `petgraph` for topological sort and cycle detection | well-known, small |
| Git | the `git` binary, hardened invocation (**[V]** set) | `libgit2` rejected |
| GitHub | `gh` CLI behind `IssueTrackerAdapter` | Vela never handles tokens |
| Windows APIs | `windows` crate (Job Objects, `SetThreadExecutionState`, session/desktop probes, UI Automation) | UIA bindings **[U]**, spike S-UIA-BINDINGS |
| Logging | `tracing` + JSON files + one redaction layer | |
| Frontend | React + TypeScript (strict), Vite, Tailwind with CSS-variable tokens, Motion, Zustand | ADR-019 for rendering |
| Canvas | React Three Fiber / Three.js, confined to `packages/ui/src/canvas` | ADR-019 |
| Frontend tests | Vitest, Testing Library, Playwright (visual) | |
| Packaging | Tauri bundler (NSIS), updater plugin, autostart plugin, notification plugin, optional global-shortcut plugin | |
| Monorepo | Cargo workspace + npm workspaces | no Nx/Turborepo |

**Python sidecar: not necessary** (the SDK is excluded, ADR-016). **Second process: not necessary** unless spike S-TRAY
fails (ADR-017).

## 2. Workspace layout (exact)

```text
/
├─ AGENTS.md  CLAUDE.md  CODEX.md  GEMINI.md  CONTEXT.md  README.md
├─ DOCUMENTATION_INDEX.md  VELA_MASTER_BUILD_PLAYBOOK.md
├─ Cargo.toml                 (workspace)   rust-toolchain.toml
├─ package.json               (npm workspaces: apps/*, packages/*)
├─ .editorconfig  .gitattributes  .gitignore
├─ docs/                      (specification pack; docs/project/CURRENT_STATE.md)
├─ apps/
│  └─ desktop/
│     ├─ index.html  vite.config.ts  tsconfig.json  package.json
│     ├─ src/                 (renderer: see section 7)
│     └─ src-tauri/           (crate `vela-desktop`: composition root, commands, tray, windowing)
├─ crates/
│  ├─ vela-domain/            (pure types, ports, state machines, graph, policy, layout)
│  ├─ vela-persistence/       (SQLite, migrations, journal, repositories)
│  ├─ vela-process/           (process runner, Job Objects, redaction, power, session probes)
│  ├─ vela-git/               (GitAdapter, worktree manager, hardened invocation)
│  ├─ vela-adapters/          (antigravity, tracker_local, tracker_github, notifier)
│  ├─ vela-uia/               (UI Automation approval source/delivery, guarded visual module)
│  ├─ vela-hook/              (binary: PreToolUse hook helper, no I/O except stdin/stdout/spool)
│  ├─ vela-core/              (orchestrator, services, broker; no Tauri, no concrete adapters)
│  └─ vela-testkit/           (dev-only: fakes, fixture-repo builder, scripted agent, fault points)
├─ packages/
│  ├─ contracts/              (generated TS types + typed invoke/listen wrappers)
│  ├─ ui/                     (design tokens, components, canvas, store selectors)
│  └─ test-fixtures/          (sample snapshots and event streams for UI tests)
├─ tools/
│  └─ fake-approval-window/   (wry/WebView2 window mimicking the Desktop card roles; test harness)
└─ tests/
   ├─ fixtures/               (fixture repository specs and Markdown issue sets)
   ├─ integration/            (Rust integration tests across crates)
   ├─ e2e/                    (fixture run end to end, fake agent)
   └─ antigravity-compat/     (real-environment suite, gated by VELA_REAL_ANTIGRAVITY=1)
```

No empty or speculative provider crates (ADR-008). Exact versions are pinned in the bootstrap ticket.

## 3. Crate boundaries and dependency rules

```text
vela-domain  ◄── vela-persistence
     ▲   ▲            ▲
     │   └────── vela-core ──────────► (ports only; concrete adapters injected)
     │                ▲
vela-process ◄─ vela-git        vela-adapters   vela-uia   (each depends on vela-domain, vela-process)
                       └────────── composition root: apps/desktop/src-tauri
```

Rules enforced by a CI check (`cargo` dependency graph test): (1) `vela-domain` depends on no other Vela crate;
(2) adapter crates (`vela-git`, `vela-adapters`, `vela-uia`) never depend on `vela-core` or on each other;
(3) `vela-core` depends on `vela-domain` and `vela-persistence` only, plus port traits from `vela-domain`;
(4) only `vela-desktop` depends on Tauri and wires concrete adapters; (5) `vela-hook` depends only on `serde` and the rule-table types, which live in a
small `vela-domain` module compiled under a `hook-table` feature so the hook binary pulls in no graph, SQLite, or Tauri code; (6) `vela-uia` compiles only on Windows (`cfg(windows)`) and exposes the same traits as the fake in
`vela-testkit`; (7) no crate calls an Antigravity-specific API outside `vela-adapters::antigravity` and `vela-uia`.

**Ports (traits in `vela-domain::ports`):** `AgentAdapter`, `GitAdapter`, `IssueTrackerAdapter`, `ProcessRunner`,
`Notifier`, `ApprovalSource`, `ApprovalDeliveryAdapter`, `PowerManager`, `SessionProbe` (interactive-desktop availability),
`Clock`, `IdGenerator`.

## 4. Process and concurrency model

-   One OS process (ADR-017). The Orchestrator is a `tokio` task that owns run and worker state and processes messages
    from: UI commands, worker tasks, the merge lane, the broker, watchers, and timers. All mutations are applied through
    it; nothing else writes orchestration state.
-   Each worker is a task driving the `AgentAdapter` turn by turn; it reports progress as messages. It cannot mutate
    state directly.
-   The **merge lane** is a serialized service invoked by the Orchestrator with a persisted lock (operation id) and runs
    only deterministic Git operations (`vela-git`) in the integration worktree (ADR-011).
-   The **persistence writer** is a dedicated OS thread; reads use separate connections. Journal events are published to
    subscribers after commit.
-   **Background operation:** window close hides the window when background operation is enabled and a run is active
    (`prevent_exit`, **[U]** S-TRAY); otherwise it offers safe pause (ADR-012). Tray menu: reopen, status, Stop All, quit.
-   **Keep-awake:** `vela-process::power` owns one long-lived thread that sets `ES_CONTINUOUS | ES_SYSTEM_REQUIRED` while
    a run is active and clears it when it stops **[V call, P effect]**; it never changes lock or power policy (ADR-012).
-   **Interactive-session probe:** `SessionProbe` reports whether an interactive unlocked desktop is available (input desktop
    reachable); it gates UIA work only (ADR-012).
-   **Cancellation:** every spawned process tree is assigned to a Windows **Job Object** with kill-on-close; Stop All and
    worker cancel terminate the job (tree kill with no orphans is **[V]** via `taskkill /T`; the Job Object mechanism is
    **[U]**, spike S-PROC-JOB); graceful cancellation is a request message first,
    escalating after the configured wait.

## 5. Tauri application structure (`apps/desktop/src-tauri`, crate `vela-desktop`)

-   `main.rs`: builds the `tokio` runtime, opens the store, constructs the Orchestrator with concrete adapters (the
    composition root), registers plugins (tray, autostart, notification, updater, optional global shortcut), and starts
    background services.
-   `commands/`: thin handlers that validate arguments and send a typed message to the core; no business logic.
-   `events.rs`: subscribes to the post-commit journal feed, coalesces for UI delivery, and emits the single event
    channel (section 8).
-   `tray.rs`, `window.rs`, `lifecycle.rs`: tray menu, hide-on-close, `prevent_exit`, single-instance, window-state persistence.
-   `tauri.conf.json`: strict CSP, no remote content, a capabilities file granting only the registered commands and the
    needed plugin permissions (`SYSTEM_ARCHITECTURE.md` section 5); updater endpoints and public key; NSIS bundle settings.
-   No arbitrary shell, filesystem, or HTTP command is exposed to the renderer.

## 6. Contracts, IPC, and event streaming

-   **Types:** every DTO derives `serde` and `ts-rs`; `cargo test` exports bindings to `packages/contracts/src/generated`.
    A contract test compares the Rust command registry with the TypeScript wrappers.
-   **Commands (request/response):** `project.import|list|remove|open`, `project.trust.set|revoke`,
    `onboarding.set`, `settings.get|set`, `preflight.run`, `graph.get|approve`, `run.create|start|pause|resume|stop_all|cancel`,
    `ticket.inspect`, `timeline.get`, `evidence.get`, `intervention.resolve`, `approval.decide`, `policy.rule.create`,
    `diagnostics.export`, `update.can_apply|acknowledge`, `snapshot.get`. Each returns a typed result or a typed error
    (`ERROR_HANDLING.md` classes).
-   **Events (one channel):** `EventEnvelope { seq, ts, run_id?, ticket_id?, worker_id?, kind, payload, schema_version }`
    where `kind` is a journal event kind (`SYSTEM_ARCHITECTURE.md` section 4, closed vocabulary).
-   **Bootstrap and resync:** the UI calls `snapshot.get(run_id)` (state plus `last_seq`), then applies events with
    `seq > last_seq`; on a gap or renderer reload it refetches the snapshot. The renderer holds no authoritative state.
-   **Backpressure:** the core coalesces UI emissions to at most 30 per second per run (batched arrays of envelopes);
    the journal itself is never coalesced.

## 7. Frontend structure and state management

```text
apps/desktop/src/
  main.tsx  app.tsx
  scenes/        home.tsx  universe.tsx  focus.tsx  completion.tsx   (scene state, no router library)
  surfaces/      preflight  inspector  timeline  evidence  intervention  settings  onboarding  trust  palette  runbar
  state/         store.ts (Zustand), selectors.ts, reducer.ts (applies EventEnvelope), effects.ts (events to canvas effects)
  ipc/           invoke.ts  listen.ts  (wrap packages/contracts)
packages/ui/src/
  design/        tokens.css  typography  glass.tsx  icons
  components/    DependencyNodeLabel  FloatingRunBar  StopAllControl  ApprovalIndicator  ...
  canvas/        (only module importing R3F/Three) scene.tsx  dots.glsl  ambient.glsl  graph.tsx  quality.ts  fallback.tsx
  a11y/          GraphTree.tsx (keyboard list/tree), FocusRing
```

State: a single Zustand store fed by `snapshot.get` plus the reducer applying `EventEnvelope`s; derived selectors; per-frame
animation state lives in the canvas module, not React state. Reduced motion and graphics mode are store settings read from
`settings.get` and the OS media query. The store never infers authoritative status from animation (`DOMAIN_MODEL.md`).

## 8. Persistence, migrations, and the journal

-   Location: the Vela data root under the per-user local application data directory (never inside a repository or a
    synced folder); one database file; WAL; `PRAGMA foreign_keys=ON`.
-   Migrations: `crates/vela-persistence/migrations/NNNN_name.sql` applied in order, tracked by `user_version`; a backup
    precedes any migration; a failed migration restores the backup (`RECOVERY.md`).
-   Tables: those in `PERSISTENCE.md`. Key shapes: `event_journal(seq PK AUTOINCREMENT, ts, run_id, ticket_id, worker_id,
    op_id, kind, payload_json, schema_version)`; `operations(op_id PK, kind, intent_json, result_json, state, created, completed)`;
    materialized rows carry `last_event_seq`.
-   Transaction rule: each Orchestrator transition = one transaction (state update + journal append + operation update).
-   Redaction: payloads are redacted before they reach the writer thread.

## 9. Configuration

Layers: compiled defaults, user-global settings (SQLite), project settings (SQLite; command profile, trust, provisioning,
promotion mode), and the immutable run snapshot. **Project policy and trust live in Vela's store, never in the repository**
(ADR-013). Onboarding choices (ADR-010, ADR-012) are user-global settings. Initial defaults are in `CONFIGURATION.md`.

## 10. Dependency graph representation

`vela-domain::graph`: `Ticket` nodes and typed `DependencyEdge`s (explicit, inferred-hazard) in a `petgraph::StableDiGraph`;
cycle detection and topological layers; frontier computation; pairwise/group safety evaluation producing labelled
evidence (`PARALLELIZATION.md`); deterministic layered layout (`vela-domain::layout`) stored with the snapshot. The
snapshot is immutable per run (graph approval is recorded in it).

## 11. Testing architecture and fixtures

| Layer | Tool | Scope |
|---|---|---|
| Domain unit | `cargo test` (+ `proptest` for graph and transition invariants) | state machines, policy evaluation, parallel safety, layout, redaction |
| Persistence | `cargo test` with temp databases | migrations, journal atomicity, crash consistency with fault points |
| Adapters | `cargo test` with fixture repos and fakes | `vela-git` (hardened invocation; worktrees on Windows paths), `vela-process` (Job Object kill, redaction), tracker fakes |
| Antigravity adapter | scripted fake `agy` executable emitting recorded stream-json transcripts, plus the **real-environment suite** `tests/antigravity-compat` gated by `VELA_REAL_ANTIGRAVITY=1` | parsing, soft-denial detection, profile isolation, hook protocol |
| Approval | `tools/fake-approval-window` (wry/WebView2 page reproducing the verified card roles, labels, and option semantics) | UIA adapter correlation, fail-closed cases, wrong-window, persistent-option refusal |
| Orchestration e2e | `vela-testkit` scripted agent + fixture repositories | AT-001..AT-010, AT-018, AT-021 |
| Crash/recovery | fault points (`FaultPoint` enum) compiled under a test feature; kill the process at every worker transition | AT-005, AT-019, AT-022 (Prompt 16) |
| UI unit/interaction | Vitest + Testing Library over `packages/test-fixtures` | store reducer, selectors, keyboard paths, intervention states |
| Visual regression | Playwright (Chromium, software GL) over the built renderer with mocked IPC | home, active graph, worker detail, conflict, needs-human, completion, reduced motion |
| Performance | scripted measurements against `PERFORMANCE_BUDGET.md` on recorded reference hardware | startup, 100/500 nodes, idle, minimized, event burst, mount/unmount leak |

**Fixture repositories** are generated by `vela-testkit::FixtureRepo` into a temp directory **outside** cloud-synced folders:
a small multi-package project with seeded tickets, a seeded review defect, a schema-overlap pair, a merge-conflict pair,
and a repository with hostile local Git configuration (the verified P10 set) for untrusted-repo tests. Real-Antigravity
tests use a throwaway repository and never the Vela repository.

## 12. Packaging and release

Tauri bundler produces a signed NSIS per-user installer (default) with the WebView2 download bootstrapper; embedded
bootstrapper or offline installer are release options; the updater uses signed manifests and the core refuses
`update.can_apply` while a run is active, requiring explicit user action after quiescence and a pre-update state backup
(FR-047, `RECOVERY.md`). A long-running background process notifies the user to restart after a WebView2 Runtime update
(`NewBrowserVersionAvailable`). The release checklist in `RELEASE_CHECKLIST.md` governs.

## 13. Resolved ambiguities and queued corrections now applied

-   **Fixed point for reviews after integration merges:** `fixed_point_sha` starts as the worker's base SHA and is **advanced to the
    integration tip each time Vela merges that tip into the worker branch**, so `git diff <fixed-point>...HEAD` shows only the ticket's
    own changes plus any conflict resolution (verified three-dot behavior of `/code-review`). Final review uses `run_base_sha`.
-   **Antigravity-created worktrees** (subagents) are not Vela worktrees; Vela-created worktrees are the only execution workspaces and
    reconciliation ignores or reports the others (`GIT_WORKFLOW.md`).
-   **Shell:** the agent's `run_command` runs through PowerShell **[V]**; the policy normalizer supports a deliberately small grammar
    (single simple command, no pipes, redirection, chaining, or subexpressions) for both PowerShell and `cmd`; anything else is `ASK`.
-   **Error handling:** exit code 0 and `result.status SUCCESS` are never proof (`ERROR_HANDLING.md`).
-   **UIA adapter rules** are in `APPROVAL_BROKER.md` section 19.

## 14. Phase 0 spikes (gate the unverified elements)

| Spike | Question | Gates |
|---|---|---|
| S-NATIVE-POSTURE | Does "generated allow rules + hook `allow`" run allowed commands while the hook blocks DENY/unknown, and does hook failure still block? | Autonomous mode, ADR-009 `MEETS` |
| S-ASK-RESUME | After a hook-blocked ASK and a human approval, does a resumed conversation re-run the command? | headless ASK path |
| S-HOOK-GLOBAL | Are hooks and settings in an isolated profile's global location loaded and protected from agent writes? | hook placement |
| S-SCHEMA-OUTPUT | Does `--json-schema` enforce the reviewer contract in print mode? | review contract |
| S-INTERACTIVE-TRUST | How does the CLI and Desktop treat a new workspace interactively? | Desktop-hosted sessions |
| S-TRAY | Does `prevent_exit` with a tray keep the process alive after the last window closes? | ADR-017 single-process model |
| S-UIA-BINDINGS | Does the `windows` crate expose the UIA interfaces needed (element, patterns, conditions)? | `vela-uia` |
| S-UIA-RELIABILITY | Repeated and varied deliveries: other versions, minimized/obscured windows, concurrent cards, the refusal path | UIA claims beyond one delivery |
| S-VISUAL | Is the visual tier ever needed on the primary path? | `vela-uia::visual` |
| S-PROC-JOB | Does a Job Object with kill-on-close terminate the whole `agy` tree (including PowerShell children) with no orphans? | `vela-process` cancellation |
| S-CAP-REMAINING | `WAITING` status, concurrency ceiling, background self-update control, interactive TUI | CAP-04/05/07 |

## 15. Verified / partial / unverified register for architecture elements

| Element | Status |
|---|---|
| `agy -p` per-turn driving, `--conversation` resume, stream-json parsing | **[V]** |
| Soft-denial detection (stderr + empty result) | **[V]** |
| Generated user-level allow rules (exact, regex, path-scoped write) | **[V]** |
| Per-worker isolated profile via `USERPROFILE`/`HOME` with working authentication | **[V]** |
| Workspace `PreToolUse` hook: input fields, `deny`, fail-closed | **[V]** |
| Hook `allow` as a grant | **[V: does not grant]** (design avoids it) |
| Allow rule + hook `allow` combination | **[U]** S-NATIVE-POSTURE |
| Global-path hook in isolated profile; agent-write protection of policy files | **[U]** / **[P]** |
| ASK resume after refusal | **[U]** |
| Skills in print mode (`.agents/skills`) | **[V]** (mechanism); Matt Pocock skills in Antigravity **[U]** |
| Desktop card read by UIA; single-use allow delivery | **[V]** (one delivery) |
| UIA reliability across variants/versions/concurrency/refusal | **[P/U]** |
| Visual fallback | **[U]** |
| Process-tree kill with no orphans (`taskkill /T`), power request call, hardened Git invocation | **[V]** |
| Job Object implementation of tree kill (standard Windows API, not itself probed) | **[U]** S-PROC-JOB |
| `prevent_exit` runtime; `windows` UIA bindings | **[U]** |
