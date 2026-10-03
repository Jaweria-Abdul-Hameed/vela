# F02 Evidence (Prompt 10, execution log)

Ticket: `docs/issues/graph/tickets/F02.md` (Domain primitives, ports, and TypeScript contract generation).
Base: `3e5aafe745ef11887380e0ec9a5dad76fdb3d41d` (`main` equal to `origin/main`, tree clean, verified from Git; F01 merged at `26b1828c4d582c06d0e2e9980021daccb1df7487`).
Worker: branch `f02/domain-primitives`, worktree `%LOCALAPPDATA%\Vela\worktrees\f02` (outside the repository and outside any sync folder).

## What F02 delivered (all inside `crates/vela-domain/src/**` and `packages/contracts/src/**`)

| Area | Where | Notes |
|---|---|---|
| Value objects | `ids/` | `RunId`, `TicketId`, `WorkerId`, `CommitSha` (full 40/64 hex, lowercased), `BranchName` (git ref-format subset), `WorktreePath`, `GateResult`/`GateStatus`, `ReviewSeverity`, `DependencyEdge`/`EdgeOrigin`, `RiskClass`. Identifiers validate on construction **and on deserialization**. |
| Typed errors | `errors/` | `ErrorClass` (8), `ErrorCode` (19, the `ERROR_HANDLING.md` names plus `AUTH_REQUIRED` and `GRAPH_INVALID`), `ErrorOrigin`, `RetryDisposition`, `VelaError` (the five questions). `PROVIDER_POLICY_BLOCK` is class `POLICY_BLOCK`, origin provider, never retried. |
| Event vocabulary | `events/vocabulary.rs` | One macro invocation generates the 53 kinds, `EventKind`, `Event` (kind plus typed payload) and a typed payload DTO per kind, so a kind cannot exist without a payload. Includes `ProviderPolicyBlockRecorded`/`Cleared` with provider-neutral wording. |
| Envelope | `events/mod.rs` | `EventEnvelope { seq, ts, run_id?, ticket_id?, worker_id?, kind, payload, schema_version }`; `kind` and `payload` are flattened from `Event`, so the wire shape is flat and the payload always matches the kind. |
| Ports | `ports/` | `AgentAdapter`, `GitAdapter`, `IssueTrackerAdapter`, `ProcessRunner`, `Notifier`, `ApprovalSource`, `ApprovalDeliveryAdapter`, `PowerManager`, `SessionProbe`, `Clock`, `IdGenerator`. `GitAdapter` = `GitRead + GitWorktrees + GitCheckpoints + GitMerges + GitRemote`; `AgentAdapter` = `AgentSession + AgentProfile + AgentReviewer`; both with blanket impls. The neutral `AgentEvent::ProviderPolicyBlock` lives here. |
| Fault mechanism | `fault/` | `FaultPoint` enum, `fault_point!` (compiles to `()` unless `fault-injection`), `VELA_FAULT_POINT` armed abort. |
| Command registry scaffold | `events/command_registry.rs` | `CommandGroup` (15 groups), `CommandRegistry`, `compare_contract`, `parse_wrapper_names`. |
| TypeScript contracts | `packages/contracts/src/generated/` (74 files, committed), `generated.test.ts` | Reached only through the `./generated/*` wildcard export; no barrel. |
| Cargo features | `vela-domain/Cargo.toml` | Already declared by F01 (`fault-injection`, `hook-table`); not edited. |

## Decisions and interpretations (recorded; none changes a frozen specification)

1. **Ports are dyn-compatible without a new dependency.** Async methods return `PortFuture<'a, T>` (a boxed `Send` future) rather than `async fn` (not dyn-compatible) or `async_trait` (not in the inventory). Every fallible method returns `VelaError`.
2. **Code-to-class mapping** (`ErrorCode::class`) is an F02 interpretation of "each maps to the typed classes above": approval surface/delivery problems and `APPROVAL_SURFACE_UNAVAILABLE` are `EXTERNAL_TEMPORARY` (but surface/target/window problems are never retried, since the document forbids clicking repeatedly), `APPROVAL_POLICY_UNKNOWN`/`STALLED`/`UNDELIVERABLE`, `TRUST_REQUIRED`, `CAPABILITY_MISSING`, `POSTURE_NOT_MET` and `STATE_STORE_UNRECOVERABLE` are `USER_ACTION_REQUIRED`, `POLICY_DENIED`/`POLICY_VIOLATION`/`PROVIDER_POLICY_BLOCK` are `POLICY_BLOCK`, `STATE_STORE_CORRUPT`/`MIGRATION_FAILED` are `INTERNAL_BUG`, `AUTH_REQUIRED` is `EXTERNAL_AUTH`, `GRAPH_INVALID` is `VALIDATION_FAILURE`. The match is exhaustive and covered by a test. A different mapping is a one-line change.
3. **Generated contracts use a golden-file test, not per-type `#[ts(export)]`.** `events/contract_export.rs` runs the real `ts-rs` `export_all` into a scratch directory under `cargo test` and compares it with the committed files (stale, missing and out-of-date files fail, and the failure message carries the new content). Regenerate with `VELA_UPDATE_CONTRACTS=1 cargo test -p vela-domain generated_typescript_contracts` (this is also how a conflict in generated files is resolved). This avoids tests that write into the tree while another test reads it, and needs no `.cargo/config.toml` (outside the write surface). Large integers (`seq`, `ts`, durations) are annotated `number` because they travel as JSON numbers.
4. **The command-registry type lives in `vela-domain::events::command_registry`.** The F01 skeleton is closed, so no new top-level module was added; the registry is part of the IPC contract with the events. TypeScript wrappers declare their names as `export const commandNames = [...] as const;` in `packages/contracts/src/commands/<group>.ts`; the contract test reads that directory (absent until the first command exists).
5. **The fault hook is always compiled; its call sites are not.** `fault::hit` and the hidden `__fault_point_armed!` macro exist in every build so the abort path is tested by default `cargo test`; the public `fault_point!` forwards to them only with `fault-injection` and expands to `()` otherwise (verified by two `const` compile tests, one of which would fail to compile if the argument were evaluated).
6. **`#![warn(missing_docs)]`** is enabled in the five modules F02 owns, so `clippy -D warnings` enforces "compile with documentation" for them without constraining other tickets' modules.

