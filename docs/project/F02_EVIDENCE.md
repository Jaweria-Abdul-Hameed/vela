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
4. **The command-registry type lives in `vela-domain::events::command_registry`.** The F01 skeleton is closed, so no new top-level module was added; the registry is part of the IPC contract with the events. A command name is independent of the group module that registers it (correction round; see `F02_DECISION_REQUESTS.md` DR-F02-1). TypeScript wrappers declare their names as `export const commandNames = [...] as const;` in `packages/contracts/src/commands/<group>.ts`; the contract test reads that directory (absent until the first command exists).
5. **The fault hook is always compiled; its call sites are not.** `fault::hit` and the hidden `__fault_point_armed!` macro exist in every build so the abort path is tested by default `cargo test`; the public `fault_point!` forwards to them only with `fault-injection` and expands to `()` otherwise (verified by two `const` compile tests, one of which would fail to compile if the argument were evaluated).
6. **`#![warn(missing_docs)]`** is enabled in the five modules F02 owns, so `clippy -D warnings` enforces "compile with documentation" for them without constraining other tickets' modules.

## Validation

Local Rust build, clippy and test are **BLOCKED/NOT VERIFIED** on this host (Smart App Control, `os error 4551` on the first build script; reproduced at the start of the first session with `cargo test -p vela-domain`; in the correction round `rustfmt` and `rustc` themselves were also blocked, so formatting was verified by applying the `cargo fmt --check` diff from CI). Smart App Control and Windows security were not changed. The required Rust gates were executed in GitHub Actions on `windows-latest`.

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
| `cargo test -p vela-domain --no-default-features --features fault-injection` (public `fault_point!` compiled in; armed child aborts through it) | CI, run from `fault::feature_matrix` inside `cargo test --workspace` | PASS (correction round) |
| `cargo test -p vela-domain --no-default-features --features hook-table` with `RUSTFLAGS=-D warnings` (hook shape: serde only) | CI, same mechanism | PASS (correction round) |

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

## Correction round (user-approved 2026-10-10; one bounded round plus one exceptional fourth review)

Starting point: `f02/domain-primitives` at `e0aeddb` (verified), `main` untouched at `3e5aafe`.

| # | Correction | Where |
|---|---|---|
| 1 | **Windows path safety.** Logical ids (`RunId`, `TicketId`, `WorkerId`, new `OperationId`) now use an allowlist `[A-Za-z0-9._#-]`: first character alphanumeric, no trailing dot, no `..`, no `~`, no Windows reserved device names. New `PathComponent` (separate type for names used in filesystem paths): lowercase `[a-z0-9._~-]`, at most 64 bytes, never reserved, and `PathComponent::encode` maps any id to one deterministically; ids that are not already canonical (uppercase, `#`, over-long) get `~` plus a stable 40-bit FNV-1a hash of the original, so `Run-1` and `run-1` get different directories. `case_fold_key` detects case collisions. `BranchName` additionally rejects `< > \| "`, reserved device-name components and trailing-dot components; `WorktreePath` rejects `..` components, reserved names, illegal punctuation, other colons, and trailing dot/space components. Serialization is unchanged (plain strings). | `ids/path_safety.rs`, `ids/identifiers.rs` |
| 2 | **Git and Agent ports against C07/C05.** Every Git method takes `GitContext { trust: RepoTrust, operation: OperationId }`; `RemoteName`, `RevisionSpec` and `Trailer` are validated types (no option injection, no trailer forgery); fetch and push return `FetchOutcome`/`PushOutcome`; `GitMerges::abort_merge` leaves a conflicted merge (before it, `discard_unpublished_merge` could only be called on a clean tree). Agent sessions are addressed by `SessionRef { worker, role: Worker or Reviewer }` for cancel, status and profile provisioning, and `ReviewRequest` names its worker, so Stop All can cancel a running review. `CommandSpec`'s `Debug` no longer prints environment values. | `ports/git.rs`, `ports/agent.rs`, `ports/process.rs`, `ports/tests.rs` |
| 3 | **Remaining iteration-3 findings.** `Display` prints class wire names; `GateResult` rejects a pass with a failing exit code at construction and on deserialize (`GateResult::new`); the wrapper parser handles type annotations, comments, template literals, and ignores helper files (`run.test.ts`, `run.d.ts`); the on-disk scan test no longer depends on the registry being empty (equality with the real registry belongs to the crate that assembles it); the `hook-table` shape has no dead test code. | `errors/`, `ids/values.rs`, `events/command_registry.rs`, `events/` |
| 4 | **Onboarding command.** The registry no longer requires a command name to start with its group name (the specification lists `onboarding.set` and `project.trust.*` but no group mapping). No command was invented; the question is recorded as DR-F02-1. | `events/command_registry.rs`, `F02_DECISION_REQUESTS.md` |
| 5 | **Regression tests.** Reserved names in any case and with extensions, traversal, `. / \ : < > \| " * ?`, trailing dot, leading `-`, case collisions (ids and branches), `NTFS` aliasing, truncation uniqueness, drive-prefix parsing, remote/revision/trailer injection, forged `GateResult`, `SessionRef` cancel of a reviewer, `CommandSpec` redaction, wrapper-parser edge cases. | the modules above |
| 6 | **Feature configurations in CI.** `fault::feature_matrix` runs `cargo test --no-default-features --features fault-injection` and `--features hook-table` (with `-D warnings`) from inside the standard `cargo test --workspace`, in a scratch target directory. | `fault/feature_matrix.rs` |

