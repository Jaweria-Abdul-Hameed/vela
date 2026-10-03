# Vela --- Master AI Build & Handoff Prompt Playbook

> **Purpose:** Canonical start-to-finish operating playbook for building
> Vela from the repository specification pack across Claude, Gemini,
> Codex, Antigravity, or other capable coding agents. Agent
> conversations are disposable; repository state is durable.


## Zero-code repository setup before Prompt 1

When starting Vela from the specification pack:

1. Create the `Vela` folder in a local, non-cloud-synced development directory; this is the recommended and reference development layout (on Windows, for example, `C:\Users\<username>\Projects\Vela`). Avoid developing directly inside folders that OneDrive, Dropbox, Google Drive, or a similar service actively synchronizes (on Windows the Desktop and Documents folders are often redirected into OneDrive), because Vela uses Git worktrees, concurrent workers, filesystem watching, and process orchestration, which a sync client can disturb. This is guidance, not a claim that a synced folder can never work: Vela's own preflight (`docs/operations/PREFLIGHT.md`) warns when the repository is under a known sync provider and blocks a worktree root there.
2. Extract the contents of the Vela specification pack directly into that folder; do not keep an unnecessary wrapper directory.
3. Place `VELA_MASTER_BUILD_PLAYBOOK.md` in the repository root.
4. Initialize Git and commit the documentation baseline.
5. Create an empty GitHub repository named `vela`, connect it as `origin`, and push `main`.
6. Open the entire `Vela` folder in the first AI coding environment.
7. Run Prompt 1 before creating application code.

Do **not** manually create the Tauri project, React project, `package.json`, Rust crates, SQLite database, or source directory structure before the architecture/bootstrap phases. Those must be created from the frozen specifications so the initial implementation does not accidentally pre-decide architecture.

Prompt 1 itself remains comprehension-only. After its report is accepted, create the first `docs/project/CURRENT_STATE.md` checkpoint, commit/push that durable state, and then proceed to Prompt 2.

## 0. Core operating doctrine

The Markdown specification pack, ADRs, GitHub issues, tests, source
code, and Git history are the source of truth. Never rely on an
important decision existing only inside an AI chat. Before changing
models, accounts, conversations, or agents, persist every durable
decision into the repository, commit it, and push it.

The intended lifecycle is:

``` text
MD specification pack
→ comprehension
→ specification audit
→ specification corrections
→ current external-assumption verification
→ architecture freeze
→ GitHub issue generation
→ independent issue/DAG audit
→ scheduler dry-run
→ repository bootstrap
→ implementation waves
→ per-ticket fresh contexts
→ tests
→ durable checkpoint
→ fixed-point code review
→ fix/retest/re-review
→ serialized integration
→ UI fidelity review
→ real Antigravity approval validation
→ resilience/security/performance passes
→ final requirements traceability
→ final whole-codebase review
→ release validation
```

### Non-negotiable cross-agent rules

-   Any capable model may take over at a phase boundary.
-   A new agent must reconstruct state from the repository rather than
    previous chat history.
-   Do not redo completed phases merely because a different model takes
    over.
-   Accepted ADRs are not casually reopened.
-   A genuine contradiction may trigger an ADR update/superseding ADR.
-   Every implementation ticket should use a fresh context whenever
    practical.
-   Parallel work requires dependency safety, write-surface safety, and
    contract safety.
-   When uncertain about parallelism, serialize.
-   Worker branches/worktrees remain isolated.
-   Review is performed against a durable Git fixed point.
-   Tests must not be weakened merely to obtain green status.
-   Subjective review comments must not create endless loops.
-   Native Antigravity permission handling is preferred, but the product
    must support the observed case where approval prompts persist.
-   Vela approves operations, not buttons.
-   Never automate passwords, 2FA, CAPTCHA, or unauthorized quota
    circumvention.

------------------------------------------------------------------------

# 1. Repository preparation

Place the complete specification pack in the repository. A
representative top-level structure is:

``` text
vela/
├── AGENTS.md
├── CLAUDE.md
├── GEMINI.md
├── CODEX.md
├── CONTEXT.md
├── README.md
├── DOCUMENTATION_INDEX.md
├── docs/
│   └── ...complete specification pack...
└── [source code created later]
```

Before implementation, establish a documentation baseline:

``` bash
git init
git add .
git commit -m "docs: establish Vela product and architecture specification"
```

Push this baseline to the canonical GitHub repository before substantial
implementation.

## Recommended addition: `docs/project/CURRENT_STATE.md`


### Mandatory lifecycle for `CURRENT_STATE.md`

`docs/project/CURRENT_STATE.md` becomes mandatory immediately after Prompt 1 is accepted.

**Prompt 1 is the only read-only exception.** It proves comprehension without modifying files. Once the user accepts its report, create `CURRENT_STATE.md` before Prompt 2 begins.

From then onward, **every numbered phase prompt (Prompts 2–21), every cross-model handoff, and every material implementation/recovery operation must verify and update `CURRENT_STATE.md` before declaring work complete.**

Keep it concise. It is a checkpoint, not a duplicate specification. Record, where applicable:

- product: `Vela`;
- current phase;
- last completed phase;
- next phase;
- canonical branch and commit SHA;
- specification status;
- architecture status;
- issue-graph status;
- implementation status;
- integration branch;
- active workers/worktrees;
- open human decisions;
- accepted/superseding ADRs relevant to current work;
- external research last verified date;
- known compatibility constraints, especially persistent Antigravity approvals and the required Approval Broker fallback;
- current blockers/failures;
- recovery/reconciliation status;
- concise next-agent instructions.

Rules:

1. Never mark a phase complete before its required validation passes.
2. `CURRENT_STATE.md` summarizes reality; it never overrides Git, tests, ADRs, specifications, or tracker state.
3. If it conflicts with concrete repository state, correct it and record the discrepancy.
4. Never put secrets, tokens, credentials, or unnecessary logs in it.
5. Update it in the same commit as the phase-boundary work when practical.
6. Before switching models, commit and push the checkpoint and verify the worktree is clean unless intentionally preserving an in-progress worker.
7. A fresh Claude, Gemini, Codex, Antigravity, or future agent must be able to determine exactly where to continue from the canonical repository plus this checkpoint.

Create and maintain a concise durable handoff checkpoint:

``` markdown
# Vela Current State

## Current Phase
ARCHITECTURE_FROZEN

## Last Completed Phase
Architecture Freeze

## Next Phase
GitHub Issue Generation

## Canonical Commit
<git-sha>

## Specification Status
READY

## Architecture Status
FROZEN

## Issue Graph
NOT_CREATED

## Implementation
NOT_STARTED

## Integration Branch
<configured branch>

## Active Workers
None

## Open Human Decisions
None

## Accepted ADRs
<list current accepted/superseding ADRs>

## External Research Last Verified
<YYYY-MM-DD>

## Known Compatibility Constraints
Antigravity may continue producing approval prompts despite permissive native settings.
Approval Broker + guarded UI automation fallback is therefore required.

## Next Agent Instructions
<precise next phase; do not reopen completed work without evidence>
```

