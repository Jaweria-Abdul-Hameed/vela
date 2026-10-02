# Repository Structure (Frozen)

This is the frozen layout. The rationale, crate boundaries, and dependency rules are in `IMPLEMENTATION_ARCHITECTURE.md`
(sections 2 and 3); component contracts are in `COMPONENT_SPECIFICATIONS.md`.

``` text
/
├─ AGENTS.md  CLAUDE.md  CODEX.md  GEMINI.md  CONTEXT.md  README.md
├─ DOCUMENTATION_INDEX.md  VELA_MASTER_BUILD_PLAYBOOK.md
├─ Cargo.toml                    (Cargo workspace)      rust-toolchain.toml
├─ package.json                  (npm workspaces: apps/*, packages/*)
├─ .editorconfig  .gitattributes  .gitignore
├─ docs/
│  ├─ product/  architecture/  orchestration/  agents/  git/  github/
│  ├─ ui/  testing/  security/  persistence/  operations/  decisions/
│  ├─ constraints/  research/  issues/  roadmap/  config/  release/
│  └─ project/                   (CURRENT_STATE.md: the cross-model checkpoint)
├─ apps/
│  └─ desktop/
│     ├─ index.html  vite.config.ts  tsconfig.json  package.json
│     ├─ src/                    (scenes, surfaces, state, ipc)
│     └─ src-tauri/              (crate vela-desktop: composition root, commands, events, tray, window, tauri.conf.json)
├─ crates/
│  ├─ vela-domain/               (pure types, ports, state machines, graph, policy, layout)
│  ├─ vela-persistence/          (SQLite, migrations/, journal, repositories)
│  ├─ vela-process/              (process runner, Job Objects, redaction, power, session probe)
│  ├─ vela-git/                  (GitAdapter, worktree manager, hardened invocation)
│  ├─ vela-adapters/             (antigravity, tracker_local, tracker_github, notifier)
│  ├─ vela-uia/                  (Windows UI Automation approval adapter, guarded visual module)
│  ├─ vela-hook/                 (binary: PreToolUse hook helper)
│  ├─ vela-core/                 (orchestrator, services, broker; no Tauri, no concrete adapters)
│  └─ vela-testkit/              (dev-only: fakes, fixture repos, scripted agent, fault points)
├─ packages/
│  ├─ contracts/                 (generated TypeScript types, typed invoke/listen wrappers)
│  ├─ ui/                        (design, components, canvas, a11y, state selectors)
│  └─ test-fixtures/             (sample snapshots and event streams)
├─ tools/
│  └─ fake-approval-window/      (WebView2 window reproducing the verified approval-card roles)
└─ tests/
   ├─ fixtures/  integration/  e2e/
   └─ antigravity-compat/        (real-environment suite, gated by VELA_REAL_ANTIGRAVITY=1)
```

Rules:

-   Crate dependency direction and the CI-enforced checks are in `IMPLEMENTATION_ARCHITECTURE.md` section 3.
-   Only `vela-adapters::antigravity` and `vela-uia` may contain Antigravity-specific behavior (ADR-008, ADR-018).
-   Empty or speculative Claude/Codex/other production runtime adapters are not created (ADR-008); fakes live in
    `vela-testkit`.
-   Test repositories are generated outside cloud-synced folders and never inside this repository.
-   Exact tool and dependency versions are pinned by the bootstrap ticket.