## Validation

Local Rust build, clippy and test are **BLOCKED/NOT VERIFIED** on this host (Smart App Control, `os error 4551` on the first build script; reproduced at the start of this session with `cargo test -p vela-domain`). Smart App Control and Windows security were not changed. The required Rust gates were executed in GitHub Actions on `windows-latest`.

| Command | Environment | Result |
|---|---|---|
| `cargo fmt --all --check` | local | PASS |
| `node scripts/check-crate-deps.mjs`, `node scripts/check-module-skeleton.mjs` | local and CI | PASS |
| `npm run typecheck -w @vela/contracts` (`tsc --noEmit`, includes the 74 generated files) | local and CI | PASS |
| `npm run test -w @vela/contracts` (4 tests) | local and CI | PASS |
| `npx eslint packages/contracts`, `prettier --check` | local and CI | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | CI only | PASS (local BLOCKED) |
| `cargo build --workspace --locked --all-targets` | CI only | PASS (local BLOCKED) |
| `cargo test --workspace --locked` (vela-domain: 42 tests) | CI only | PASS (local BLOCKED) |
| `cargo test -p vela-domain --features fault-injection` | not run | **NOT VERIFIED**: no CI job enables the feature (the standard workflow is outside F02's surface and a temporary probe workflow was declined). The feature-on macro is a one-line forwarder to `__fault_point_armed!`, which the default run executes (armed child aborts, unarmed child survives). The `vela-testkit` crate (F03) will enable the feature in the standard run. |
| `cargo test -p vela-domain --no-default-features --features hook-table` | not run | **NOT VERIFIED** for the same reason (the standard run unifies features to `full`). Desk-checked: without `full`, only `serde` is used; `ts-rs`/`serde_json` appear only behind `feature = "full"`. |

CI evidence (workflow `ci.yml`, trigger push, branch `f02/domain-primitives`):

- Run **37149648510** (`89cae06`): Rust job failed at `cargo test` only on `generated_typescript_contracts_match_the_committed_files`, as intended on the first run (nothing was committed yet); every other test passed (41), and fmt, structure checks, clippy `-D warnings` and build had passed. The failure output carried the generated files, which were committed (74 files).
- Run **37149849788** (`bc717ba`): both jobs PASS (the golden test compared byte-for-byte).
- Run **37150008510** (`2b19b7a`): both jobs PASS; Rust job 1m42s; `vela-domain` `test result: ok. 42 passed; 0 failed`; TypeScript job: contracts package 4 tests pass.

## Skills

`/implement` and the Matt Pocock skills are still not installed (no `.claude/` directory), so the `AGENTS.md` loop was followed manually; nothing was faked.

## Scope

Only the F02 surfaces were touched (`crates/vela-domain/src/**`, `packages/contracts/src/**`) plus this record. No manifest, lockfile, CI, F01 checker or skeleton file changed. No other ticket, spike or human gate was started.

## Review record (authoritative `/code-review`, fixed point `3e5aafe745ef11887380e0ec9a5dad76fdb3d41d`)

| Iteration | Reviewed head | Findings | Action |
|---|---|---|---|
| 1 | `5ed93c4` | 10 | Fixed in `4d64660`: every approval error is never retried; `VelaError::may_retry` derives retry from invariants (a forged `PROVIDER_POLICY_BLOCK` is never retried). Escalated, not fixed: missing `onboarding` command group (spec lists `onboarding.set`, the protocol's 15 groups omit it); golden-file export vs "cargo test exports bindings"; feature-on configurations not runnable in the standard CI. |
| 2 | `4d64660` | 12 | Fixed in `89173bc`: `GateResult::is_pass` rejects a non-zero exit code; identifiers reject `/ \ :` and `..`; `Display` uses wire names for codes. |
| 3 | `89173bc` | 8 | **Not fixed: the iteration limit (3) is reached.** |

CI for the reviewed heads: `4d64660` run on that SHA PASS; `89173bc` PASS (clippy `-D warnings`, build, `vela-domain` tests, TypeScript job). Verdict: **REVIEW_STALLED**. Open items from iteration 3 (the identifier item is classified medium and keeps the policy from passing):

1. Identifiers are still not Windows path-safe: `.`, trailing `.`, device names (`CON`, `NUL`, ...), `< > | " * ?`, leading `-`, and case-collisions are accepted (medium).
2. `Display` still prints the class with `Debug` spelling (low).
3. `GateResult` status/exit-code contradiction is guarded only in `is_pass`, not at construction/deserialization (low-medium).
4. `parse_wrapper_names` and the wrapper scan are brittle (type-annotated declarations, comments, non-group `.ts` files) (low, scaffold).
5. `MergeOutcome::Conflicted` leaves a mid-merge worktree and the port has no abort operation (design; port signature).
6. Dead code in the `hook-table`-only test configuration (not run by CI).
7. This evidence record was stale for the final head (corrected here).
Earlier accepted-as-design items: reviewer session identity, trust/operation-id/`RemoteName` types on the Git port, string-typed closed-set payload fields.