Update this file at every major phase boundary and commit it. It is a
checkpoint, not a replacement for the full specifications.

------------------------------------------------------------------------

# 2. Universal cross-model handoff prompt

Use this whenever moving Claude → Gemini → Codex → Antigravity, opening
a fresh chat, changing accounts, or recovering after a lost context.
Append the next phase prompt after it.

``` text
You are taking over an existing Vela development repository from another AI agent.


Before continuing, preserve the canonical runtime-scope distinction: Vela v1 is **Antigravity-first, provider-extensible**. Antigravity on Windows is the required production runtime. Claude/Gemini/Codex may have been used by previous agents to build Vela; that does not make them runtime targets. Read ADR-006 and ADR-008 together and ensure `CURRENT_STATE.md` reflects Antigravity runtime readiness/blockers.


`docs/project/CURRENT_STATE.md` is a required handoff checkpoint after Prompt 1. Read it near the start of reconstruction. If it is missing when Prompt 1 should already have been completed, do not guess: reconstruct state from Git, ADRs, specifications, issues/tracker, source, and tests; then create/correct `CURRENT_STATE.md` to match reality before continuing.

At the END of the handoff/phase, update `CURRENT_STATE.md` again with the phase actually completed, next phase, canonical branch/HEAD, blockers/human decisions, and concise next-agent instructions. Commit and push that durable checkpoint before another model handoff whenever repository policy permits.

IMPORTANT:

You have NO dependency on the previous agent's conversation history.

The repository is the canonical source of truth.

Do not assume anything based on what a previous model may have decided unless that decision is persisted in:
- repository documentation;
- ADRs;
- GitHub issues;
- source code;
- tests;
- Git history.

First establish the current project state.

Read in this order:

1. AGENTS.md
2. docs/project/CURRENT_STATE.md if present
3. CONTEXT.md
4. README.md
5. DOCUMENTATION_INDEX.md
6. relevant ADRs
7. current Git status/history
8. current GitHub issues/milestones/dependencies if available
9. documents relevant to the phase I am asking you to continue.

Determine:

- what phases have already been completed;
- what architectural decisions are frozen;
- what specification changes were made;
- what implementation exists;
- what remains;
- whether the working tree is clean;
- current branch and HEAD;
- whether documentation and implementation are consistent;
- whether there is unfinished work from the previous agent;
- whether CURRENT_STATE.md accurately reflects repository reality.

DO NOT redo completed phases merely because you did not personally perform them.

DO NOT silently reinterpret or replace accepted architectural decisions.

If something important appears to exist only in previous conversation history and is not persisted in the repository, flag it as missing context rather than guessing.

If CURRENT_STATE.md conflicts with Git/source/tests, treat the concrete repository state as evidence and report the discrepancy before proceeding.

After reconstructing state, continue with the phase prompt appended below.
```

### Handoff completion rule

Before relinquishing control, the outgoing agent must persist important
discoveries/decisions, update `CURRENT_STATE.md`, leave the worktree in
an understood state, commit the intended durable state, and push when
repository policy permits. A sentence spoken only in chat is not a
handoff artifact.

------------------------------------------------------------------------




## Canonical V1 runtime scope — applies to every prompt

This rule applies to the entire playbook and must be carried through comprehension, architecture, issue generation, implementation, review, handoff, validation, and release:

> **Vela v1 is Antigravity-first, provider-extensible.**

Google Antigravity on Windows is the default, first-class, required, release-blocking runtime execution environment for Vela v1.

The provider-neutral `AgentAdapter` architecture exists to isolate the core from external-product changes and permit future integrations. It does **not** require v1 to implement Claude, Codex, or other production runtime adapters.

Claude, Gemini, Codex, Antigravity, and other tools may be used **to build, audit, or review Vela**. Build-time agent choice is separate from Vela's shipped runtime scope.

For all prompts below:

- prioritize complete Antigravity capability discovery, execution, approvals, recovery, skills, and end-to-end validation;
- preserve provider-neutral core interfaces;
- use fake/test adapters where useful for deterministic core tests;
- do not generate or implement peer Claude/Codex/other runtime-provider tickets unless an approved specification change explicitly adds them;
- do not silently fall back to an unrelated provider when required Antigravity capabilities are missing;
- record Antigravity runtime readiness and blockers in `docs/project/CURRENT_STATE.md`;
- treat ADR-006 (provider-independent core) and ADR-008 (Antigravity-first product scope) as complementary.

## Universal phase-checkpoint rule for Prompts 1–21

Apply this rule **in addition to every prompt below**:

- **Prompt 1:** do not modify files while producing the comprehension report. After the user accepts the report, create `docs/project/CURRENT_STATE.md` and record that comprehension is complete and Prompt 2 is next.
- **Prompts 2–21:** before reporting the prompt/phase complete, reread repository reality and update `docs/project/CURRENT_STATE.md` with the completed phase, next phase, canonical branch/HEAD, material status changes, blockers/human decisions, and durable compatibility/recovery information.
- If a prompt is analysis-only and changes no other repository files, the accepted phase checkpoint is still a legitimate durable update.
- If work stops at `NEEDS_HUMAN`, `REVIEW_STALLED`, `APPROVAL_STALLED`, `FAILED`, or another exceptional state, record that state rather than falsely advancing the phase.
- Commit/push the checkpoint at phase boundaries according to repository Git policy before a model/provider handoff.

This is what makes private model conversations disposable while repository state remains durable.

# 3. Prompt 1 --- Complete project comprehension

Use a strong reasoning model. Do not code.

