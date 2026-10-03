# Shared-Surface Protocol (Prompt 7 audit addition)

**Status:** normative for every ticket in `docs/issues/graph/`. It exists because several files and directories are touched by many tickets that the
dependency graph deliberately lets run in parallel. Parallelism is safe only if each of those shared surfaces is either owned by one ticket or is
changed through a conflict-free mechanism. Where a mechanism below is not delivered by its owning ticket, the tickets that rely on it must serialize
(`docs/orchestration/PARALLELIZATION.md`: when uncertain, sequential).

Owning tickets: **F01** (sections 1, 2), **F02** (sections 1, 4, 6), **F04** (sections 4, 5), **F05** (section 7), **F06** (section 3), **F08** (section 4), **O01** (sections 5, 10).

## 1. Crate and package module skeleton (owner: F01, then F02)

F01 creates every workspace member in the frozen layout and every module listed below as an empty module declared in the crate's `lib.rs` (or `mod.rs`).
A ticket writes only inside its own module and never edits another ticket's module or the `lib.rs` declaration list. A ticket that genuinely needs a
module that is not listed raises a follow-up and does not edit `lib.rs` opportunistically.

| Crate / package | Modules |
|---|---|
| `vela-domain` | `ids`, `errors`, `events`, `ports`, `fault`, `state`, `graph`, `parallel`, `layout`, `policy`, `review`, `approval` |
| `vela-persistence` | `writer`, `migrations`, `journal`, `integrity`, `recovery`, `repositories` |
| `vela-process` | `runner`, `tree`, `power`, `session`, `redact` |
| `vela-git` | `lock`, `read`, `worktree`, `checkpoint`, `merge`, `remote` |
| `vela-adapters` | `antigravity::{capability, events, session, profile, posture, ask, skills, prompts, review, analyst, capacity, drift}`, `tracker_local`, `tracker_github`, `notifier` |
| `vela-uia` | `discovery`, `delivery`, `visual` |
| `vela-hook` | binary only |
| `vela-core` | `orchestrator`, `project`, `settings`, `trust`, `preflight` (+ `antigravity`), `context`, `analysis::{ingest, graph_build, analyst}`, `run`, `scheduler`, `provisioning`, `worker`, `review`, `merge_lane` (+ `conflict`), `finalize`, `intervention`, `capacity`, `operations`, `recovery::{reconcile, resume, inventory, environment, continuation}`, `stop` (+ `safe_points`), `broker` (+ `watchdog`, `rules`, `session`), `remote_queue`, `promotion`, `conversations`, `notifications`, `diagnostics` |
| `vela-testkit` | `fixture_repo`, `fault_harness`, `fakes`, `fake_agy`, `fake_gh`, `recovery_matrix` |
| `apps/desktop/src-tauri` | `commands/<group>.rs`, `wiring/{store,orchestrator,project,settings,adapters,approval,antigravity}.rs`, `plugins/{notification,tray,autostart,window_state,single_instance,updater,global_shortcut}.rs`, `events.rs`, `tray.rs`, `lifecycle.rs`, `window.rs`, `notify.rs`, `autostart.rs`, `update.rs` |
| `packages/ui` | `design`, `components`, `canvas::{ambient, graph, effects, focus, fallback}` plus `canvas/quality.ts`, `a11y`, `safe-content` |
| `apps/desktop/src` | `scenes/{home,universe,focus,completion}.tsx`, `surfaces/{projects,trust,preflight,analysis,buildready,inspector,timeline,evidence,intervention,runbar,settings,onboarding,palette,conflict}`, `state`, `ipc` |

## 2. Dependencies and lockfiles (owner: F01)

- F01 declares every third-party dependency and every `windows` crate feature set that `IMPLEMENTATION_ARCHITECTURE.md` section 1 assigns to a crate,
  with exact versions in `[workspace.dependencies]` and each crate's own `Cargo.toml`, and commits `Cargo.lock` and `package-lock.json` resolved for
  the whole inventory. Builds use `--locked` / `npm ci`.
- Later tickets edit only their own crate's `Cargo.toml` and normally add no dependency.
- A ticket that needs a dependency outside the inventory states it as a **dependency request** in its completion report, adds it to its own crate only,
  and regenerates the lockfile in a separate commit. A lockfile conflict is **never hand-merged**: the merge lane regenerates the lockfile from the
  merged manifests. Two tickets that both add a new dependency are serialized (hard blocker class "lockfile", `PARALLELIZATION.md`).

## 3. Database migrations (owner: F06; reserved numbering)

