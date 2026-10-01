# System Architecture

## 1. Recommended stack

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
-   The integration branch has a single serialized merge lane even while
    implementation workers are parallel.
-   No new frontier starts while integration health is unknown after a
    merge.

## 4. Event model

All meaningful events enter an append-oriented event journal: -
`RunCreated` - `PreflightCompleted` - `GraphBuilt` - `TicketReady` -
`WorkerStarted` - `CommandStarted` - `CommandFinished` -
`TestGateFinished` - `CheckpointCommitted` - `ReviewStarted` -
`ReviewFindingRaised` - `ReviewFindingResolved` - `WorkerCompleted` -
`MergeStarted` - `MergeCompleted` - `IntegrationGateFailed` -
`HumanActionRequired` - `RunPaused` - `RunResumed` - `RunCompleted`

The current state may be materialized for fast reads, but the journal is
retained for debugging/replay.

## 5. IPC

Frontend/backend communication uses typed request/response commands for
user actions and event streams for ongoing state. Never expose arbitrary
shell execution directly to the renderer.

## 6. Background operation

Closing/minimizing the visual window must be distinct from terminating
orchestration. The product must make this explicit. Background execution
should continue through the core process/service according to platform
capability and user setting.

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
safe.

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
