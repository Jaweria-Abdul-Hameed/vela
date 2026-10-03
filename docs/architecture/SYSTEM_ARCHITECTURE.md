# System Architecture

## 1. Stack (frozen; see `IMPLEMENTATION_ARCHITECTURE.md` and ADR-017)

The list below is the original recommendation. The **frozen** stack, crate boundaries, process model, and rejected
alternatives are in `IMPLEMENTATION_ARCHITECTURE.md` and ADR-017/018/019, which take precedence where they differ
(notably: no Python sidecar, `ts-rs` contracts, `rusqlite`, the `git` binary, and the `gh` CLI).

**Desktop shell:** Tauri\
**UI:** React + TypeScript\
**Styling:** Tailwind CSS plus CSS custom properties/design tokens\
**Motion:** Motion for UI transitions\
**Spatial/3D:** React Three Fiber / Three.js where justified\
**Core local backend:** Rust/Tauri commands and background services\
**Optional agent adapter sidecar:** only if a provider SDK materially
requires Python or another runtime\
**Persistence:** SQLite with migrations\
**Git:** command adapter or libgit2-equivalent only where behavior is
well understood\
**Process execution:** explicit shell adapter, Windows-aware\
**GitHub:** API/CLI adapter behind issue-tracker interface

The architecture must not require a localhost browser experience in
production. Tauri's embedded webview is an implementation detail of a
native installable application.

## 2. Component boundaries

``` text
┌──────────────────────────────────────────────────────────────┐
│                         React UI                             │
│ Home / Canvas / DAG / Worker Detail / Timeline / Settings   │
└────────────────────────────┬─────────────────────────────────┘
                             │ typed IPC/events
┌────────────────────────────▼─────────────────────────────────┐
│                       Vela Core                             │
│ Project Service                                             │
│ Preflight Service                                           │
│ Context Index                                               │
│ Graph Service                                               │
│ Scheduler                                                   │
│ Policy Engine                                               │
│ Run State Machine                                           │
│ Recovery/Reconciliation                                     │
│ Event Journal                                               │
└──────────┬──────────────┬──────────────┬─────────────────────┘
           │              │              │
           ▼              ▼              ▼
      Agent Adapter    Git Adapter   Tracker Adapter
           │              │              │
      Antigravity      git/cmd       GitHub/local MD
      future agents
           │
           ▼
      Process/Test Adapter
```

## 3. Core invariants

-   UI never directly mutates Git or process state.
-   Scheduler never assumes a UI is open.
-   Every external side effect is represented as an operation with
    idempotency/reconciliation semantics.
-   Persistent state changes precede or atomically bracket important
    external actions where possible.
-   Provider-specific quota/session details never leak into core domain
    types beyond generic capabilities/status.
-   A worker cannot merge itself directly into integration unless the
    merge policy explicitly delegates that responsibility.
-   The integration branch has a single serialized, deterministic merge
    lane (Vela code, not an agent) running in a dedicated integration
    worktree, even while implementation workers are parallel (ADR-011).
-   No new frontier starts while integration health is not `HEALTHY`
    (including unknown after a merge).
-   Vela's ALLOW/ASK/DENY policy is authoritative and must remain
    enforceable; Vela does not depend on unconditional native
    auto-execution (ADR-009).
-   Repositories are `UNTRUSTED` until the user explicitly trusts them;
    repository, issue, and `AGENTS.md` text is untrusted input (ADR-013).

## 4. Event model

All meaningful events enter an append-oriented event journal: -
`RunCreated` - `PreflightCompleted` - `GraphBuilt` - `TicketReady` -
`WorkerStarted` - `CommandStarted` - `CommandFinished` -
`TestGateFinished` - `CheckpointCommitted` - `ReviewStarted` -
`ReviewFindingRaised` - `ReviewFindingResolved` - `WorkerCompleted` -
`MergeStarted` - `MergeCompleted` - `IntegrationGateFailed` -
`HumanActionRequired` - `RunPaused` - `RunResumed` - `RunCompleted`

