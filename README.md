# Vela --- Autonomous AI Engineering Control Plane

> **Status:** Product definition overview; normative precedence is defined in `DOCUMENTATION_INDEX.md`\
> **Target:** Windows-first native desktop application\
> **Primary purpose:** Safely orchestrate long-running, multi-issue AI
> coding work across isolated agent sessions while making the execution
> understandable, recoverable, and visually exceptional.

## 1. Product thesis

Vela is not another IDE and is not a chat wrapper. It is a local
engineering orchestration application that sits above coding agents and
turns a repository, a set of specifications, and a dependency-aware
issue graph into a controlled autonomous build.

The user should be able to prepare the repository and Markdown context,
open Vela, select the project, inspect the proposed execution graph,
press **Build**, and leave the computer. Vela is responsible for
understanding what can run concurrently, creating isolated workspaces,
starting fresh agent contexts, invoking implementation and review
workflows, running tests, committing and pushing safe checkpoints,
merging completed work into an integration branch, advancing the
dependency frontier, recovering from interruption, and stopping when
human judgment is genuinely required.

The experience must feel less like Jira and less like a terminal
dashboard and more like watching a software system assemble itself.

## 2. Non-negotiable product principles

1.  **The repository is the source of truth.** Agents receive durable
    context through versioned Markdown, issues, commits, tests, ADRs,
    and code---not through hidden conversational memory.
2.  **Fresh context per ticket.** One ticket is implemented in one fresh
    execution context unless a documented recovery operation resumes it.
3.  **Tracer bullets over horizontal layers.** Tickets should be
    end-to-end vertical slices whenever practical.
4.  **Parallelism is earned, not assumed.** Two tickets run concurrently
    only when dependency, predicted-write-set, contract, migration, and
    semantic-conflict checks permit it.
5.  **Isolation before concurrency.** Parallel workers use separate Git
    branches and worktrees.
6.  **Review is a gate.** Implementation is not complete merely because
    code was generated.
7.  **Tests are evidence.** Vela records commands, outcomes, and
    relevant logs.
8.  **Git history is a recovery mechanism.** Durable checkpoints precede
    risky transitions.
9.  **Human intervention is exceptional but first-class.** Ambiguous
    requirements, destructive actions, credential prompts, unresolved
    conflicts, and policy violations become explicit `NEEDS_HUMAN`
    states.
10. **No brittle pixel automation when a supported interface exists.**
    Prefer Antigravity settings/CLI/SDK and Git/GitHub APIs over
    coordinate clicking.
11. **The UI is an operational instrument.** Animation communicates
    actual system state.
12. **Idle beauty must not waste resources.** Rendering and animation
    throttle aggressively when unfocused/minimized.

## 3. Canonical build flow

``` text
Repository + Markdown + Issue Tracker
                │
                ▼
            PRE-FLIGHT
                │
                ▼
       CONTEXT / REPO ANALYSIS
                │
                ▼
      ISSUE GRAPH CONSTRUCTION
                │
                ▼
   PARALLELIZATION SAFETY ANALYSIS
                │
                ▼
        USER GRAPH APPROVAL
                │
                ▼
      CREATE INTEGRATION BRANCH
                │
                ▼
          READY FRONTIER
       ┌────────┼────────┐
       ▼        ▼        ▼
    worker A worker B worker C
    worktree worktree worktree
       │        │        │
   implement implement implement
       │        │        │
      test     test      test
       │        │        │
    checkpoint checkpoint checkpoint
       │        │        │
     review    review    review
       │        │        │
    fix loop  fix loop  fix loop
       └────────┼────────┘
                ▼
          MERGE WORKERS
                │
                ▼
       INTEGRATION TEST GATE
                │
                ▼
        ADVANCE FRONTIER
                │
          repeat until done
                ▼
       FINAL INTEGRATION REVIEW
                │
                ▼
          PUSH / PR / COMPLETE
```

## 4. Documentation map

Start with:

-   `AGENTS.md` --- universal instructions for any coding agent.
-   `docs/product/PRODUCT_SPEC.md` --- complete behavior.
-   `docs/architecture/SYSTEM_ARCHITECTURE.md` --- components and
    boundaries.
-   `docs/orchestration/ORCHESTRATION_ENGINE.md` --- execution state
    machine.
-   `docs/orchestration/PARALLELIZATION.md` --- dependency and
    concurrency rules.
-   `docs/ui/UI_UX_SPEC.md` --- visual and interaction source of truth.
-   `docs/ui/MOTION_AND_3D.md` --- reactive grid, spatial graph,
    animation, performance.