``` text
You are taking ownership of a new software project called Vela.

The repository contains an extensive Markdown specification pack describing the complete product: product requirements, architecture, orchestration, agent protocols, Git/worktree strategy, approval automation, recovery, security, persistence, testing, GitHub workflow, UI/UX, motion/3D behavior, accessibility, performance, ADRs, constraints, external integrations, and implementation planning.

IMPORTANT:

DO NOT IMPLEMENT ANYTHING YET.
DO NOT GENERATE SOURCE CODE.
DO NOT CREATE ISSUES YET.
DO NOT REDESIGN THE PRODUCT.

Your first task is complete project comprehension.

Start with:

1. AGENTS.md
2. CONTEXT.md
3. README.md
4. DOCUMENTATION_INDEX.md

Then systematically inspect every relevant document under docs/.

Treat the repository Markdown as the canonical product context.

Build a complete mental model of:

- what Vela is;
- what Vela is explicitly not;
- the intended user workflow;
- the full autonomous execution lifecycle;
- project preflight;
- issue ingestion;
- issue DAG generation;
- dependency handling;
- conservative parallelization;
- predicted write-set/conflict analysis;
- tracer-bullet development;
- fresh agent context per issue;
- Git branch/worktree isolation;
- Matt Pocock skill integration;
- /implement;
- testing;
- checkpoint-before-authoritative-review;
- /code-review;
- review/fix/retest/re-review loops;
- integration branch behavior;
- serialized merge lane;
- GitHub interaction;
- push behavior;
- recovery and reconciliation;
- crash/reboot handling;
- Stop All;
- human intervention;
- execution profiles and capacity;
- Antigravity integration;
- the Approval Broker;
- Approval Watchdog;
- native approval delivery;
- Windows UI Automation fallback;
- guarded visual fallback;
- approval-loop detection;
- security boundaries;
- provider independence;
- persistence/event journal;
- observability;
- the complete Stitch-inspired UI;
- cursor-reactive dot field;
- ambient violet/blue/cyan fields;
- spatial dependency constellation;
- glass surfaces;
- graph interactions;
- event-driven animation;
- accessibility;
- reduced motion;
- graphics performance;
- native Windows desktop behavior;
- Tauri/React/Rust boundaries;
- testing and release expectations.

Pay particular attention to ADRs because they explain decisions that must not be casually reopened.

Do not assume that one document is more recent simply because you happened to read it later. Resolve precedence using explicit ADR supersession and the documentation hierarchy.

In particular, understand that ADR-007 supersedes the absolute interpretation of ADR-004: native Antigravity permission mechanisms are preferred, but guarded UI automation is required as a fallback because real-world approval prompts may persist.

After reading everything, produce ONLY a PROJECT COMPREHENSION REPORT containing:

1. Product mission
2. Primary user journey
3. Major subsystems
4. Architecture summary
5. Complete orchestration lifecycle
6. Agent execution lifecycle
7. Git/worktree lifecycle
8. Review lifecycle
9. Parallelization model
10. Approval Broker architecture
11. Recovery model
12. Security model
13. Persistence model
14. UI/UX philosophy
15. UI rendering/motion architecture
16. External integrations
17. Major architectural invariants
18. Non-goals
19. Highest-risk technical areas
20. Any apparent contradictions, ambiguities, duplicate requirements, missing decisions, or implementation-blocking gaps you discovered.

For every possible contradiction or gap, cite the exact repository Markdown files involved.

Do not solve those contradictions yet.
Do not modify files.
Do not code.

The purpose of this phase is to prove that you understand the complete project before planning implementation.
```

**Gate:** Correct misunderstandings now. Do not proceed while the
comprehension report materially misrepresents the product.

------------------------------------------------------------------------

# 4. Prompt 2 --- Specification audit

``` text
Continue working on Vela.

You have now read the complete specification pack and produced a project comprehension report.

Still DO NOT IMPLEMENT THE APPLICATION.

Perform a rigorous specification audit.

Read the relevant documents again whenever necessary. Do not rely purely on your previous summary.

Audit the complete specification for:

- contradictions;
- missing requirements;
- undefined state transitions;
- impossible state transitions;
- unclear ownership between components;
- missing failure paths;
- missing recovery paths;
- unsafe Git behavior;
- concurrency races;
- parallel-worker conflicts;
- stale-context risks;
- review-loop problems;
- approval automation edge cases;
- Windows-specific edge cases;
- Tauri lifecycle issues;
- renderer/backend ownership problems;
- persistence crash-consistency issues;
- event-journal inconsistencies;
- missing idempotency requirements;
- provider-specific assumptions leaking into core architecture;
- security issues;
- prompt-injection risks;
- secrets exposure;
- GitHub outage behavior;
- network interruption;
- laptop sleep/restart;
- agent crashes;
- orphaned child processes;
- missing worktrees;
- dirty repositories;
- external repository modifications;
- branch divergence;
- merge conflicts;
- rate limits/capacity exhaustion;
- ambiguous approval prompts;
- wrong-window auto-approval;
- repeated approval loops;
- Antigravity UI version drift;
- accessibility omissions;
- reduced-motion omissions;
- high-DPI/multi-monitor issues;
- UI performance risks;
- WebGL lifecycle/resource leaks;
- large dependency graphs;
- installation/update issues;
- diagnostics/logging omissions;
- anything that would make unattended execution unreliable.

Also check whether every functional requirement has a plausible implementation and verification path.

Classify every finding as:

BLOCKING
IMPORTANT
MINOR
NOT ACTUALLY A PROBLEM

For each real finding provide:

- exact documents involved;
- why it matters;
- concrete scenario that triggers it;
- recommended specification change;
- whether it requires an ADR.

Do NOT make speculative product changes just because you personally prefer another design.

If the existing specification already resolves something, cite that resolution rather than reporting a false gap.

End with one of:

SPECIFICATION READY FOR ARCHITECTURE FREEZE

or

SPECIFICATION NOT READY

with the exact blocking items.

Do not implement anything.
```

------------------------------------------------------------------------

# 5. Prompt 3 --- Apply legitimate specification fixes

Run only if the audit found genuine BLOCKING/IMPORTANT issues.

``` text
Using the specification audit you just completed, update the repository documentation to resolve ONLY findings classified BLOCKING or IMPORTANT that represent genuine gaps or contradictions.

Rules:

1. Preserve all existing product requirements unless a contradiction requires clarification.
2. Do not silently remove behavior.
3. Do not simplify Vela merely to make implementation easier.
4. Use ADRs for architectural decisions with meaningful alternatives/tradeoffs.
5. If an existing ADR is superseded, preserve it and create a superseding ADR rather than deleting history.
6. Update every affected document so the specification remains internally consistent.
7. Update requirements traceability where relevant.
8. Update DOCUMENTATION_INDEX.md for newly created documents.
9. Do not write source code.
10. Do not create GitHub implementation issues yet.

After modifications:

- run a second consistency audit;
- provide the files changed;
- explain each specification change;
- explicitly confirm whether any original requirement was removed;
- identify any remaining human decisions.

If no human decision remains, finish with:

ARCHITECTURE SPECIFICATION READY TO FREEZE

Update docs/project/CURRENT_STATE.md to reflect this phase if that file exists.
```

Suggested commit:

``` text
docs: resolve specification audit findings
```

------------------------------------------------------------------------

# 6. Prompt 4 --- Verify volatile external assumptions

This phase must use current primary sources rather than model memory.