Additional required events (the vocabulary is closed: new event kinds
require a documented addition here): - `ProjectTrusted` -
`ProjectTrustRevoked` - `OnboardingChoiceRecorded` -
`ApprovalAutomationConsentChanged` - `ApprovalDetected` -
`ApprovalClassified` - `ApprovalDeliveryAttempted` -
`ApprovalDeliveryVerified` - `ApprovalStalled` - `PolicyViolationDetected` -
`StopRequested` - `RunStopping` - `WorkerCancelled` - `WorkerFailed` -
`WorkerPaused` - `WorkerResumed` - `ReconciliationStarted` -
`ReconciliationCompleted` - `DiscrepancyDetected` - `PushCompleted` -
`PushFailed` - `MergeDiscarded` - `ConflictResolutionStarted` -
`WorktreeProvisioned` - `WorktreeRemoved` - `ProfileCooldownStarted` -
`ProfileAvailable` - `ProviderPolicyBlockRecorded` -
`ProviderPolicyBlockCleared` - `PromotionCompleted` - `StateStoreBackupCreated` -
`MigrationApplied` - `MigrationFailed`.

State and journal authority is defined in `PERSISTENCE.md`.

## 5. IPC

Frontend/backend communication uses typed request/response commands for
user actions and event streams for ongoing state. Never expose arbitrary
shell execution directly to the renderer.

Renderer hardening requirements (the renderer displays untrusted
Markdown, issue text, logs, and agent output while holding an IPC bridge
to a privileged core):

-   strict Content Security Policy: no remote script, no inline script or
    `eval`, no remote content loaded into the webview;
-   untrusted content is rendered inertly as text or a restricted
    Markdown subset with raw HTML disabled and no unsanitized HTML
    injection;
-   IPC commands are an explicit allowlist with least-privilege
    capabilities per window; no generic filesystem or shell command is
    exposed; every argument is validated in the core, never trusted from
    the renderer;
-   external links open through the OS with an explicit user action.

## 6. Background operation

Closing/minimizing the visual window must be distinct from terminating
orchestration. The product must make this explicit. Background execution
continues through the core process/service only after the user has
explicitly enabled background operation during onboarding; the normative
rules for window close, tray, login auto-start, reboot continuation,
keep-awake, and locked/disconnected sessions are in ADR-012. Whether the
core is a separate OS process or service is an architecture decision that
must satisfy ADR-012.

## 7. Adapter strategy

Every volatile integration implements a capability interface. Example
agent capabilities: - start session, - provide context pointers, -
invoke skill/command, - stream events, - cancel, - query status, -
recover session if supported, - report usage/capacity if supported.

Vela must feature-detect rather than infer capabilities from version
strings alone where possible.

# Approval Control Plane

Add an `ApprovalBroker` domain service and `ApprovalDeliveryAdapter`
boundary.

``` text
Antigravity request / stalled-worker signal
                 │
                 ▼
          Approval Watchdog
                 │
                 ▼
          Approval Broker
       policy classification
          /       |       \
       ALLOW      ASK      DENY
         │         │         │
         ▼         ▼         ▼
 Approval Delivery  Human UI  Reject/hold
         │
   ┌─────┼──────────────┐
   ▼     ▼              ▼
native  accessibility  visual fallback
API/    /control UI    (last resort)
setting automation
```

The policy decision and the delivery mechanism are separate
abstractions. No UI adapter is allowed to decide whether an action is
safe. The UI-automation and visual tiers are used only after the user has
enabled guarded UI automation (ADR-010), and evidence for classification
follows the evidence-binding rules of ADR-009.

The UI automation adapter must be isolated from the scheduler, versioned
against observed Antigravity surfaces, cancellable, observable, and fail
closed when it cannot identify the expected approval control with
sufficient confidence.

# Runtime Architecture Priority

The runtime architecture is intentionally asymmetric in v1:

```text
Vela Core
   │
   └── AgentAdapter
          │
          └── AntigravityAdapter  ← required/default v1 implementation
                 │
                 ├── capability discovery
                 ├── worker/session lifecycle
                 ├── command/skill workflow
                 ├── approval integration
                 └── recovery/reconciliation

Future optional adapters:
   ├── ClaudeAdapter
   ├── CodexAdapter
   └── OtherAdapter
```

The abstraction points upward; product priority points downward to Antigravity. Do not force the core to know Antigravity details, but do not weaken Antigravity integration to chase artificial provider symmetry.