`crates/vela-persistence/migrations/NNNN_<ticket>_<name>.sql`. The ladder applies files in **numeric order from an empty database and tolerates gaps**; a
duplicate number is a test failure. To make parallel schema work collision-free, each ticket owns a reserved number block and the tables below. A
ticket never creates a table owned by another ticket and never uses a number outside its block. Blocks are ordered by schema dependency, not by
ticket order; foreign keys point only to tables in lower blocks or to tables whose parent block is lower. Adapter crates never depend on
`vela-persistence`: an adapter ticket returns records to the core service that persists them. Migration files and `repositories/` modules are
discovered by a `build.rs` directory scan (F06), and each migration-owning ticket also owns its `repositories/<ticket>_*.rs` files, so no shared
list is edited.

| Block | Ticket | Tables / changes |
|---|---|---|
| 0001-0009 | F06 | `projects`, `build_runs`, `event_journal`, `operations`, `settings`, `state_store_backups` |
| 0010-0019 | S01 | project repository-identity columns |
| 0020-0029 | S02 | settings layers, onboarding keys, `approval_automation_consent`, run policy snapshot |
| 0030-0039 | S03 | `project_trust` and trust consent records |
| 0040-0049 | S04 | `preflight_reports`, `adapter_capability_snapshots`, `skill_installations` (adapters return records; the preflight service persists them) |
| 0060-0069 | S06 | `graph_snapshots`, `tickets`, `dependency_edges` |
| 0070-0079 | O01 | `workers` |
| 0080-0089 | S08 | `worktrees`, run base-SHA and integration-branch columns |
| 0090-0099 | S10 | `commands`, `test_runs`, worker conversation-id column |
| 0100-0109 | S11 | `review_cycles`, `review_findings`, `git_checkpoints`, reviewer conversation-id column |
| 0110-0119 | S15 | `human_interventions` |
| 0120-0129 | S16 | `execution_profiles` |
| 0130-0139 | R01 | operation-record columns and indexes |
| 0140-0149 | K01 | `approval_requests`, `approval_decisions`, `approval_delivery_attempts`, `approval_fingerprints`, `policy_rules` |
| 0160-0169 | S17 | `notifications` |
| 0170-0179 | T03 | remote update queue |
| 0900-0999 | reserved | any later ticket must record a new reservation here through a decision request |

## 4. Command registry and generated contracts (owners: F02, F08)

- Commands are registered **one module per command group** (`apps/desktop/src-tauri/src/commands/<group>.rs` with
  `packages/contracts/src/commands/<group>.ts`). Groups: `project`, `settings`, `trust`, `preflight`, `graph`, `run`, `ticket`, `timeline`, `evidence`,
  `intervention`, `approval`, `policy`, `diagnostics`, `update`, `snapshot`. The registry, the Tauri capability manifest, and the contract test are
  assembled from the group modules in sorted order, so adding a group edits no other group's file.
- Generated command permission manifests live in `apps/desktop/src-tauri/permissions/` (generated, committed, one file per group, sorted); the hand-written grants in `capabilities/*.json` reference them and are never edited by a command-adding ticket.
- `packages/contracts` exposes generated types through a `package.json` `exports` wildcard (`./generated/*`). There is **no hand-edited barrel**.
  `ts-rs` output is committed and regenerated deterministically; a conflict in generated files is resolved by regeneration, never by hand.
- The event vocabulary is closed (`SYSTEM_ARCHITECTURE.md` section 4). Adding an event kind is a documented specification change, not a ticket detail.

## 5. Composition root, plugins, and Tauri configuration (owner: F04)

- `wiring/<concern>.rs` is owned by one ticket: F08 `store`, O01 `orchestrator`, S01 `project`, S02 `settings`, W01 `adapters`, K07 `approval`, A16
  `antigravity`. Every other core service registers with the Orchestrator through the registries in section 10 and needs no `src-tauri` edit. `plugins/<name>.rs` is owned by the ticket that needs the plugin and is registered from one pre-declared sorted list.
- `tauri.conf.json` is pre-seeded by F04 with empty `bundle`, `plugins`, and `app.security` sections. Z01 edits `bundle`; Z02 edits the updater
  plugin entry; no other ticket edits it. Capabilities are one JSON file per surface in `capabilities/` (auto-loaded), never one shared file.
  Signing configuration uses overlay files so no ticket edits another's keys and no production material is ever committed: Z01 owns `tauri.dev-signing.conf.json`
  (development Authenticode) and `docs/release/DEV_SIGNING.md`; Z02 owns `tauri.updater-test.conf.json` (test updater public key) and `docs/release/UPDATER.md`;
  H04 owns `docs/release/SIGNING.md` (non-secret production identifiers only); `release.yml` (Z01) builds the production profile from H04's secrets and variables.
  Authenticode and the Tauri updater signature are separate mechanisms with separate keys and are never conflated.
- A release-profile check fails the build if the `test-agent` cargo feature (a scripted agent, never a product feature) is enabled.

## 6. Fault injection (owners: F02 mechanism, F03 harness, each owning ticket declares its points)

