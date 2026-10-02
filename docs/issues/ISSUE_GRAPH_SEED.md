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

-   **I070 Policy engine hardening and extended coverage** (core is I006)
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

# Seed Additions and Ordering Corrections (Prompt 3)

IDs remain placeholders. Real blocking edges are created during ticket generation.

## Additional tickets

-   **I006 Policy engine core and rule model** (Wave 0/1). Carries the rule schema, default rule set,
    and `ALLOW`/`ASK`/`DENY` evaluation. **I016 depends on I006.** I070 is renamed "Policy engine
    hardening and extended coverage" and no longer carries the core.
-   **I007 Repository trust model and untrusted-repository data-only analysis** (Wave 0/1), including
    hardened Git for untrusted repositories (ADR-013). Blocks Build and provisioning.
-   **I008 Windows lifecycle: onboarding, tray, background operation, keep-awake, login auto-start**
    (Wave 1; ADR-012). Includes the onboarding choices consumed by ADR-010.
-   **I048 Antigravity AgentAdapter session lifecycle** (Wave 3): start, cancel, observe, resume
    where supported, capability contract (CAP-01..CAP-11), correlation identities.
-   **I049 Worktree provisioning contract** (Wave 3; `GIT_WORKFLOW.md`).
-   **I046 Final integration review and promotion workflow** (Wave 4; ADR-014).
-   **I047 Dependency analyst task and snapshot validation** (Wave 2; `PARALLELIZATION.md`).
-   **I057 State-store integrity, backup, migration failure and inventory rebuild** (Wave 5;
    `RECOVERY.md`).
-   **I058 Notifications (FR-028)** (Wave 5).
-   **I059 Settings, onboarding screens, command palette** (Wave 6).
-   **I080 Native Permission Posture preflight probe** (Wave 1b; ADR-009; extends I039).
-   **I081 Renderer hardening: CSP, inert rendering, IPC allowlist** (Wave 0/1 baseline, enforced
    thereafter; `SYSTEM_ARCHITECTURE.md` section 5).
-   **I082 Merge-lane conflict-resolution attempt and escalation** (Wave 4; ADR-011).

## Ordering corrections

-   The Approval Broker cluster (I016..I019, I028, I029, I038, I039, I080) belongs to Wave 1b, after
    I005 (adapters, fakes), I006 (policy core), I007 (trust), and the event journal I011.
-   I019 (UIA adapter) is gated on Phase 0 item 8 findings; I028 (visual fallback) depends on I019.
-   I040 (merge lane) depends on I030 (worktrees), I035 (checkpoints), and the integration-worktree
    provisioning in I049.
-   I076 (installer/update/signing) includes the "never update during a run" requirement (FR-047).

# Architecture-Driven Seed Additions (Prompt 5)

Placeholder IDs; real blocking edges are created during ticket generation (Prompt 6). Each follows from `IMPLEMENTATION_ARCHITECTURE.md`.

-   **I090 Workspace bootstrap:** Cargo workspace, npm workspaces, toolchain pins, formatting/lint/typecheck, CI dependency-direction check.
-   **I091 `vela-domain` ports and types with `ts-rs` export and the command-registry contract test.**
-   **I092 `vela-persistence`: writer thread, migrations ladder, journal, backup, integrity check.**
-   **I093 `vela-process`: process runner, redaction layer, Job Object tree kill (spike S-PROC-JOB), keep-awake thread, session probe.**
-   **I094 `vela-git`: hardened invocation, worktree manager, fixture hostile-config repo.**
-   **I095 `vela-hook` binary and rule-table format.**
-   **I096 `vela-testkit`: fixture repos, scripted agent, fake `agy` with recorded transcripts, fault points.**
-   **I097 `tools/fake-approval-window` reproducing the verified card roles.**
-   **I098 Phase 0 spikes S-NATIVE-POSTURE, S-ASK-RESUME, S-HOOK-GLOBAL, S-SCHEMA-OUTPUT, S-INTERACTIVE-TRUST, S-TRAY, S-UIA-BINDINGS, S-UIA-RELIABILITY, S-VISUAL, S-PROC-JOB, S-CAP-REMAINING (each its own timeboxed ticket with a recorded result).**
-   **I099 `tests/antigravity-compat` real-environment suite gated by `VELA_REAL_ANTIGRAVITY=1`.**
-   **Packaging ticket:** signed NSIS, WebView2 bootstrapper, updater deferral during runs (extends I076).

Ordering: spikes precede the tickets that depend on their result; no ticket may mark an **[U]** element ready before its spike ticket is closed with evidence.
