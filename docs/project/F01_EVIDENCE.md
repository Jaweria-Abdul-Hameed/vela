# F01 Bootstrap Evidence (Prompt 9, execution log)

Ticket: `docs/issues/graph/tickets/F01.md`. Base: `532cf32890e6dd26169213390ce8a0c8a45caf70` (`main` equals `origin/main`, tree clean, verified from Git).
Status of this record: implementation checkpointed on branch `f01/bootstrap` (`71bf92a`, not merged to `main`). **Rust gates: LOCAL NOT VERIFIED/BLOCKED on this host; CI PASS on `windows-latest`** (section "CI evidence"). Review: see the end of this file.

## Local limitation (environment-specific): Smart App Control blocks Cargo build scripts

This is a property of the development host, not evidence that the Rust implementation is broken: the identical commit builds and tests green in CI. `cargo build` has **not** run successfully on this host.

- `HKLM\SYSTEM\CurrentControlSet\Control\CI\Policy\VerifiedAndReputablePolicyState` = `1` (Smart App Control in **enforcement**), read only.
- `cargo build` fails on the first dependency that has a build script:
  `could not execute process ...\target\debug\build\serde_core-60ad127564a1df96\build-script-build (never executed)` /
  `An Application Control policy has blocked this file. (os error 4551)`. Reproduced twice, and again with `CARGO_TARGET_DIR` outside the
  repository (`thiserror` build script), so it is not a path problem. A freshly compiled trivial executable did run, so the policy is a
  reputation decision on the generated binaries, not a blanket block.
- Consequence: `cargo build`, `clippy`, `test` cannot run locally on this machine for any crate, not only F01. This affects every later Rust ticket; CI is the accepted evidence path for Rust gates (user decision, 2026-10-03, option b).
- Nothing was changed: Smart App Control, Defender, and policies are untouched (`CURRENT_STATE.md`: surface an actual blocker instead of weakening it).
  Turning Smart App Control off is a one-way change in Windows (it cannot be re-enabled without resetting or reinstalling Windows), so it is a user decision.

## Environment changes made (per user, outside the repository)

- `rustup` and Rust 1.99.0 (`minimal` profile + `clippy`, `rustfmt`) installed under `%USERPROFILE%\.cargo` and `%USERPROFILE%\.rustup`.
  `PATH` was **not** modified (`--no-modify-path`); add `%USERPROFILE%\.cargo\bin` to run `cargo` from a normal shell.
- `npm install` created `node_modules/` (git-ignored) and `package-lock.json`.

## Validation results

| Command | Purpose | Result |
|---|---|---|
| `git rev-parse HEAD origin/main`, `git status -sb` | starting checkpoint | PASS (both `532cf32...`, clean) |
| `cargo generate-lockfile` | resolve the full pinned inventory (550 packages locked) | PASS (all pinned versions and `windows` feature names resolved) |
| `npm install` | resolve the npm inventory, create lockfile | PASS (280 packages) |
| `node scripts/check-crate-deps.mjs` | dependency direction on the real workspace | PASS (14 members) |
| `node scripts/check-module-skeleton.mjs` | module skeleton on the real workspace | PASS (10 Rust crates, 2 TS packages) |
| `node --test "scripts/*.test.mjs"` (with `cargo` on PATH) | checker unit tests + illegal-edge demonstration | PASS (39 of 39; includes `vela-core -> vela-git` exits 1 against a real Cargo workspace) |
| `npm run typecheck` | `tsc` for the four packages and the scripts | PASS |
| `npm run test:ts` | Vitest in four packages + checker tests | PASS (4 packages, 1 test each; 37 checker tests without `cargo`, 39 with) |
| `npm run lint:ts` | ESLint (typescript-eslint type-checked, react-hooks) | PASS |
| `npm run fmt:check:ts` | Prettier | PASS |
| `cargo fmt --all --check` | rustfmt | PASS |
| `cargo build --workspace --locked` (local) | Rust build | **LOCAL BLOCKED** (environment): os error 4551 above |
| clippy, `cargo test` (local) | Rust lint and tests | **LOCAL NOT VERIFIED** (need a build); see CI evidence |
| `npm ci` + `cargo build --locked` from a clean clone (local) | clean-clone transcript | **LOCAL NOT VERIFIED**; CI performs both from a clean checkout |
| Tauri build, frontend production build, app launch | not in F01 scope | NOT APPLICABLE (F04/F05; F01 non-goal: no Tauri app) |

