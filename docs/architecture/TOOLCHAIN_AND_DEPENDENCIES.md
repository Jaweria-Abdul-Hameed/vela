# Toolchain and Dependency Inventory (F01)

Status: written by ticket F01 (repository bootstrap). It records the exact pins that `IMPLEMENTATION_ARCHITECTURE.md`
section 2 defers to "the bootstrap ticket", the per-crate dependency assignment, and the structure checks. It does not
change the frozen architecture; where the specifications left a gap, the interpretation is listed under "Interpretations".

## Pinned tools

| Tool | Pin | Where it is enforced |
|---|---|---|
| Rust | 1.99.0 (stable, 2026-09-28), edition 2024, `rust-version = "1.99"`, components `clippy` and `rustfmt` | `rust-toolchain.toml`, `Cargo.toml` |
| Node | 24.19.0 | `.node-version`, `engines` in `package.json`, `.npmrc` (`engine-strict`) |
| npm | 11.17.0 | `packageManager`, `engines` |
| TypeScript | 6.0.3 | root `devDependencies`. TypeScript 7 is current upstream, but `typescript-eslint` 8.71 supports `<6.1.0`, so 6.0.3 is the newest compatible release. |
| Cargo and npm dependencies | exact versions (`=x.y.z` in `[workspace.dependencies]`; exact strings in every `package.json`) | `Cargo.toml`, `package.json` files, committed `Cargo.lock` and `package-lock.json` |

Versions were taken from the upstream registries on 2026-10-03 (crates.io, npm, `static.rust-lang.org`). GitHub Actions in
`.github/workflows/ci.yml` are pinned by commit SHA.

## Rust workspace members (frozen layout, `DIRECTORY_STRUCTURE.md`)

`apps/desktop/src-tauri` (`vela-desktop`), `crates/vela-{domain,persistence,process,git,adapters,uia,hook,core,testkit}`,
`tests/{integration,e2e,antigravity-compat}`, `tools/fake-approval-window`. No provider crates exist (ADR-008).

## Third-party Rust dependencies by crate

| Crate | Dependencies (normal) | Notes |
|---|---|---|
| `vela-domain` | `serde`; optional under feature `full` (default): `serde_json`, `petgraph`, `ts-rs`. Dev: `proptest` | Features `hook-table` and `fault-injection` are declared empty (see Interpretations). |
| `vela-persistence` | `rusqlite` (`bundled`), `serde`, `serde_json`, `tracing` | |
| `vela-process` | `tokio`, `tracing`, `tracing-subscriber`; `windows` (cfg(windows)) | `windows` features: `Win32_Foundation`, `Win32_Security`, `Win32_System_JobObjects`, `Win32_System_Threading`, `Win32_System_Power`, `Win32_System_RemoteDesktop`, `Win32_System_StationsAndDesktops` |
| `vela-git` | `tokio`, `tracing` | |
| `vela-adapters` | `tokio`, `serde`, `serde_json`, `tracing` | |
| `vela-uia` | `tracing`; `windows` (cfg(windows)) | `windows` features **[U]** (spike S-UIA-BINDINGS): `Win32_Foundation`, `Win32_System_Com`, `Win32_System_Variant`, `Win32_System_Ole`, `Win32_UI_Accessibility`, `Win32_UI_WindowsAndMessaging`; feature `visual` adds `Win32_Graphics_Gdi` **[U]** (S-VISUAL) |
| `vela-hook` | `serde`, `serde_json`, `vela-domain` (default features off, `hook-table` only) | |
| `vela-core` | `tokio`, `serde`, `serde_json`, `tracing`. Dev: `vela-testkit` | |
| `vela-testkit` | `tokio`, `serde`, `serde_json` | |
| `vela-desktop` | `tauri` (+ `tray-icon`), `tauri-plugin-{notification,autostart,updater,single-instance,window-state}`, optional `tauri-plugin-global-shortcut` (feature `global-shortcut`), `tauri-build` (build), `tokio`, `serde`, `serde_json`, `ts-rs`, `tracing`, `tracing-subscriber`, `tracing-appender` | Library only until F04 adds `main.rs`, `build.rs` and `tauri.conf.json`. |
| `tools/fake-approval-window` | `wry`, `tao` | `tao` is the window library `wry` needs; the inventory names only "wry/WebView2". |
| test crates | `tokio` plus the Vela crates they exercise | |