``` text
Before implementation planning, verify every external/volatile assumption in the Vela specifications against CURRENT primary documentation.

Do not use memory for current external-tool behavior.

Research primary sources for:

1. Google Antigravity current Windows behavior
2. Antigravity permission settings
3. Antigravity terminal auto-execution
4. Antigravity CLI/headless capabilities
5. Antigravity SDK/automation capabilities if available
6. Antigravity session lifecycle/recovery capabilities
7. Antigravity approval behavior
8. Antigravity Windows UI limitations relevant to our Approval Broker
9. Matt Pocock's current skills repository
10. implement
11. implement-spec
12. code-review
13. setup/installation of those skills
14. fixed-point review behavior
15. Git worktree assumptions
16. current Tauri stable architecture relevant to this project
17. any other external API/tool on which the specifications depend.

Compare current reality against:

docs/research/
docs/agents/MATT_POCKOCK_SKILLS.md
docs/orchestration/APPROVAL_BROKER.md
relevant ADRs
and other affected documents.

Classify each assumption:

VERIFIED
CHANGED
UNDOCUMENTED
UNSUPPORTED
REQUIRES RUNTIME CAPABILITY DETECTION

IMPORTANT:

The user's actual observed environment is evidence that Antigravity approval prompts may persist despite permissive settings. Do not erase that product requirement merely because documentation claims a permissive mode exists.

If documentation and observed behavior differ, Vela must remain defensive/capability-detected.

Update dated research documents where appropriate.

Do not start application implementation.

Finish with an EXTERNAL INTEGRATION READINESS REPORT.
Update CURRENT_STATE.md and persist any durable external-integration conclusions before handoff.
```

------------------------------------------------------------------------

# 7. Prompt 5 --- Architecture freeze


> **V1 scope guard for this phase:** preserve `AgentAdapter` as the architectural seam, but make `AntigravityAdapter` the required/default production implementation. Do not design equal-weight Claude/Codex runtime implementations. Fake adapters are still expected for tests.

``` text
We are ready to freeze Vela's implementation architecture.

Read:

AGENTS.md
PRODUCT_SPEC
SYSTEM_ARCHITECTURE
DOMAIN_MODEL
ADAPTERS
all orchestration documents
all persistence documents
all security documents
all UI documents
all ADRs
the latest external-integration research

Do not reopen accepted ADRs without discovering an actual contradiction.

Produce the concrete implementation architecture.

Resolve:

- exact workspace/monorepo layout;
- Tauri application structure;
- React/TypeScript frontend structure;
- Rust crate boundaries;
- whether any Python sidecar is genuinely necessary;
- typed IPC contracts;
- event streaming;
- SQLite access and migrations;
- event journal implementation;
- scheduler ownership;
- worker lifecycle;
- process execution;
- cancellation;
- Git adapter;
- worktree manager;
- GitHub adapter;
- agent adapter;
- Antigravity adapter;
- Approval Broker;
- Windows UI Automation adapter boundary;
- visual approval fallback boundary;
- policy engine;
- recovery/reconciliation;
- logging/redaction;
- configuration;
- dependency graph representation;
- frontend state management;
- R3F/Three.js boundary;
- DOM vs WebGL responsibilities;
- graphics-quality modes;
- testing architecture;
- fixture repositories;
- packaging.

Prefer the smallest number of technologies that satisfy the specifications.

Do not introduce infrastructure because it is fashionable.

For every major component specify:

INPUTS
OUTPUTS
STATE OWNED
DEPENDENCIES
FAILURE MODES
TEST STRATEGY

Then verify that the architecture can implement every FR in PRODUCT_SPEC.

If new ADRs are necessary, create them.

Update DIRECTORY_STRUCTURE.md and affected architecture docs.
Update CURRENT_STATE.md with architecture status, canonical commit placeholder/current SHA, next phase, and remaining human decisions.

Do not implement features yet.

Finish by stating whether architecture is ready for ticket generation.
```

**Phase boundary:** commit and push. This is an excellent point to
switch from Claude to Gemini or any other model using the universal
handoff prompt.

------------------------------------------------------------------------

# 8. Prompt 6 --- Generate the complete GitHub issue graph


> **V1 scope guard for this phase:** generate concrete Antigravity production-integration tickets and fake/test-adapter tickets, but do not generate Claude/Codex/other production runtime-adapter tickets unless an approved scope change explicitly requires them.

``` text
Now convert the frozen Vela specification into the complete implementation issue graph.

This is a planning operation, not implementation.

First inspect the currently installed Matt Pocock engineering skills. If an appropriate specification-to-ticket workflow such as /to-tickets is available, use it according to its current documentation.

The repository specifications remain authoritative.

Create GitHub issues for the complete product.

Ticket philosophy:

- tracer bullets;
- vertical slices;
- one fresh implementation context whenever practical;
- independently testable;
- independently reviewable;
- explicit dependencies;
- no fake sequential dependencies;
- safe parallelism where real;
- small enough for reliable agent implementation;
- large enough to deliver meaningful behavior.

Every issue MUST contain:

1. Outcome-oriented title
2. Objective
3. Requirements IDs
4. User-visible behavior where applicable
5. Technical scope
6. Architectural constraints
7. Dependencies / blocked-by relationships
8. Expected write surfaces
9. Known shared contracts
10. Parallelization notes
11. Acceptance criteria
12. Required unit tests
13. Required integration tests
14. Required UI/visual tests where relevant
15. Required manual evidence where relevant
16. Relevant specification documents
17. Relevant ADRs
18. Non-goals
19. Definition of done

Cover ALL product areas, including:

foundation
Tauri shell
React frontend
Rust core
typed IPC
SQLite
event journal
project import
preflight
context discovery
local issues
GitHub issues
DAG
cycle detection
parallelization
predicted write sets
scheduler
worktrees
process execution
agent sessions
Matt Pocock integration
testing
checkpointing
review
review/fix loop
merge lane
integration validation
Git push
GitHub PR behavior
recovery
reconciliation
Stop All
pause/resume
human intervention
execution profiles
capacity
Approval Broker
Approval Watchdog
native approval path
Windows UI Automation
guarded visual fallback
approval-loop detection
security/policy
prompt injection
observability
diagnostics
home UI
reactive dots
ambient gradients
dependency constellation
node states
issue inspector
timeline
conflict visualization
run controls
accessibility
reduced motion
graphics quality
performance
large graphs
installer
updates
release validation.

DO NOT accidentally omit "boring" infrastructure because UI work is more visible.

After issue creation, construct the actual dependency DAG from the created issue IDs.
Update CURRENT_STATE.md with issue-graph status and next phase.

Do not implement anything yet.
```

------------------------------------------------------------------------

# 9. Prompt 7 --- Independent issue/DAG audit