Not changed (recorded, not decided here): closed-set payload strings (DR-F02-2); the `FaultPoint::ALL` and `ErrorCode::ALL` arrays remain hand-maintained (each has a uniqueness and round-trip test and an exhaustive `match`).

CI: run 38039110178 (`4c33642`) failed `cargo fmt --check` (rustfmt cannot run locally), run 38039320794 (`b145b5b`) failed three tests (drive-prefix parsing, a doc comment that changed `GateResult.ts`), run **38039547725** (`c8476e5`) PASSED both jobs: clippy `-D warnings`, build, `vela-domain` 69 tests, TypeScript job.

## Review iteration 4 (exceptional, user-authorized; head `31a2b84`, CI run 38039690334 PASS)

Nine findings. **Verdict: the Engineering exit policy does not pass; F02 stays REVIEW_STALLED and was not merged.** No further iteration is authorized. Blocking (medium correctness or security, classified here):

1. **`GateResult` can be serialized but not read back.** Validation runs on construction and deserialize only; the fields are public and `Serialize` does not validate, so a contradictory record written to the append-only journal fails to deserialize on replay. Also `Fail` with `exit_code: Some(0)` is not rejected, and `WorkerCompleted.gates`/`IntegrationGateFailed.result` accept any status. Fix: private fields with accessors (or validate on serialize) and stricter status/exit-code pairing.
2. **`VelaError::may_retry` ignores `side_effects`.** An `EXTERNAL_TEMPORARY` error that already committed, pushed or merged is reported retryable, contradicting "never retry non-idempotent actions blindly". Fix: return false when any side effect is recorded.
3. **`PathComponent::encode` is not the identity for safe text of 54-64 bytes** (it truncates and hash-suffixes it), contradicting its documented guarantee and the stable-layout property. Fix: truncate only above `MAX_COMPONENT_LEN`.
4. **`ConversationId` is an unvalidated `pub String`** fed back into `TurnRequest` (option-injection risk in the adapter), unlike the validated Git inputs.

Non-blocking (low or design): `CheckpointMessage` fields are public so its subject validation can be bypassed; `compare_contract` matches names only and drops the group, and the directory scan is test-only; the feature-matrix test costs two cold nested builds per `cargo test --workspace` and overrides `RUSTFLAGS`; the `WorktreePath` doc comment and generated `WorktreePath.ts` describe the old rules; `GitMerges::merge` takes a branch, not the reviewed commit, and does not report the merged source; `case_fold_key` folds ASCII only; whitespace-only trailer values and the `commandNames` prefix match in the wrapper parser.

