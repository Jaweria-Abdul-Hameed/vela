# Initial GitHub Issue Graph Seed

This is the recommended conversion plan. IDs are placeholders; create
issues and replace symbolic dependencies with real GitHub issue links.

## Wave 0 --- foundations

-   **I001 Repository bootstrap and engineering standards**
-   **I002 Architecture skeleton: Tauri + React + typed IPC**
-   **I003 SQLite persistence and migration framework**
-   **I004 Design tokens and base visual shell**
-   **I005 Adapter interfaces and fake adapters for tests**

## Wave 1 --- vertical product skeleton

Blocked appropriately by Wave 0: - **I010 Project import + recent
project persistence** - **I011 Event journal + run state machine
skeleton** - **I012 Git repository inspection adapter** - **I013
Reactive dot-field prototype with reduced-motion fallback** - **I014
Ambient gradient field + graphics quality controls** - **I015 Agent
adapter capability discovery**

## Wave 2 --- preflight and graph

-   **I020 Preflight engine end-to-end**
-   **I021 Markdown context manifest/discovery**
-   **I022 Local Markdown issue ingestion**
-   **I023 GitHub issue adapter**
-   **I024 Dependency DAG validation/cycle detection**
-   **I025 Predicted write-set model**
-   **I026 Parallelization safety engine**
-   **I027 Spatial dependency graph MVP**

## Wave 3 --- worker execution

-   **I030 Worktree/branch provisioning**
-   **I031 Process runner with cancellation/redaction**
-   **I032 Fresh agent session worker**
-   **I033 Matt Pocock skills bootstrap/detection**
-   **I034 Focused test/typecheck gate**
-   **I035 Durable checkpoint commits**
-   **I036 Fixed-point review integration**
-   **I037 Review fix-loop policy**

## Wave 4 --- integration

-   **I040 Serialized merge lane**
-   **I041 Integration validation gate**
-   **I042 Frontier recomputation after merge**
-   **I043 Push/remote reconciliation**
-   **I044 Draft/final PR workflow**
-   **I045 Worktree cleanup**

## Wave 5 --- resilience

-   **I050 Operation idempotency**
-   **I051 Startup reconciliation**
-   **I052 Crash/reboot recovery**
-   **I053 Stop All / pause / resume**
-   **I054 Human intervention model**
-   **I055 Capacity/profile state model**
-   **I056 Network/GitHub outage recovery**

## Wave 6 --- complete UI

-   **I060 Project universe camera/navigation**
-   **I061 Rich dependency node states**
-   **I062 Issue focus transition and inspector**
-   **I063 Live execution timeline**
-   **I064 Review/test evidence surfaces**
-   **I065 Conflict forecasting visualization**
-   **I066 Floating run controls and kill switch**
-   **I067 Intervention sheet**
-   **I068 Completion state**
-   **I069 Keyboard-accessible graph/list alternative**

## Wave 7 --- hardening

-   **I070 Security policy engine**
-   **I071 Prompt-injection/untrusted-repo boundaries**
-   **I072 Performance profiling/adaptive rendering**
-   **I073 500-node graph performance**
-   **I074 End-to-end fixture repository tests**
-   **I075 Visual regression suite**
-   **I076 Installer/update/signing pipeline**
-   **I077 Diagnostics/exportable run report**

These waves are not a substitute for actual blocking edges. `to-tickets`
or equivalent planning should translate the specification into native
tracker relationships and may refine ticket boundaries while preserving
product requirements.

# Approval Broker Issue Cluster

Add the following implementation tickets and wire their real
dependencies during ticket generation:

-   **I016 Approval Broker domain model + policy classification**
-   **I017 Approval Watchdog / stalled-worker detection**
-   **I018 Native Antigravity approval delivery adapter**
-   **I019 Windows UI Automation/accessibility approval adapter**
-   **I028 Guarded visual fallback adapter**
-   **I029 Approval fingerprint + loop protection**
-   **I038 Approval telemetry and intervention UI**
-   **I039 Effective-approval preflight probe**

Dependencies: - broker/policy precedes all delivery adapters; - watchdog
can be developed in parallel with native adapter after core event/run
skeleton; - UI automation depends on broker contract, not on visual UI
work; - visual fallback depends on semantic UI adapter contract and must
remain isolated; - end-to-end unattended acceptance tests depend on
broker + at least one working delivery path.

# Issue-Generation Scope Guard

When translating this seed into GitHub issues:
- create concrete Antigravity production integration tickets;
- retain fake/test adapter tickets needed for architecture and deterministic testing;
- do **not** generate Claude/Codex/other production runtime-adapter tickets for v1 unless an approved specification change adds them;
- treat Antigravity end-to-end readiness as release-blocking.