Prefer a different model or at least a fresh context.

``` text
Perform an independent audit of the complete Vela GitHub issue graph.

Do not implement anything.

Read the canonical specifications first and then inspect every generated issue.

Your job is to find:

- missing requirements;
- duplicated tickets;
- tickets too large for one context;
- tickets too small to be useful;
- horizontal-layer tickets that should be tracer bullets;
- missing acceptance criteria;
- weak test requirements;
- missing ADR references;
- incorrect dependencies;
- unnecessary dependencies;
- dependency cycles;
- issues that claim to be parallel but share dangerous write surfaces;
- issues serialized unnecessarily;
- architecture work accidentally deferred until after dependents;
- UI tickets that ignore the visual specification;
- approval automation gaps;
- recovery gaps;
- security gaps;
- packaging gaps.

Construct the dependency DAG yourself independently from issue metadata and compare it with the declared graph.

For every pair/group proposed for parallel execution, analyze:

- file/module overlap;
- API/type contracts;
- database/schema changes;
- configuration;
- lockfiles;
- generated artifacts;
- shared test fixtures;
- architectural coupling;
- semantic dependency.

When uncertain, prefer sequential execution.

Correct the issues and dependencies where required.

Then produce:

- total issue count;
- dependency graph;
- critical path;
- initial ready frontier;
- safely parallel groups;
- explicitly serialized groups and reasons;
- requirements coverage matrix;
- any remaining human decisions.

Update CURRENT_STATE.md if the graph changed materially.
Do not code.
```

------------------------------------------------------------------------

# 10. Prompt 8 --- Scheduler dry-run

``` text
Simulate Vela's entire implementation issue graph WITHOUT modifying source code.

Starting from the initial ready frontier:

For each scheduling step:

1. identify completed prerequisites;
2. identify ready issues;
3. calculate safe parallel set;
4. explain rejected parallel combinations;
5. simulate completion;
6. recompute frontier.

Continue until every issue is scheduled.

Look specifically for:

- deadlocks;
- dependency cycles;
- massive bottleneck tickets;
- unsafe parallelism;
- integration points occurring too late;
- UI foundations arriving too late;
- test infrastructure arriving too late;
- recovery being impossible to test until the end;
- Approval Broker architecture arriving too late;
- tickets depending on APIs that do not yet exist.

Do not change code.

If the graph needs changes, modify the issues/dependencies and rerun the simulation.

Stop only when the graph can progress from start to finish without an unexplained scheduling dead end.
Update CURRENT_STATE.md to mark the planning graph ready for implementation.
```

------------------------------------------------------------------------

# 11. Prompt 9 --- Bootstrap the repository

``` text
Begin implementation of Vela.

This is the repository bootstrap ticket only.

Read AGENTS.md and the complete assigned GitHub issue.

Follow the architecture exactly.

Initialize only the infrastructure required by this ticket.

Requirements:

- use the frozen directory architecture;
- pin/document relevant tool versions;
- create reproducible development commands;
- establish formatting/linting/typechecking;
- establish test infrastructure;
- establish Tauri + React/TypeScript + Rust workspace boundaries as specified;
- do not prematurely implement later features;
- keep the application runnable at the end of the ticket.

Follow the repository's implementation protocol.

Use TDD where appropriate.

Run all required tests.

Create a durable checkpoint before authoritative review.

Run /code-review against the recorded fixed point.

Fix actionable findings.

Rerun affected tests.

Repeat review until the Engineering exit policy passes or the configured loop limit is reached.

Commit with the ticket reference.

Push according to repository Git policy.

Return the structured completion report required by AGENTS.md.
Update durable project state where the specifications require it.
```

------------------------------------------------------------------------

# 12. Prompt 10 --- Master autonomous implementation prompt

Use after foundations, skills, and issue graph are ready.

``` text
Implement the Vela specification using the complete GitHub issue graph.

Before execution:

1. Read AGENTS.md.
2. Verify the repository specifications and ADRs.
3. Verify the issue graph.
4. Verify Matt Pocock engineering skills are installed and configured.
5. If missing, install/setup them according to their current upstream instructions and verify them before continuing.
6. Run project preflight.
7. Verify the integration branch strategy.
8. Verify worktree support.
9. Verify required test commands.
10. Verify the current Antigravity/agent execution capabilities.
11. Verify Vela's configured autonomy/approval path.
12. Do not assume native Always Proceed eliminates prompts.
13. Ensure the Approval Broker/fallback requirements remain part of implementation.

Then execute the issue graph using the implement-spec workflow where compatible with our repository policy.

NON-NEGOTIABLE EXECUTION RULES:

- one issue per fresh implementation context;
- full issue identifier/reference;
- parallel workers always use separate worktrees and branches;
- only dependency-safe AND write-safe AND contract-safe tickets run concurrently;
- when uncertain, serialize;
- use tracer-bullet execution;
- run /implement for each ticket;
- run focused tests during development;
- run required ticket validation;
- create a durable checkpoint before authoritative fixed-point review;
- run /code-review;
- fix blocking/actionable findings;
- rerun affected tests;
- checkpoint;
- review again;
- continue until Engineering review policy passes or loop limit is reached;
- never call review success while required tests fail;
- never weaken tests to obtain green status;
- push successful worker branches;
- merge through the serialized integration lane;
- run integration validation after merges;
- do not advance the dependency frontier while integration health is unknown;
- recompute the frontier after every successful merge;
- use a new context for newly started issues;
- persist meaningful discoveries in repository documentation rather than relying on conversation history;
- stop and mark NEEDS_HUMAN for genuine ambiguity;
- do not silently redesign requirements.

If a worker fails:
preserve its worktree, branch, commits, logs, and state.

If an agent/session crashes:
reconcile before retrying.

If network/GitHub fails:
preserve local work and retry idempotently.

If capacity is exhausted:
pause/migrate only using supported authorized execution profiles. Do not automate account switching in a way that bypasses provider limits or authentication safeguards.

Do not automate passwords, 2FA, CAPTCHA, or quota circumvention.

Continue until:
- all in-scope issues are complete,
- integration tests pass,
- final review passes,
- repository state is clean and recoverable.

Provide progress through structured execution state rather than asking me for ordinary approvals.
```

------------------------------------------------------------------------

# 13. Prompt 11 --- Individual ticket worker

Use for manually launched `/implement` sessions or as the worker
contract inside orchestration.