## Skills

`/implement` and the Matt Pocock skills are not installed in this environment (no `.claude/` directory; `/setup-matt-pocock-skills`
was never run, `CURRENT_STATE.md`). No invocation was faked: the `AGENTS.md` implementation loop was followed manually. The
`/code-review` step was not run because the required gates are not green and no checkpoint commit exists.

## CI evidence (accepted path for Rust gates)

- Branch `f01/bootstrap`, commit `71bf92a` (parent: base `532cf32890e6dd26169213390ce8a0c8a45caf70`); workflow `.github/workflows/ci.yml`; run **37143358949**, trigger push, runner `windows-latest`. Result: **PASS** (both jobs, first attempt, no fixes needed).
- **Rust job (5m15s)**, toolchain from `rust-toolchain.toml` (1.99.0), in this order: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings` (no warnings); `cargo build --workspace --locked --all-targets`; `cargo test --workspace --locked` (all `test result: ok`, 0 failed; the placeholder and gate-logic tests ran); `node scripts/check-crate-deps.mjs` ("passed (14 workspace members)"); `node scripts/check-module-skeleton.mjs` ("passed (10 Rust crates, 2 TypeScript packages)").
- **TypeScript job (1m12s)**: `npm ci` from the committed lockfile (280 packages), then `npm run verify:ts`: Prettier, ESLint, `tsc` (4 packages + scripts), Vitest (4 packages, all pass), checker tests 39 pass, 0 fail, 0 skipped (the runner has `cargo`, so the real illegal-edge CLI tests ran).
- Before pushing, the workflow was tightened (not weakened): `build:rust` gained `--all-targets`, and CI now also triggers on pushes to any branch and manually.
- Not yet shown: a local clean-clone transcript (local Rust blocked). The CI checkout is a clean clone and is the equivalent evidence.

## Review iteration 1 (`/code-review` skill, fixed point `532cf32...f01/bootstrap` at `1a92ec8`)

The skill ran as a forked session (it carries no implementer conversation state but is not a separately started session), so independence is partial. It reported 9 findings (no crash bugs), all treated as actionable except the duplicate-run half of the last one:

| # | Finding | Disposition |
|---|---|---|
| 1 | rule 5 only checked as manifest shape | Fixed: `cargo tree -p vela-hook` check plus unit tests |
| 2 | entry guard `argv[1] === import.meta.url` fails open | Fixed: `import.meta.main` in both scripts |
| 3 | CLI illegal-edge tests skippable in CI | Fixed: tests throw when `cargo` is missing and `CI` is set |
| 4 | `vela-hook` skeleton closed, would block A04 | Fixed: crate roots may set `closed: false`; `vela-hook` is open |
| 5 | CURRENT_STATE stale and contradictory; no review section here | Fixed (this file and CURRENT_STATE rewritten) |
| 6 | `tests/fixtures` missing from the frozen layout | Fixed: created with `.gitkeep` |
| 7 | skeleton tests used weak count assertions | Fixed: exact module names must all load |
| 8 | `@types/node` 26 vs Node 24 | Fixed: `@types/node` 24.19.1 |
| 9 | CI `cancel-in-progress` on every ref; push and PR double runs | Partly fixed: cancellation off for `main`. Double runs on PR branches accepted (a work branch needs push CI, and the cost is bounded) |

Local gates after the fixes: checker tests 47 pass, 0 skipped (with `CI=1`); typecheck, ESLint, Prettier, Vitest PASS; both structure checks PASS on the real workspace. CI for this fix commit: see "CI runs" below.

## CI runs

| Run | Commit | Result |
|---|---|---|
| 37143358949 | `71bf92a` (first implementation checkpoint) | success (Rust and TypeScript jobs) |
| 37144098726 | `890a31a` (review iteration 1 fixes) | success; checker tests 47 pass, 0 skipped |
| 37144479005 | `94c1563` (review iteration 2 fixes) | success (both jobs) |

Each run executed, on `windows-latest`: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo build --workspace --locked --all-targets`, `cargo test --workspace --locked`, `check-crate-deps`, `check-module-skeleton`; and `npm ci` plus `npm run verify:ts`. These are CI results; none of the Rust gates has run successfully on the development host (Smart App Control).

