# F01 Bootstrap Evidence (Prompt 9, execution log)

Ticket: `docs/issues/graph/tickets/F01.md`. Base: `532cf32890e6dd26169213390ce8a0c8a45caf70` (`main` equals `origin/main`, tree clean, verified from Git).
Status of this record: **F01 is NOT complete.** The implementation is written but uncommitted, and the Rust gates could not run on this machine (blocker below).

## Blocker: Smart App Control blocks Cargo build scripts

- `HKLM\SYSTEM\CurrentControlSet\Control\CI\Policy\VerifiedAndReputablePolicyState` = `1` (Smart App Control in **enforcement**), read only.
- `cargo build` fails on the first dependency that has a build script:
  `could not execute process ...\target\debug\build\serde_core-60ad127564a1df96\build-script-build (never executed)` /
  `An Application Control policy has blocked this file. (os error 4551)`. Reproduced twice, and again with `CARGO_TARGET_DIR` outside the
  repository (`thiserror` build script), so it is not a path problem. A freshly compiled trivial executable did run, so the policy is a
  reputation decision on the generated binaries, not a blanket block.
- Consequence: `cargo build`, `clippy`, `test` cannot run locally on this machine for any crate, not only F01. This affects every later Rust ticket.
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
| `cargo build --workspace --locked` | Rust build | **FAIL (environment)**: os error 4551 above |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Rust lint | **NOT VERIFIED** (needs build) |
| `cargo test --workspace --locked` | Rust tests | **NOT VERIFIED** (needs build) |
| `cargo build --locked` / `npm ci` from a clean clone | acceptance criterion, clean-clone transcript | **NOT VERIFIED** (`npm ci` not yet run from a clean clone; Rust blocked) |
| CI run on a clean checkout | required integration test | **NOT VERIFIED** (no push; workflow written, never executed) |
| Tauri build, frontend production build, app launch | not in F01 scope | NOT APPLICABLE (F04/F05; F01 non-goal: no Tauri app) |

## Skills

`/implement` and the Matt Pocock skills are not installed in this environment (no `.claude/` directory; `/setup-matt-pocock-skills`
was never run, `CURRENT_STATE.md`). No invocation was faked: the `AGENTS.md` implementation loop was followed manually. The
`/code-review` step was not run because the required gates are not green and no checkpoint commit exists.