``` text
You are implementing exactly ONE Vela GitHub issue.

Read AGENTS.md first.

Then read:
- the complete assigned issue;
- every specification document referenced by it;
- relevant ADRs;
- relevant existing code/tests;
- integration changes that occurred since this ticket was authored.

Do not work on another issue.

Do not redesign settled architecture.

Verify:
- your assigned worktree;
- your branch;
- your recorded integration base SHA.

Then run:

/implement <FULL GITHUB ISSUE URL>

Follow the repository implementation protocol.

After implementation:

1. run focused tests;
2. run required ticket gates;
3. checkpoint the complete implementation in Git;
4. record the fixed-point/base SHA;
5. run:

/code-review <EXPLICIT FIXED POINT>

6. classify findings using Vela's Engineering review policy;
7. fix all blocking/actionable findings;
8. rerun affected tests;
9. checkpoint fixes;
10. run code review again;
11. repeat until:
   - zero blocking findings,
   - zero unresolved high findings,
   - zero unresolved medium correctness/spec findings,
   - required tests/typecheck/build are green;

or until the configured review-loop limit is reached.

Do not endlessly fix subjective stylistic suggestions.

If review oscillates or reaches the loop limit, report REVIEW_STALLED.

When successful:
- push your branch;
- do NOT independently merge around Vela's integration lane;
- return the structured completion report from AGENTS.md.
```

------------------------------------------------------------------------

# 14. Prompt 12 --- Dedicated integration merger

``` text
You are the Vela integration merger.

You do not implement product features.

Inputs:
- successful worker branch;
- worker completion report;
- current integration branch;
- recorded worker base;
- test/review evidence.

Before merge:

1. verify worker HEAD exists;
2. verify worker review passed;
3. verify required ticket gates passed;
4. fetch/update refs if policy allows;
5. inspect changes to integration since worker base;
6. detect semantic/file/contract conflicts;
7. update/rebase/merge according to repository policy;
8. rerun conflict-sensitive tests if integration changed.

If conflict resolution is purely mechanical and unambiguous, resolve it.

If resolution requires choosing product behavior or reconciling incompatible architecture, STOP with NEEDS_HUMAN.

Merge only through the serialized integration lane.

After merge:

- run integration validation;
- record integration-before SHA;
- record integration-after SHA;
- push integration according to policy;
- report result to scheduler.

If integration validation fails:

DO NOT continue the dependency frontier.

Determine whether safe automatic revert/repair is permitted by repository policy; otherwise escalate with complete evidence.
```

------------------------------------------------------------------------

# 15. Prompt 13 --- UI implementation supplement

Append this to any substantial UI ticket.

``` text
This is an Vela UI ticket.

Before touching UI code, reread ALL of:

docs/ui/UI_UX_SPEC.md
docs/ui/DESIGN_SYSTEM.md
docs/ui/MOTION_AND_3D.md
docs/ui/REFERENCE_BRIEF.md
docs/ui/INTERACTION_SPEC.md
docs/ui/ACCESSIBILITY.md
docs/ui/PERFORMANCE_BUDGET.md
docs/ui/SCREEN_INVENTORY.md
docs/ui/UI_ACCEPTANCE_CHECKLIST.md

The supplied Google Stitch visual reference is the primary mood direction.

ABSOLUTELY DO NOT turn Vela into a conventional dark SaaS/admin dashboard.

The canvas is the hero.

Required visual language:

- near-black spatial environment;
- tiny ordered dot lattice;
- cursor-reactive displacement;
- dots subtly brighten/scale near pointer;
- smooth spring return;
- huge diffused violet/electric-blue/cyan gradient fields;
- large areas of true dark negative space;
- clean soft-white typography;
- restrained glass;
- glass only for contextual floating information;
- subtle depth;
- physical camera motion;
- dependency graph as a constellation;
- fine luminous dependency edges;
- running nodes with restrained breathing energy;
- event-driven ripples;
- merge flow along edges;
- issue-focus camera transitions;
- drag/pan/zoom;
- premium rather than gamer/cyberpunk styling.

DO NOT:

- create a permanent dashboard sidebar unless a specification explicitly requires one;
- fill the screen with cards;
- put glass on every element;
- use thick neon borders;
- use random RGB colors;
- create meaningless animations;
- use excessive bloom;
- hide information behind purely decorative 3D;
- sacrifice accessibility;
- keep the GPU busy when the app is idle/minimized.

DOM should own text-heavy crisp UI.
WebGL/R3F should own the spatial/environmental effects where justified.

Every animation must correspond to:
interaction,
focus,
topology,
or actual orchestration state.

Implement reduced-motion behavior simultaneously, not later.

Implement Full/Balanced/Efficiency graphics behavior where relevant.

Before marking complete, run the complete UI acceptance checklist.

Provide visual evidence/screenshots for the implemented state.
```

------------------------------------------------------------------------

# 16. Prompt 14 --- Independent UI fidelity review

Prefer a different model/context from the implementer.

``` text
Act as an independent Vela visual-design reviewer.

Do NOT modify code initially.

Read the complete UI specification and inspect the running application.

Evaluate:

- Stitch-inspired visual identity;
- black-space dominance;
- dot-grid quality;
- cursor interaction;
- ambient gradients;
- glass restraint;
- typography;
- spatial hierarchy;
- graph aesthetics;
- graph readability;
- node states;
- dependency edges;
- camera movement;
- issue focus;
- run controls;
- animation semantics;
- visual calm;
- accessibility;
- reduced motion;
- performance;
- minimized/unfocused behavior;
- high-DPI behavior.

Explicitly detect generic-AI UI failure modes:

- too many cards;
- dashboard appearance;
- unnecessary sidebar;
- excessive gradients;
- purple everywhere;
- thick glowing borders;
- fake futuristic decoration;
- random particles;
- cramped spacing;
- excessive text;
- inconsistent radius;
- inconsistent blur;
- animation without meaning.

Classify findings:

BLOCKING FIDELITY
IMPORTANT
POLISH
SUBJECTIVE

Only BLOCKING FIDELITY and IMPORTANT findings should automatically trigger fixes.

Then fix those findings, run relevant tests, and re-review.

Do not endlessly optimize subjective polish.
```

------------------------------------------------------------------------

# 17. Prompt 15 --- Real Antigravity Approval Broker validation

Run on the actual Windows/Antigravity environment using harmless
project-scoped operations.

