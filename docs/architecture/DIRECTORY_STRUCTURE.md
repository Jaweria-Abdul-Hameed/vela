# Proposed Repository Structure

``` text
/
├─ AGENTS.md
├─ CLAUDE.md
├─ GEMINI.md
├─ CODEX.md
├─ CONTEXT.md
├─ docs/
│  ├─ product/
│  ├─ architecture/
│  ├─ orchestration/
│  ├─ agents/
│  ├─ git/
│  ├─ github/
│  ├─ ui/
│  ├─ testing/
│  ├─ security/
│  ├─ persistence/
│  ├─ operations/
│  ├─ decisions/
│  ├─ constraints/
│  ├─ research/
│  ├─ issues/
│  ├─ roadmap/
│  ├─ config/
│  └─ release/
├─ apps/
│  └─ desktop/
├─ crates/
│  ├─ vela-core/
│  ├─ vela-persistence/
│  ├─ vela-git/
│  ├─ vela-process/
│  └─ vela-adapters/
├─ packages/
│  ├─ ui/
│  ├─ contracts/
│  └─ test-fixtures/
└─ tests/
   ├─ fixtures/
   ├─ integration/
   └─ e2e/
```

Exact monorepo tooling remains an implementation decision, but domain
boundaries should remain recognizable.

# Runtime Adapter Directory Intent

Directory structure may reserve a provider/agent adapter boundary, but v1 production implementation should center the Antigravity adapter plus fake/test adapters. Empty or speculative Claude/Codex production implementations should not be created merely to make the directory appear symmetric.