-   `docs/git/GIT_WORKFLOW.md` --- branches, worktrees, commits, merges.
-   `docs/agents/AGENT_PROTOCOL.md` --- session contract.
-   `docs/agents/REVIEW_PROTOCOL.md` --- review/fix loop.
-   `docs/testing/TEST_STRATEGY.md` --- validation gates.
-   `docs/issues/ISSUE_AUTHORING.md` --- converting this pack into
    tracer-bullet GitHub issues.
-   `docs/roadmap/IMPLEMENTATION_PLAN.md` --- staged delivery.

## 5. Current external assumptions

Vela must treat third-party behavior as an adapter, not as an eternal
invariant. As of the research date, Antigravity 2.0 on Windows exposes
Terminal Command Auto Execution modes including **Always Proceed**,
while the newer unified permission engine is documented differently
across operating systems. Matt Pocock's current engineering skills
include `implement`, `code-review`, and `implement-spec`;
`implement-spec` treats tickets as a blocking graph and uses isolated
worktrees for parallel implementers. These facts are useful integration
targets, but Vela must detect capabilities at runtime and degrade
safely rather than hard-code assumptions.

## 6. Definition of product success

Vela succeeds when a user can hand it a well-prepared repository and
issue graph, start a build, walk away, and later answer all of these
questions without guessing:

-   What is running?
-   Why was it allowed to run in parallel?
-   Which agent/session owns it?
-   Which branch/worktree contains it?
-   What commands were executed?
-   Which tests passed or failed?
-   What did review find?
-   What was fixed?
-   What was committed and pushed?
-   What is blocking the next issue?
-   Can the run recover after a crash or reboot?
-   What requires human action?
-   Is the integration branch currently healthy?

# Critical Compatibility Requirement: Approval Broker

The product requirement for unattended execution includes environments
where Antigravity still presents approval prompts despite permissive
settings. Vela therefore includes a policy-driven Approval Broker,
Approval Watchdog, and guarded UI-automation fallback (the UI-automation tiers are opt-in at onboarding, ADR-010; Vela does not depend on unconditional native auto-execution, ADR-009). This does **not**
replace or weaken the earlier safety model: Vela classifies the action
first, then uses the best available mechanism to communicate an
allow/ask/deny decision.

# Vela v1 Product Focus: Antigravity First

Vela v1 is an **Antigravity-first, provider-extensible** autonomous engineering control plane for Windows.

Google Antigravity is the default, first-class, required runtime execution environment for v1. Complete and reliable Antigravity orchestration is release-blocking.

The provider-neutral core and `AgentAdapter` boundary exist to:
- isolate Vela from changes in an external product;
- keep scheduler/orchestration logic clean;
- permit future runtime integrations.

They do **not** mean v1 must implement Claude, Codex, Gemini CLI, or other coding-agent runtimes as peers.

Claude, Gemini, Codex, or other models/tools may be used **to build Vela**. That does not make them Vela v1 runtime targets.

Unless an explicitly approved issue expands runtime scope, engineering effort must prefer deeper, safer, more reliable Antigravity integration over adding another provider.

# Development Commands (F01 bootstrap)

Pinned toolchain: Rust **1.99.0** (`rust-toolchain.toml`), Node **24.19.0** (`.node-version`), npm **11.17.0**
(`packageManager`), TypeScript **6.0.3**. The full pin list, the per-crate dependency assignment and the
structure rules are in `docs/architecture/TOOLCHAIN_AND_DEPENDENCIES.md`. Prerequisites on Windows: the MSVC
build tools with a Windows SDK, `rustup` (with `%USERPROFILE%\.cargo\bin` on `PATH`), Node, and Git.

| Purpose | Command |
|---|---|
| Install (from the committed lockfile) | `npm ci` |
| Verify everything (what CI runs) | `npm run verify` |
| Verify TypeScript only | `npm run verify:ts` (format check, ESLint, typecheck, Vitest, checker tests) |
| Verify Rust only | `npm run verify:rust` (rustfmt, clippy, build, test, structure checks) |
| Format | `npm run fmt` (Prettier and `cargo fmt`); check only: `npm run fmt:check` |
| Lint | `npm run lint:ts` (ESLint); `npm run lint:rust` (clippy, warnings are errors) |
| Typecheck | `npm run typecheck` |
| Test | `npm run test:ts`; `npm run test:rust` (`cargo test --workspace --locked`) |
| Build | `npm run build:rust` (`cargo build --workspace --locked --all-targets`) |
| Crate dependency direction and module skeleton | `npm run check:structure` |

Rust builds and tests always use `--locked`; both lockfiles (`Cargo.lock`, `package-lock.json`) are committed and
are never hand-merged (`docs/issues/graph/SHARED_SURFACE_PROTOCOL.md` section 2). There is no application to launch
until the Tauri shell ticket (F04); the development command for the app is documented by that ticket.
