# F03 Evidence (Prompt 10, execution log)

Ticket: `docs/issues/graph/tickets/F03.md` (Test kit foundation: fixture repositories, fault-injection harness, synced-folder guard).
Base: `ed85f5ab07ae19dd9f25bd0b07d5043e9a37497a` (`main` equal to `origin/main`, tree clean, CI green; F02 merged at `fd15065f041fc3daa80ca34836710388f75b248d`). Verified from Git at the start.
Worker: branch `f03/test-kit-foundation`, worktree `%LOCALAPPDATA%\Vela\worktrees\f03` (outside the repository and outside any sync folder).

## What F03 delivered

| Area | Where | Notes |
|---|---|---|
| `FixtureRepo` builder | `crates/vela-testkit/src/fixture_repo/mod.rs`, `content.rs`, `git.rs` | Six kinds (`FixtureKind`): multi-package project, seeded tickets, seeded review defect, schema-overlap pair, merge-conflict pair, hostile local config. Built into a unique temp directory with fixed text, fixed author and committer, and a fixed commit clock (`BASE_EPOCH` plus 60 s per commit), so two builds have identical commit ids. Every `git` call uses an empty global config (`GIT_CONFIG_GLOBAL`), `GIT_CONFIG_NOSYSTEM=1`, `core.autocrlf=false` and the neutralising set below. |
| Synced-folder guard | `fixture_repo/guard.rs` | `classify_path` (pure; OneDrive, Dropbox, Google Drive, iCloud, Box by whole path component, case-insensitive, both separators, `\\?\` prefix, 8.3 short names, trailing dots and spaces), `classify_path_with_roots` (explicit roots, fed from the `OneDrive*` environment variables by `sync_roots_from_env`), `ensure_fixture_location` (also resolves symlinks and junctions of the nearest existing ancestor, and refuses any location inside a Git repository, per `DIRECTORY_STRUCTURE.md`). The builder and the harness call it before creating anything. |
| Hostile local-config repository | `fixture_repo/hostile.rs` | The P10 exposure set: `core.fsmonitor`, `diff.external`, `diff.hostile.textconv`, `filter.hostile.clean/smudge`, hooks `pre-commit`, `post-checkout`, `reference-transaction`, `post-merge`, plus a tracked `*.dat` routed through the driver by `.gitattributes`. Every payload is `echo fired > ../markers/<name>`; the config and hooks are written as plain files after the repository is committed, so building never runs a payload. `FixtureRepo::fired_markers()` reports which payloads ran. |
| Scratch directories | `fixture_repo/scratch.rs` | `ScratchDir`: guard-checked, uniquely named (pid, counter, nanos), removed on drop, tolerant of the read-only object files Git leaves on Windows. |
| Kill-and-restart harness | `crates/vela-testkit/src/fault_harness/mod.rs` | `KillRestartHarness`: re-executes the current test binary on a named child test (`--exact`) with `VELA_FAULT_POINT` set to a `FaultPoint` name, requires that the child crashed (an abort, not a libtest failure), snapshots the state directory, restarts the child with nothing armed, and returns a `KillRestartReport` (both runs, the state found, the state after). Timeout kill, `ChildNotFound` when the filter matches no test, `NotKilled` when the child failed some other way. |
| Sample state machine | `fault_harness/sample.rs` | `fresh -> prepared -> committed` with atomic write-then-rename, `FaultPoint::SelfTest` between the two persists, and an idempotent `recover`. It is the harness self-test and the worked example for R03. |
| Fixture specifications | `tests/fixtures/specs/fixture-repos.json`, `tests/fixtures/issue-sets/seeded-tickets/*.md` | The JSON states the expected branches, commit count and files of each kind; the tests compare real Git state against it (and check the kinds listed equal the kinds built). The Markdown issue set is committed into the seeded-tickets fixture. `tests/fixtures/.gitattributes` pins LF. |
| Manifest | `crates/vela-testkit/Cargo.toml` | `vela-domain` now carries `features = ["fault-injection"]` (the only change; no new dependency, `Cargo.lock` unchanged). |

## Decisions

1. **The child aborts itself; the harness does not signal it.** F02's mechanism is `std::process::abort()` at an armed `fault_point!`, which is what a crash looks like (no unwinding, no destructors). "Kills the child" is satisfied by that, so the harness requires `crashed()` (not success and exit code not 101, libtest's failing-test code) and fails with `NotKilled` otherwise. The harness additionally kills a child that exceeds its timeout.
2. **Only `FaultPoint::SelfTest` exists.** Variants belong to the tickets that own the transitions (`SHARED_SURFACE_PROTOCOL.md` section 6), so F03 did not add any; the sample uses `SelfTest`, whose documentation already names the F03 harness. `KillRestartHarness::point_named` resolves any name in `FaultPoint::ALL`, so R03 can iterate the matrix without harness changes.
3. **Child protocol.** A crash test names one `#[test]` in its own binary; that test calls `child_context()` (returns `None` in a normal run, so the test is a no-op) and then does the work of the `Phase::Run` or `Phase::Restart` launch, reporting through `report("key=value")`. The state directory and phase travel in `VELA_HARNESS_STATE_DIR` and `VELA_HARNESS_PHASE`.
4. **`fault-injection` is enabled by the testkit manifest (revised after review 1: package-only coverage of the feature-off build added).** Under unified features this turns the feature on for `vela-domain` in every workspace build that includes the dev-only testkit; it adds no call sites. F02's nested feature-matrix test (`--no-default-features --features fault-injection` / `hook-table`) is unaffected and passed in CI.
5. **Neutralising set, and a P10 refinement found by CI.** `neutralizing_args()` is the test kit's own copy of the verified P10 set (`core.fsmonitor=false`, `core.hooksPath=NUL`/`/dev/null`, `core.pager=cat`, empty `diff.external`, empty clean/smudge/textconv of the hostile driver, `--no-pager`). CI showed that an empty `diff.external` alone makes plain `git diff` fail ("cannot spawn"), so diff-producing commands must also pass `--no-ext-diff --no-textconv`, exactly as P10 recorded. The set is documented accordingly; the production invocation is still `vela-git`'s (S08/G tickets). It covers the fixture's driver name only, not arbitrary drivers of an untrusted repository.
6. **Control test for the hostile fixture.** A workout (status, diff, add, commit, branch, log -p, checkout, merge) run without neutralisation fires all nine markers; the same workout with neutralisation fires none and still produces real content. So "nothing fired" is a meaningful assertion, and the fixture really contains each neutralizable driver and hook. The payloads only write marker files inside the fixture directory.
7. **Guard scope.** Classification is by whole path component prefix (`onedrive*`, `dropbox*`, `google drive*`/`googledrive*`/`my drive`/`google-drive`, `icloud*`/`mobile documents`/`com~apple~clouddocs`, `box sync*`); conservative by design (a folder named `OneDriveTemp` is refused). The extra refusal of locations inside a Git repository follows `DIRECTORY_STRUCTURE.md` ("never inside this repository"); a home directory that is itself a Git repository would also be refused (documented in the module).
8. **Git version.** `GIT_CONFIG_GLOBAL` needs Git 2.32 or newer; `git init -b` needs 2.28. CI uses 2.45+ and this host has 2.45.1.

## Acceptance criteria

| Criterion | Evidence |
|---|---|
| Each fixture repository kind is generated deterministically and verified by Git commands | `every_kind_matches_its_specification_when_inspected_with_git` (branches, commit counts, tracked files, clean status against `fixture-repos.json`), `building_the_same_kind_twice_gives_identical_commits` (identical `refs/heads` snapshot and tree for every kind), `commits_carry_the_fixed_identity_and_a_monotonic_fixed_clock`, `different_kinds_are_distinguishable`, plus a per-kind test using real Git (`seeded_tickets_are_the_markdown_issue_set`, `the_seeded_review_defect_is_one_known_line_on_its_branch`, `the_schema_overlap_pair_touches_the_same_schema_and_merges_cleanly`, `the_merge_conflict_pair_really_conflicts`, `the_hostile_repository_really_contains_the_neutralizable_drivers_and_hooks`). |
| The synced-folder guard rejects a OneDrive path in a test | `a_onedrive_parent_is_rejected_by_the_builder_and_nothing_is_created`, `a_onedrive_location_is_rejected_without_touching_the_disk`, `every_synced_provider_parent_is_rejected_by_the_builder`, `a_synced_parent_is_refused_before_anything_is_created`, `the_state_directory_refuses_a_synced_parent`. |
| The kill-and-restart harness arms a named point, kills the child, restarts it and reports the state found, on a sample state machine | `arm_kill_restart_reports_the_state_found` (state found `prepared`, restart rolls forward to `committed`, second restart idempotent), `the_fault_feature_is_compiled_in_through_this_crates_manifest`, `an_unarmed_run_completes_and_commits`, `a_fresh_state_directory_is_found_empty`, `a_child_that_fails_before_the_point_is_not_mistaken_for_a_crash`, `a_hanging_child_is_killed_at_the_timeout`, `a_missing_child_test_is_an_error_not_a_pass`, `fault_points_are_armed_by_name_and_unknown_names_are_rejected`. |
| Required unit tests: determinism, guard classification (OneDrive, Dropbox, Google Drive, iCloud), harness self-test | `guard::tests::*` (nine tests), the determinism tests above, `fault_harness::sample::tests::*`. |
| Required integration test: each kind inspected with real Git, hostile repository really contains the drivers and hooks | the per-kind tests above and the hostile control and neutralised workouts. |

## Validation

| Check | Where | Result |
|---|---|---|
| `node scripts/check-module-skeleton.mjs` | local | PASS (10 Rust crates, 2 TypeScript packages); `lib.rs` untouched |
| `cargo fmt --check`, structure checks, `cargo clippy --workspace --all-targets --locked -D warnings`, `cargo build --workspace --locked --all-targets`, `cargo test --workspace --locked` | CI only (local Rust is blocked by Smart App Control, os error 4551) | PASS on the final code checkpoint (see run ids). vela-testkit: 47 tests, 0 failed, about 13 s |
| TypeScript job (format, lint, typecheck, test) | CI | PASS (no TypeScript changed) |
| Local `cargo`, `rustfmt`, `clippy` | local | NOT RUN: blocked on this host; recorded, not worked around |

CI runs on the branch (windows-latest; every push):

| Run id | Head | Outcome |
|---|---|---|
| 38060074179 | `ac19886` | FAIL: rustfmt diff (fmt runs first) |
| 38060177037 | `6a276c6` | FAIL: clippy `redundant_closure` in `guard.rs` |
| 38060533609 | `118d4da` | PASS (both jobs) |
| 38060821941 | `ebab15a` | FAIL: rustfmt diff (after adding the workout test and trailing-dot guard case) |
| 38060951478 | `e7b785c` | FAIL: two hostile tests: empty `diff.external` makes plain `git diff` fail (needs `--no-ext-diff --no-textconv`), and the control's clean filter emptied every blob so `commit` found nothing to commit (workout now uses `--allow-empty` and a smudge-triggering checkout) |
| 38061609595 | `99019e2` | FAIL: rustfmt diff |
| 38061752167 | `7cba6b7` | PASS (both jobs): first code checkpoint |
| 38062014402 | `2f54390` | PASS (both jobs): review iteration 1 head |

## Write-surface and scope check

Changed: `crates/vela-testkit/src/fixture_repo/**`, `crates/vela-testkit/src/fault_harness/**`, `crates/vela-testkit/Cargo.toml` (own feature, allowed by the shared-surface protocol), `tests/fixtures/**`, and this document. Not touched: `lib.rs` (module declaration list), `scripts/module-skeleton.json`, root manifests, lockfiles, any `.github` file, any other crate. No new dependency, so no dependency request. Direction check (`check-crate-deps`) passed in CI.

## Review history

### Iteration 1 (head `2f54390`, CI run 38062014402 green; fixed point `ed85f5a`)

The review returned nine findings without severities; classified by the Engineering policy:

| # | Finding | Class | Disposition |
|---|---|---|---|
| 1 | The testkit manifest turns `vela-domain/fault-injection` on for every unified workspace build, so F02 "feature off" coverage (the `const` compile tests) is no longer exercised in CI, and a `--workspace` release build would carry live points | blocking, medium | Instructed design kept (feature enabled through the testkit manifest). Coverage restored: `fault_harness/feature_off.rs` runs `cargo test -p vela-domain --lib` (feature off, package-only build, scratch target) from the workspace test run and asserts the feature really is off. Release builds are per package (`-p`), which does not select the testkit; recorded as a follow-up for the release tickets (never build artifacts with `--workspace`) |
| 2 | Git environment only partly isolated (XDG/home attributes and ignore files, `GIT_TEMPLATE_DIR`, `GIT_EXTERNAL_DIFF`, `GIT_CONFIG_PARAMETERS`/`COUNT`) | blocking, medium (determinism, security) | Fixed: those variables removed, `HOME`, `USERPROFILE`, `XDG_CONFIG_HOME` pointed at the scratch directory, `core.attributesFile`/`core.excludesFile` set to the null device; unit test inspects the command environment |
| 3 | Box not recognised (`Box`, `Box-Box`) | blocking, medium (ticket guard coverage) | Fixed: `box` exact, `box-box*`, `box sync*`, with tests; `D:Projectsoxes` stays unclassified. Relocated Dropbox, Google Drive and iCloud roots remain undetectable by name (non-blocking, recorded) |
| 4 | `crashed()` accepted any non-101 failure | blocking, medium | Fixed: only the abort signature counts (no exit code, 134, or `0xC0000409`); exit 1, 2 and an access violation are not crashes |
| 5 | Repository refusal rejected any ancestor with `.git` (a dotfiles home would make the kit unusable) | blocking, medium | Fixed: refuses only locations inside the Vela repository (the workspace root of this crate) |
| 6 | Spec fields `defect`, `shared_path`, `merge` never read | non-blocking, low, cheap | Fixed: `the_specification_fields_agree_with_the_builders` and an exact file-count check |
| 7 | Sample lacks fsync, stale `state.tmp` cleanup | non-blocking, low, cheap | Fixed: `sync_all` on the temporary file, `recover` removes a stale temporary (tested). A second fault point in the write/rename window needs a `FaultPoint` variant owned by a later ticket |
| 8 | Timeout drops child output; `running 0 tests` substring match | non-blocking, low, cheap | Fixed: `Timeout` carries stdout and stderr; `ChildNotFound` matches the whole line |
| 9 | Duplicate io-error plumbing in harness and fixture modules | non-blocking, low | Not changed (cosmetic; two small separate error types) |

CI after the fixes: run 38062601858 FAIL (rustfmt diff), run 38062724626 PASS at `3004747`.


## Follow-ups and open items (none block F03)

- `FaultPoint` variants and the crash matrix belong to the owning tickets and R03; the harness is ready for them.
- F09 (port fakes) follows in this crate and must add modules, not edit these; later fixture kinds add a builder function and a spec entry.
- `vela-git` (G tickets) owns the production hardened Git invocation; the testkit copy is for fixtures only.
- The guard is name-and-root based; it cannot detect an arbitrary sync client pointed at an arbitrarily named folder. `sync_roots_from_env` narrows that for OneDrive.
