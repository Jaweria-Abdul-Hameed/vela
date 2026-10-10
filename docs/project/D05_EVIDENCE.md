# D05 Evidence (execution log)

Ticket: `docs/issues/graph/tickets/D05.md` (policy engine core: operation model, PowerShell and cmd normalizer, ordered rules, ALLOW/ASK/DENY).
Base (fixed point, recorded): `efc6ef3cd0ddc5fb0c05ccf054382e613c4645f8` (`main` equal to `origin/main`, clean; F01, F02, F03 and D01 integrated).

## Preflight (before any code)

- D05 is on the recomputed ready frontier in `CURRENT_STATE.md` (direct blocker F02 done, merged at `fd15065`); `ISSUE_GRAPH.json` lists `blockedBy: [F02]`, depth 2.
- Concurrency with D02: D05's write surface is `crates/vela-domain/src/policy/**`; D02's is `crates/vela-domain/src/graph/**`. Both modules are pre-created by F01 (`lib.rs` is never edited by a ticket), no shared contract is written by both, neither holds a resource key (`RK-*` are for real-account, foreground, clean-machine and quiet-machine work), D05 declares no migration block or fault point and no dependency change. D02 had not started (no D02 branch or commit). The dry run (`SCHEDULER_DRY_RUN.md` round 3) schedules both together. Verdict: safe to run concurrently; no write-surface, resource-key, schema or semantic conflict.
- Isolation: this worker uses the designated branch `claude/ecstatic-meitner-kns93i` in its own checkout, created from `main` with no divergence.

## What D05 delivered (`crates/vela-domain/src/policy/**`, pure, serde-only, no new dependency)

| File | Content |
|---|---|
| `path.rs` | Lexical Windows path resolution (case-fold, `.`/`..`, UNC, drive forms) and `Placement`; refuses device namespaces, alternate streams, short names, `~`, reserved names. |
| `normalizer.rs` | `normalize(text, Dialect)` for one simple command in PowerShell or cmd syntax; typed `Rejection` for pipes, redirection, chaining, subexpressions, expansion, escapes, comments, wildcards, quote tricks, invisible and look-alike characters; `Unspecified` requires agreement of both dialects. |
| `operation.rs` | `Operation`, `TrustState`, `RunMode`, `ProfileEntry` (validated on deserialization), `CommandProfile`, `PolicyContext`. |
| `classify.rs` | `OperationClass` (21 classes) and the table-driven classifier (Git, deletes, credentials, security controls, protected paths, nested shells, network, system, infrastructure). |
| `rule.rs` | `Decision`, `RuleId`, `RuleCondition`, `Rule`, `RuleSet` with creation-time conflict reporting (`Shadowed`, `Redundant`, `Overlap`) and the `ALLOW`-basis invariant. |
| `defaults.rs` | The ordered default rules and `default_rules_with(user_rules)`. |
| `engine.rs` | `PolicyEngine::evaluate` and `PolicyDecision {decision, rule_id, reason, scope}`; post-match re-check of the `ALLOW` basis. |
| `table.rs` | `HookRuleTable` (format version 1, re-validated on load) and `native_allow_rules` (provider-neutral). |
| `tests.rs` | Operation-class tables, injection corpus, conflicts, invariants, generated artifacts, redaction, property tests. |

## Acceptance criteria

1. **Force push, credential access, outside-root writes, unknown executables are DENY or ASK; no input yields ALLOW outside the confirmed profile (property test).** `operation_class_table` and `file_operations_and_roots` (about 130 rows); `no_command_is_allowed_outside_the_confirmed_profile` (every `ALLOW` must match an independent oracle: trusted, and the normalized command equals a profile entry), `commands_are_never_allowed_without_a_profile_or_trust`, `file_operations_are_allowed_only_inside_the_root`, `suffixes_never_extend_a_profile_allow`, `normalization_is_total_and_canonical` (proptest, 2000 cases each; also run once at 60000 cases in release, passing). `no_rule_can_widen_allow` shows no rule set can grant `ALLOW` without a basis.
2. **PowerShell and cmd injection corpus is rejected or ASK.** `injection_corpus_is_never_allowed_and_always_explained` runs 75 PowerShell and 44 cmd cases (chaining, pipes, redirection, subexpressions, expansion, encoded commands, look-alike Unicode quotes and dashes, newlines, NUL, stop-parsing, quote and backslash splitting, nested interpreters) under each applicable dialect and `Unspecified`.
3. **Rule conflicts are reported at creation time.** `rule_conflicts_are_reported_at_creation` and `user_rules_tighten_and_win_over_later_defaults`.

Constraints: repository, issue and `AGENTS.md` text never changes policy (inputs are the operation and the settings-derived context only; `text_claiming_authority_does_not_change_the_decision`); secrets never reach decisions or `Debug` output (`secrets_in_arguments_never_reach_decisions_or_debug_output`, and decisions carry the executable name and class only).

## Decisions

Ten interpretations are recorded as non-blocking decision requests in `D05_DECISION_REQUESTS.md` (DR-D05-1 to DR-D05-10). Notable: the `ALLOW` set is exactly profile commands plus structured in-worktree reads and writes in trusted repositories; the destructive-class `DENY` versus `ASK` split follows `SECURITY_AND_PERMISSIONS.md`; resolution is lexical (adapters resolve links first).

## Validation

Run on a Linux cloud host (local Rust works here, unlike the Windows development machine). The Tauri desktop crate and the fake approval window need GTK, which this host lacks; they are untouched by D05 and excluded locally (`--exclude vela-desktop --exclude fake-approval-window`); CI on `windows-latest` runs the full workspace. Node 22 is installed (the repository pins Node 24), so the TypeScript job was not run locally; D05 changes no TypeScript.

| Command | Exit | Notes |
|---|---|---|
| `cargo fmt --all --check` | 0 | |
| `cargo clippy --workspace --exclude vela-desktop --exclude fake-approval-window --all-targets --locked -- -D warnings` | 0 | |
| `cargo build --workspace --exclude ... --locked --all-targets` | 0 | about 21 s |
| `cargo test --workspace --exclude ... --locked` | 0 | `vela-domain` 172 tests (includes the nested `hook-table` and `fault-injection` configuration run), `vela-testkit` 53 |
| `cargo test -p vela-domain --lib --locked --no-default-features --features hook-table policy` | 0 | 45 policy tests with only `serde` |
| `node scripts/check-crate-deps.mjs`, `node scripts/check-module-skeleton.mjs` | 0 | |