The `FaultPoint` enum and `fault_point!` macro live in `vela-domain::fault` (compiled to nothing unless the `fault-injection` feature is on). Each ticket
that owns a non-idempotent external action or a persisted transition (S08, S10, S11, S12, S13, S14, R08, G03, G05, K01, R01) declares and places its own points as it
implements them. R03 owns the crash matrix and a standing completeness test that fails when a declared point has no matrix entry; S13, S14, and R08 declare and place their own points and test them with the F03 harness, and R03 (which depends on S13, S14, and R08 since Prompt 8) adds their matrix entries, so no declaring ticket writes into the matrix directories and entry order never depends on merge order. Z03 runs the completeness test last. R03 does not retrofit points. This keeps recovery testable as each slice lands.

## 7. Frontend shared files (owners: F05, U01)

- Scenes and surfaces self-register through Vite `import.meta.glob`; adding one never edits `app.tsx` or a registry file.
- Design tokens are renamed only by U01. Settings sections register through the section registry delivered by U15.
- `packages/test-fixtures` is organized as `streams/<surface>/`; a ticket adds only its own subdirectory.

## 8. Resource keys (serialize regardless of the dependency graph)

Two tickets, or two work sessions, that need the same key run one at a time.

| Key | Meaning | Holders |
|---|---|---|
| `RK-AGY-REAL-ACCOUNT` | the user's one real Antigravity account, its CLI conversation store, and the Desktop app's real sessions (quota, conversation list, session state) | spikes SP04, SP05, SP06, SP07, SP08, SP09, SP10, SP11; SP12 (real-Desktop part); manual real-Desktop evidence in K05, K06, A12; every gated real-environment run in A03, A05, A06, A07, A10, A11, A15, A16, Z04 |
| `RK-USER-ATTENTION` | the single human performing exact-step observations or setup | SP04, SP06, SP08, SP09, SP11, SP12 (real part), H01 and H04-H06, U19, manual evidence in K05, K06, A12, S17, L02, L03, L04, P03, R06, Z04, Z06 (clean-machine lifecycle and production-signed run), Z07 (clean-machine VM interaction) |
| `RK-FOREGROUND-WINDOW` | the interactive desktop foreground (UI Automation and real-window tests move focus) | SP01, SP02, SP12, K05, K06, W01 smoke, Z07 and Z06 clean-machine evidence |
| `RK-CLEAN-MACHINE` | the clean Windows test machine: the H05 VMware VM and its snapshots (a quiet-machine workload on the 16 GB host: never with PF1, heavy agent work, or large WSL workloads) | Z07, Z06 (production-signed run) |
| `RK-QUIET-MACHINE` | exclusive use of the reference machine while performance is measured (no builds, no other runs) | PF1, the U17 measurement runs |

Within the real-account group the recommended order is SP08 first (it decides whether the isolated-profile design stands and is confounded by any other
CLI conversation created concurrently), then SP09 (it informs H06 together with SP08), SP04, then SP06 and SP05, SP07, SP10, and SP11 last (it deliberately provokes quota, waiting, and
self-update behavior). `tests/antigravity-compat` (A15) takes a machine-wide lock for `RK-AGY-REAL-ACCOUNT`.

## 9. Shared documents

- `docs/project/CURRENT_STATE.md` is edited only on the integration branch by the person or lead agent closing a phase. Tickets report their state
  deltas in the completion report and never edit it in parallel.
- Each spike writes only its own `docs/research/SPIKE_<id>.md` and reports register deltas. The verification register
  (`IMPLEMENTATION_ARCHITECTURE.md` sections 14-15) is updated by one consolidating step after the spikes that gate a decision have closed.
- `tests/fixtures/transcripts/**` is owned by A02 (TK1 follows it); `tests/e2e/**` is split by subdirectory (`fixture-suite`, `antigravity`, `visual`, `clean-machine` (Z07),
  `perf`, `app-smoke`); `tests/integration/security/**` is split into `injection` (X01) and `regression` (X04).

## 10. Orchestrator extension points (owner: O01)

The Orchestrator owns three open-ended registries so that the many orchestration tickets add behavior without editing a shared enum or function:
a **command registry** (each module registers a handler for its own message type as a trait object), a **snapshot-contributor registry** (each module
contributes its part of `snapshot.get`), and a **service registry** (each core service registers itself, so only S01 and S02, which precede O01, need
their own `wiring/` file). The run and worker state-machine tables (D01) are closed: a ticket needs a documented specification change to add a state or
transition, and no ticket adds one opportunistically. Profile availability (S16) is a service in that registry; the scheduler (S09) and
recovery-resume (R02) read it and treat its absence as "available", so neither waits on S16 and S16 edits neither ticket's files. Adding the
`PROVIDER_POLICY_BLOCK` kind and the neutral `ProviderPolicyBlock` event is a documented specification change (ORCHESTRATION_ENGINE.md section 3.2,
CAPACITY_AND_PROFILES.md, SYSTEM_ARCHITECTURE.md section 4).