## Review iteration 2 (`/code-review`, head `890a31a`)

| # | Finding | Disposition |
|---|---|---|
| 1 | `cargo tree -p vela-hook` does not see workspace feature unification | Documentation corrected: the check is the hook package's own resolved tree, not the unified workspace build. Not a code defect: unification is inherent to Cargo and the manifest shape check still pins the edge |
| 2 | tree check missed build dependencies and non-host targets | Fixed: `-e normal,build --target all` |
| 3 | block-comment opener inside a line comment swallowed `mod` lines | Fixed: single-pass comment stripping, with test |
| 4 | nested modules and stacked or same-line `cfg(test)` attributes miscounted | Fixed: only top-level declarations count; attributes handled; tests added |
| 5 | no CI record for the fix commit | Fixed: the CI runs table above |
| 6 | rule 6 not checked for `vela-uia` consumers | Fixed: any dependency on `vela-uia` must be `cfg(windows)`-gated, with test |
| 7 | `serde_json` in the hook allowlist and `wiring/settings` are specification deviations raised without a decision request | **Open, needs the user.** Both are recorded as interpretations 2 and 3 in `TOOLCHAIN_AND_DEPENDENCIES.md`. They are not code defects; the user is asked to accept them or direct a different resolution |
| 8 | CLI test temp workspaces not removed | Fixed: `after` cleanup; verified no leftovers |

Local gates after the fixes: 50 checker tests pass, 0 skipped (`CI=1`); typecheck, ESLint, Prettier, Vitest PASS; both structure checks PASS on the real workspace.

## Review iteration 3 (`/code-review`, head `94c1563`; the configured limit of 3 is now reached)

No crash-level or high-severity findings. 8 findings:

| # | Finding | Disposition |
|---|---|---|
| 1 | rule 5 unified-build concern (hook linked with `full` in a workspace build) | Carried forward, not an F01 code defect. **Follow-up for the packaging ticket (Z01):** build and sign `vela-hook` with `cargo build -p vela-hook` (features resolved for the hook alone), never from the unified workspace build |
| 2 | specification deviations (`serde_json` in the hook allowlist, `wiring/settings`, `tao`) encoded before a decision request | **Open, needs the user** (same as iteration 2 item 7). `tao` is also an inference: `wry` needs a window library |
| 3 | `vela-testkit` allowed as a dev-dependency of `vela-process` (duplicate crate copies) | Fixed: forbidden for every crate testkit depends on, with test |
| 4 | TypeScript CI job did not install the pinned Rust toolchain | Fixed: explicit `rustup toolchain install` step |
| 5 | Prettier would format generated ts-rs output | Fixed: `.prettierignore` excludes `**/generated/` and the lockfile |
| 6 | comment stripper unaware of string literals and nested block comments | Not fixed: minor; limitation of a dependency-free line scanner, spurious failure only for unusual `mod.rs` content |
| 7 | rule 6 uses a `cfg(windows)` substring test | Not fixed: minor; a narrower `cfg(all(windows, ...))` gate would be flagged and would need a deliberate policy change |
| 8 | structure checks ran last in `verify:rust` | Fixed: they now run right after the format check |

**Status: REVIEW_STALLED by the letter of the protocol.** The review loop limit (3) is reached and the iteration 3 fixes (items 3, 4, 5, 8) have not been re-reviewed. Severity fell each iteration (no high findings in iterations 2 and 3). The user decides whether to accept this, authorize one more iteration, or resolve items 1, 2 and 6/7 differently. Nothing was merged to `main`.