The npm assignment is in each `package.json` (`apps/desktop`: React, Zustand, Motion, Tauri API and CLI, Vite, Tailwind,
Vitest, Testing Library, Playwright; `packages/ui`: React, Motion, Three.js, React Three Fiber; `packages/contracts`: Tauri API).

## Structure checks (CI-enforced)

- `scripts/check-crate-deps.mjs` reads `cargo metadata --no-deps` and enforces `IMPLEMENTATION_ARCHITECTURE.md` section 3
  rules 1 to 6 plus the frozen member list (an unknown crate fails). Unit tests cover allowed and forbidden edges, and a test
  builds a throwaway Cargo workspace where `vela-core` depends on `vela-git` and asserts the CLI exits 1.
  Rule 5 is also checked against the resolved build: `cargo tree -p vela-hook` must contain no `petgraph`, `ts-rs`, `rusqlite`, `libsqlite3-sys`, `wry` or Tauri crate.
  **Rule 7** (no Antigravity-specific API outside `vela-adapters::antigravity` and `vela-uia`) is not a dependency property and is
  left to review; it is not automated.
- `scripts/check-module-skeleton.mjs` compares each `lib.rs`/`main.rs` and its nested module files with
  `scripts/module-skeleton.json` (the machine-readable form of `SHARED_SURFACE_PROTOCOL.md` section 1). A missing module fails
  everywhere; an extra module fails at the crate root and inside closed groups (`antigravity`, `analysis`, `recovery`, `commands`,
  `wiring`, `plugins`). Groups the protocol writes as `x (+ y)` are open, because their owning ticket owns the module directory.
  TypeScript skeleton files are checked for existence.
- Every module is a directory `name/mod.rs`, matching the tickets' `name/**` write surfaces.

## Interpretations (gaps and one correction, recorded for later tickets)

1. **Optional `vela-domain` dependencies and empty features.** Rule 5 requires the hook binary to pull in no graph, SQLite, or Tauri
   code, so `petgraph`, `ts-rs` and `serde_json` sit behind the default `full` feature and the hook uses
   `default-features = false, features = ["hook-table"]` through a direct path dependency. F01 declares the empty features
   `hook-table` and `fault-injection` because that dependency cannot resolve without them. F02/A04 add modules under them and
   gate the corresponding modules (inside the module, never in `lib.rs`).
2. **`serde_json` in `vela-hook`.** Rule 5 says "only `serde`", but the hook protocol is JSON on stdin and stdout. `serde_json` is
   the serde data format, not a graph, SQLite, or Tauri dependency; the checker allows exactly `serde` and `serde_json`.
3. **`wiring/settings.rs`.** `SHARED_SURFACE_PROTOCOL.md` section 5 says S02 owns `wiring/settings.rs`, but the section 1 table omitted
   it. The table was corrected to include `settings` so S02 does not edit a shared declaration list.
4. **`preflight::antigravity`.** Section 1 writes `preflight (+ antigravity)`; ticket K10 writes `preflight/antigravity/**`, so it is
   a submodule directory. K10 writes `preflight/approval/**` too, which is why `preflight` is an open group.
5. **`packages/contracts` `exports`.** The `./generated/*` wildcard is declared now (F02 fills `src/generated`); there is no `main`
   and no barrel.
6. **Stubs fail loudly.** `vela-hook` and `fake-approval-window` are binary stubs that exit non-zero with a "not implemented"
   message, so neither can be mistaken for a working hook (a hook must fail closed) or harness.
7. **Files the layout lists but F04/F05 own.** (`tests/fixtures/` is created empty, with a `.gitkeep`; F03 owns its content.) `apps/desktop/index.html`, `vite.config.ts`, `src-tauri/main.rs`, `build.rs`,
   `tauri.conf.json`, and the renderer shell are created by F04/F05; F01 creates only manifests, the skeleton modules and placeholder tests.
8. **Added root files not in F01's write-surface list:** `.node-version`, `.npmrc`, `rustfmt.toml`, `.prettierrc.json`,
   `tsconfig.base.json`, `eslint.config.js`, `scripts/` (the two checkers, their tests and the skeleton manifest). All are required
   to pin tools, to enforce formatting and lint, or to implement the checkers F01 requires; none adds application behavior.