``` text
Validate Vela's Approval Broker against the ACTUAL installed Antigravity environment on this Windows machine.

This is a controlled compatibility test.

Read:

docs/orchestration/APPROVAL_BROKER.md
docs/security/SECURITY_AND_PERMISSIONS.md
ADR-007
relevant acceptance tests.

Test in increasing risk order using harmless project-scoped operations.

Verify:

1. whether native permissive settings actually avoid the prompt;
2. if a prompt remains, whether Vela detects it;
3. whether it correlates the prompt with the correct Antigravity process/window/session;
4. whether the operation is normalized correctly;
5. whether policy classifies it correctly;
6. whether Windows UI Automation can identify the correct approval control;
7. whether one approval is delivered;
8. whether Vela verifies worker progress afterward;
9. whether window movement/resizing breaks detection;
10. whether DPI scaling breaks detection;
11. whether another application's Approve button is ignored;
12. whether an ambiguous approval surface fails closed;
13. whether repeated prompts trigger APPROVAL_STALLED;
14. whether destructive/sensitive requests remain human-gated.

Do not test with destructive commands.

Do not test credential/2FA automation.

Do not loosen the policy merely to make the test pass.

If the installed Antigravity version differs from our compatibility assumptions, update the adapter and dated research documentation.

Produce a compatibility report with the Antigravity version/environment tested.
```

------------------------------------------------------------------------

# 18. Prompt 16 --- Crash/recovery torture test

``` text
Perform Vela resilience testing.

Use fixture/test repositories, not valuable production repositories.

Systematically interrupt Vela at every meaningful worker transition:

PROVISIONING
CONTEXT_LOADING
IMPLEMENTING
FOCUSED_VALIDATION
CHECKPOINTING
WAITING_APPROVAL
REVIEWING
FIXING
READY_TO_MERGE
MERGING
INTEGRATION_VALIDATION

For each:

1. create known state;
2. terminate the relevant Vela/agent process abruptly;
3. restart Vela;
4. run reconciliation;
5. verify Git refs;
6. verify worktrees;
7. verify SQLite state;
8. verify event journal;
9. verify child processes;
10. verify remote state where relevant;
11. resume;
12. confirm no duplicated destructive/non-idempotent action.

Also test:

- laptop sleep/wake;
- network disconnect;
- GitHub outage simulation;
- agent crash;
- orphan command;
- branch modified externally;
- integration branch moved externally;
- missing worktree;
- dirty worktree;
- push succeeded but local completion record did not persist;
- merge succeeded but process died before state persistence;
- approval delivered immediately before crash.

Every failure discovered must receive a regression test.

Do not weaken reconciliation rules to make tests pass.
```

------------------------------------------------------------------------

# 19. Prompt 17 --- Security review

Use a strong model in a fresh context.

``` text
Perform a security review of Vela.

Assume repositories, issue text, Markdown, command output, package scripts, and agent-generated text can be malicious or compromised.

Review:

- prompt injection;
- malicious AGENTS-like files;
- command injection;
- shell quoting;
- path traversal;
- symlink escapes;
- arbitrary file access;
- process spawning;
- package install scripts;
- Git hooks;
- credential leakage;
- environment variables;
- logs;
- diagnostic bundles;
- SQLite;
- GitHub tokens;
- provider credentials;
- Approval Broker;
- Windows UI Automation;
- visual auto-approval;
- wrong-window attacks;
- malicious fake approval dialogs;
- remote URLs;
- destructive Git operations;
- branch protection;
- update mechanism.

Create a threat model.

For each finding:
severity,
attack path,
impact,
existing mitigation,
recommended fix,
test.

Fix real vulnerabilities.

Do not report speculative style concerns as vulnerabilities.

Rerun security-relevant tests afterward.
```

------------------------------------------------------------------------

# 20. Prompt 18 --- Performance pass

``` text
Perform an evidence-based Vela performance pass.

Do not optimize from intuition alone.

Measure:

- cold startup;
- warm startup;
- time to interactive;
- project load;
- SQLite queries;
- scheduler overhead;
- event burst handling;
- graph with 50 nodes;
- 100 nodes;
- 500 nodes;
- pan/zoom frame stability;
- dot-field GPU cost;
- ambient gradient GPU cost;
- issue-focus transition;
- memory usage;
- GPU memory;
- idle CPU;
- idle GPU;
- unfocused resource use;
- minimized resource use;
- Full graphics;
- Balanced graphics;
- Efficiency graphics.

Profile first.

Only optimize measured bottlenecks.

Particular requirements:

- React must not rerender per animation frame unnecessarily;
- Three.js resources must be disposed;
- dots/nodes should use GPU-friendly approaches/instancing where appropriate;
- expensive rendering must throttle or suspend when minimized;
- orchestration must continue while visual rendering sleeps;
- text/log lists must virtualize where needed.

After changes, rerun measurements and provide before/after evidence.
```

------------------------------------------------------------------------

# 21. Prompt 19 --- Final requirements audit

``` text
Do NOT assume Vela is finished because all GitHub issues are closed.

Perform a complete final requirements audit.

Read every current specification and every ADR.

For every functional requirement FR-001 onward:

identify:
- implementation;
- source files;
- tests;
- evidence;
- current status.

Then audit every non-functional requirement.

Check all acceptance tests.

Check issue closure against actual implementation.

Check for TODO/FIXME/stub/mock behavior accidentally left in production paths.

Check for skipped tests.

Check for disabled lint rules.

Check for unhandled states.

Check for documentation drift.

Check whether UI implementation actually matches UI specs.

Check whether Approval Broker works in the real environment.

Check whether crash recovery was actually tested.

Check whether installer works on a clean Windows environment.

Produce a REQUIREMENTS TRACEABILITY REPORT.

Anything without evidence is NOT VERIFIED.

Do not modify requirements to match the implementation.
```

------------------------------------------------------------------------

# 22. Prompt 20 --- Final whole-codebase review

``` text
Perform the final Vela code review against the fixed point representing the beginning of implementation.

Review the entire integrated product.

Run independent Standards and Spec reviews.

Focus particularly on:

- correctness;
- race conditions;
- scheduler invariants;
- Git safety;
- worktree isolation;
- crash consistency;
- idempotency;
- approval safety;
- policy bypass;
- security;
- event-journal correctness;
- process cancellation;
- resource cleanup;
- UI state consistency;
- accessibility;
- graphics lifecycle;
- performance;
- specification compliance.

Classify findings using the Engineering review policy.

Fix blocking/actionable findings.

Run the complete test suite.

Checkpoint.

Re-review.

Repeat only until the defined exit policy passes or the review-loop limit is reached.

Do not spend endless cycles on subjective micro-refactoring.
```

------------------------------------------------------------------------

# 23. Prompt 21 --- Release candidate


> **V1 release guard:** Vela cannot be called release-ready without the required Antigravity-on-Windows integration evidence. A fake adapter or a different build-time agent cannot substitute for that release gate.