To pass: fix items 1-4 (small, local changes, all inside F02's surfaces), regenerate `WorktreePath.ts` if its doc changes, and obtain a further authoritative review.

## Correction round 2 (user-approved 2026-10-10; one bounded round plus one exceptional fifth review)

Starting point verified: `f02/domain-primitives` at `6395445`, reviewed checkpoint `31a2b84`, `main` and `origin/main` at `3e5aafe`, no other ticket started.

| # | Iteration-4 finding | Fix | Regression tests |
|---|---|---|---|
| 1 | `GateResult` could be serialized but not read back; `Fail` with exit 0 not rejected | Fields are private; `GateResult::new` and `Deserialize` share one rule set: non-empty gate, `Pass` has exit 0 or none, `Fail` has non-zero or none, `NotApplicable` has no exit code or duration, `Blocked` keeps the blocked tool's code. Every constructible value therefore serializes to something that deserializes to the same value. Accessors replace field access. | `contradictory_gate_records_cannot_be_constructed`; an exhaustive status x exit-code x duration sweep checking that construction, round trip and rejection on read agree; `forged_gate_json_is_rejected_on_read` |
| 2 | `may_retry` ignored side effects | `SideEffects` gains `uncertain` (outcome unknown, e.g. a timeout after the request was sent) and `any()`; `VelaError::requires_reconciliation()` is true after a commit, push, merge or uncertain outcome; `may_retry()` is false then (also for forged stored fields), and `with_side_effects` sets the stored retry to `Never`. | `an_error_after_a_side_effect_or_with_an_uncertain_outcome_is_never_blindly_retried` |
| 3 | `PathComponent::encode` was not the identity for safe 54-64 byte text | Truncation applies only above `MAX_COMPONENT_LEN`; a changed name near the limit is shortened so that body plus suffix fits. The guarantee is documented exactly: text of `[a-z0-9._-]` that meets the other rules and is at most 64 bytes maps to itself, so `encode(s) == new(s)`. | `safe_text_up_to_the_limit_is_its_own_component_at_every_length` (lengths 1 to 64, and a mixed 64-byte name); `text_over_the_limit_or_needing_a_change_near_it_stays_within_the_limit_and_distinct` |
| 4 | `ConversationId` was an unvalidated `pub String` | `ConversationId::new` is the only constructor (`[A-Za-z0-9._-]`, alphanumeric start, at most 128 bytes); every `conversation` field of `TurnRequest`, `AgentEvent`, `TurnResult` and `ReviewReport` uses the type, so a malformed provider-reported id is a validation error at the adapter boundary and can never reach a later request. | `conversation_ids_accept_provider_style_ids_and_reject_anything_that_could_be_an_option_or_injection`, `a_conversation_id_can_only_reach_a_later_request_through_validation` |

Review-to-merge commit binding (finding 8 of iteration 4). The existing protocol does **not** guarantee it at the port level: `REVIEW_PROTOCOL.md` records `review_head_sha` per review and ORCHESTRATION_ENGINE moves a worker `REVIEWING -> READY_TO_MERGE`, but `GitMerges::merge` took only a branch name, so a worker branch that moved after review (a late checkpoint) could have been merged unreviewed, and the outcome did not say what was merged. That is an F02 contract gap (the port signature is F02's), so F02 now has `GitMerges::merge(ctx, integration, source, expected_head)`: the adapter must refuse with `VALIDATION_FAILURE`, merging nothing, unless the tip of `source` equals `expected_head`, and `MergeOutcome::Merged` reports `merged_source`. **Dependency recorded for S12 (merge lane and integration validation):** the lane must pass the `review_head_sha` of the passing review as `expected_head` and journal `merged_source`; the enforcement inside `vela-git` belongs to the ticket that implements `GitMerges` (S12/S13 per the shared-surface table). F02 implements neither.

Smaller review-4 items fixed because they were cheap and in scope: `CheckpointMessage` fields are private (subject validation cannot be bypassed), whitespace-only trailer values are rejected, the wrapper parser matches `commandNames` only as a whole identifier, and the `WorktreePath` documentation (and generated `WorktreePath.ts`) states the actual rules. Not changed: `compare_contract` still compares names (the group-file check lives in the scan of the crate that assembles the registry), the feature-matrix test keeps its two nested builds (it is how CI exercises the non-default configurations without a workflow change), `case_fold_key` folds ASCII only (identifiers are ASCII; `BranchName` accepts non-ASCII letters, so Unicode folding is left to the Git adapter), and `CommandSpec` `Debug` still prints `args` (documented: secrets belong in the environment allowlist).

Decision requests DR-F02-1 and DR-F02-2 are unchanged and still open (`F02_DECISION_REQUESTS.md`).

CI for this round: run 38041105779 (`79fe658`) failed `cargo fmt --check` (rustfmt cannot run locally); run 38041191441 (`7f07572`) failed only the generated-contract golden test (new `uncertain` field and documentation changes); run **38041363423** (`e27509f`) PASSED both jobs after the TypeScript contracts were regenerated.