``` text
Prepare Vela as a release candidate.

Follow docs/release/RELEASE_CHECKLIST.md exactly.

Do not skip items.

Verify:

- clean Windows installation;
- application launches without development tooling;
- no localhost/manual dev server required;
- repository/project import;
- preflight;
- issue analysis;
- safe parallel execution;
- fresh sessions;
- worktrees;
- implementation;
- tests;
- review loop;
- Git integration;
- push;
- merge;
- integration validation;
- recovery;
- Stop All;
- Approval Broker;
- real Antigravity approval compatibility;
- human intervention;
- accessibility;
- reduced motion;
- graphics modes;
- performance;
- diagnostics;
- secret redaction;
- installer;
- upgrades/migrations.

Create a release validation report.

For every checklist item include:

PASS
FAIL
NOT APPLICABLE

and evidence.

No FAIL item may be silently waived.

If everything passes, create the release commit/tag according to repository policy.

Do not call the product release-ready unless the checklist actually passes.
```

------------------------------------------------------------------------

# 24. Recommended model/context choreography

The exact vendor is less important than repository discipline. A good
pattern is:

``` text
Strong reasoning context
  Prompt 1 — comprehension
  Prompt 2 — specification audit
  Prompt 3 — specification corrections
  Prompt 4 — external verification
  Prompt 5 — architecture freeze
       ↓
COMMIT + PUSH + CURRENT_STATE
       ↓
Fresh model/context
  HANDOFF + Prompt 6 — issue generation
       ↓
Different model/context
  HANDOFF + Prompt 7 — independent issue audit
  Prompt 8 — scheduler simulation
       ↓
Implementation begins
```

After planning, stop carrying the giant planning conversation.
Implementation should be repository-driven:

``` text
                     CANONICAL REPOSITORY
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
          Fresh Chat       Fresh Chat       Fresh Chat
          Issue #31        Issue #34        Issue #38
              │               │               │
          /implement       /implement       /implement
              │               │               │
            tests           tests           tests
              │               │               │
         checkpoint      checkpoint      checkpoint
              │               │               │
        /code-review     /code-review     /code-review
              │               │               │
          fix loop         fix loop         fix loop
              └───────────────┼───────────────┘
                              ▼
                        MERGE AGENT
                              │
                        integration
                              │
                         next frontier
```

### Deliberately mix reviewers and implementers

Where practical, do not have the same context both create and
independently validate its own most important decisions. Useful
separations include:

-   one model architects, another audits tickets;
-   one agent implements a risky subsystem, another security-reviews it;
-   one agent implements UI, another performs visual fidelity review;
-   workers implement tickets, a dedicated merger owns integration.

This is not because any one vendor is inherently the correct one. It
reduces correlated mistakes.

------------------------------------------------------------------------

# 25. Context-loading policy

Do **not** force every ticket worker to read all specification files.
That wastes context and reduces focus.

### Planning/architecture agent

Read the whole pack.

### Ticket worker

Read: 1. `AGENTS.md`; 2. assigned issue; 3. documents referenced by the
issue; 4. relevant ADRs; 5. affected code/tests; 6. integration changes
since ticket creation/base.

### Merger

Read integration policy, worker report/evidence, relevant changed
contracts, and current integration state.

### Specialist auditor

Read the full specification subset for its specialty plus actual
implementation/evidence.

------------------------------------------------------------------------

# 26. Durable-state rule

Whenever an agent discovers something that future work depends on, ask:

``` text
Will another agent need this after this conversation disappears?
```

If yes, persist it as one or more of:

-   specification update;
-   ADR;
-   GitHub issue update;
-   test;
-   source-code contract/type;
-   migration;
-   fixture;
-   research note;
-   `CURRENT_STATE.md` update;
-   commit message/history.

Never use chat history as the only storage location for architecture,
constraints, known bugs, integration assumptions, or pending work.

------------------------------------------------------------------------

# 27. Phase-boundary checklist

Before switching models or starting the next major phase, verify:

-   [ ] Relevant work is persisted in files/issues/code.
-   [ ] `CURRENT_STATE.md` is current.
-   [ ] Git status is understood.
-   [ ] Intended changes are committed.
-   [ ] Commits are pushed when policy permits.
-   [ ] No critical decision exists only in chat.
-   [ ] New ADRs are indexed.
-   [ ] Issue dependencies reflect current architecture.
-   [ ] Tests/evidence for completed work are preserved.
-   [ ] Next phase is explicitly identified.
-   [ ] Any `NEEDS_HUMAN` decision is written down rather than guessed.

------------------------------------------------------------------------

# 28. If an agent starts drifting

Use this correction prompt without discarding valid work:

``` text
Stop expanding scope.

Re-anchor to the Vela repository.

Read AGENTS.md, CURRENT_STATE.md, the assigned issue, its referenced specifications, and relevant ADRs again.

Identify exactly where your current plan or implementation diverges from the canonical requirements.

Do not rewrite valid completed work merely to match your preferred architecture.
Do not invent requirements.
Do not remove requirements for convenience.
Do not continue unrelated refactoring.

Return:
1. the divergence;
2. affected files;
3. the canonical requirement/ADR;
4. the smallest corrective action;
5. tests required to prove correction.

Then perform only that corrective action if it is unambiguous. Otherwise mark NEEDS_HUMAN.
```

------------------------------------------------------------------------

# 29. If an agent loses context midway through a ticket

``` text
Reconstruct this Vela ticket from durable state before continuing.

Read AGENTS.md, CURRENT_STATE.md, the full assigned GitHub issue, referenced specifications/ADRs, current branch history, git status/diff, existing tests, and commits made for this ticket.

Determine:
- ticket objective;
- completed acceptance criteria;
- incomplete acceptance criteria;
- current implementation state;
- tests already run and their evidence if persisted;
- current review fixed point if one exists;
- unresolved review findings;
- whether integration has changed since the ticket base.

Do not restart the ticket from scratch unless the existing work is invalid.
Do not assume anything from lost chat history.
Continue from the repository state.
```

------------------------------------------------------------------------

# 30. Definition of a successful Vela build process

The process succeeds when the application is not merely "implemented,"
but when:

-   the current repository specifications are internally consistent;
-   volatile integrations have been verified or capability-detected;
-   architecture decisions are durable;
-   every requirement maps to implementation and evidence;
-   issues form a valid dependency graph;
-   parallelism is conservative and isolated;
-   each worker can operate from a fresh context;
-   implementation and review use durable Git fixed points;
-   integration is serialized and validated;
-   the UI matches the intended Stitch-inspired spatial aesthetic rather
    than generic dashboard conventions;
-   approval handling works in the user's actual Antigravity environment
    while failing closed for ambiguous/sensitive operations;
-   crashes/restarts/network failures are recoverable;
-   security and performance have independent evidence-based reviews;
-   the packaged Windows application runs without the user manually
    hosting a localhost development server;
-   the release checklist passes with evidence;
-   a different AI agent can enter the repository cold and accurately
    continue the work.

The guiding principle remains:

> **Agent conversations are disposable. Repository state is durable.**
