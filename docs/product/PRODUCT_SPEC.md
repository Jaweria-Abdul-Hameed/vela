# Product Specification

## 1. Problem

Long autonomous coding runs currently demand too much babysitting.
Permission prompts interrupt work; independent issues are not safely
scheduled; agents accumulate stale context; review and test loops are
inconsistent; parallel sessions can collide in one checkout;
quota/capacity failures interrupt execution; and after several agents
have modified a repository it becomes difficult to reconstruct exactly
what happened.

Vela turns this into a durable workflow.

## 2. Primary user journey

### 2.1 First launch

The user launches an installed Windows desktop application. No browser
tab and no manual local web server are required.

### 2.2 Add project

The home canvas offers: - drag/drop repository folder, - file/folder
picker, - recent projects, - optional clone/import flow later.

### 2.3 Preflight

Vela detects: - Git repository and working-tree cleanliness, - remote
origin, - default branch, - current branch, - Git/GitHub authentication
capability, - package managers/runtimes inferred from repository, -
test/lint/typecheck/build commands where discoverable, -
Antigravity/agent integration capability, - skills
installation/configuration, - required Markdown context, - issue tracker
configuration, - available execution profiles, - conflicting
processes/locks.

Preflight never silently mutates the repository beyond explicitly safe
bootstrap actions, which are defined in `PREFLIGHT.md` (it never modifies the
primary checkout's working tree, index, or branch). A newly imported repository
is `UNTRUSTED` and is analyzed as data only until the user trusts it (ADR-013).

### 2.4 Context synthesis

Vela reads the problem statement, specs, architecture, constraints,
ADRs, agent instructions, issues, and relevant code structure. It
produces a normalized project model.

### 2.5 Issue graph

Issues are represented as a DAG. Each node contains: - stable issue
ID, - title, - objective, - acceptance criteria, - explicit blockers, -
inferred semantic dependencies, - expected modules/contracts, -
predicted write set, - migration/schema impact, - test expectations, -
risk class, - execution status.

### 2.6 Parallelization analysis

A ticket is eligible for the ready frontier only if every blocker is
complete and integration is healthy. Two ready tickets may execute
together only if concurrency analysis approves the pair/group.

### 2.7 Build

Vela creates an integration branch and starts workers up to the
configured concurrency cap. Each worker gets a fresh context, branch,
worktree, fixed-point commit, ticket, and context pointers.

### 2.8 Ticket completion

The worker implements, tests, checkpoints, reviews, fixes, retests, and
reports. Vela validates the report independently where practical,
pushes the branch (workers supervised by Vela do not push; ADR-015), merges
through the deterministic serialized merge lane in the dedicated integration
worktree (ADR-011), then runs integration gates.

### 2.9 Advance

Successful merge may unlock new nodes. Vela recomputes the ready
frontier rather than treating issues as a static ordered list.

### 2.10 Finalization

After all tickets: - full integration test suite, - final code review
against the recorded `run_base_sha` fixed point (`run_base_sha...integration_head`),
- fix pass if needed (run states `FINAL_FIXING`), - final
validation, - push, - promotion per ADR-014 (default: validated integration branch
-> PR -> human-controlled merge; local-ready when no remote), - cleanup worktrees
(per `GIT_WORKFLOW.md`), - produce execution summary.

## 3. Functional requirements

### FR-001 Project management

Vela shall register, open, remove-from-Vela, and resume local projects
without deleting source repositories.

### FR-002 Preflight

Vela shall produce a pass/warn/fail preflight report before autonomous
execution.

### FR-003 Documentation discovery

Vela shall discover canonical Markdown files and allow
repository-specific additions.

### FR-004 Skill bootstrap

If required engineering skills are missing, Vela shall offer/perform a
documented installation/setup workflow under user policy and verify
successful availability before execution.

### FR-005 Issue ingestion

Vela shall ingest local issue Markdown and/or a configured issue
tracker.

### FR-006 Dependency graph

Vela shall represent blocking relationships as a DAG and reject/flag
cycles.

### FR-007 Semantic dependency analysis

Vela shall supplement explicit edges with evidence-based inferred
hazards without silently rewriting user-authored dependencies.

### FR-008 Safe parallelization

Vela shall calculate safe execution waves/frontiers using
`PARALLELIZATION.md`.

### FR-009 Fresh sessions

Vela shall create a fresh agent execution context per ticket.

### FR-010 Worktree isolation

Vela shall create isolated worktrees/branches for concurrent tickets.

### FR-011 Implementation invocation

Vela shall invoke the configured implementation workflow with full
ticket reference/context pointers.

### FR-012 Testing

Vela shall execute ticket-specific and repository-required validation
commands.

### FR-013 Review

Vela shall execute a fixed-point code review after a durable diff
exists.

### FR-014 Review fix loop

Vela shall repeat fix → test → review until policy passes or a
termination condition occurs.

### FR-015 Git checkpoints

Vela shall create recoverable commits before risky transitions.

### FR-016 Push

Vela shall push ticket/integration branches according to repository
policy.

### FR-017 Merge

Vela shall merge successful ticket branches into the integration branch
using a controlled process: the deterministic serialized merge lane in a
dedicated integration worktree, merge-based and without routine force-push
(ADR-011).

### FR-018 Integration gate

Vela shall validate integration after merges and stop frontier
advancement if integration becomes unhealthy.

### FR-019 Recovery

Vela shall persist state sufficiently to recover from app crash, agent
crash, laptop restart, network loss, and interrupted review.

### FR-020 Human intervention

Vela shall surface an actionable intervention card instead of hanging
indefinitely.

### FR-021 Kill switch

Vela shall provide a globally reachable Stop All control: always reachable from
the in-app run control, from the tray when background operation is enabled, and
via an optional configurable global shortcut (`ORCHESTRATION_ENGINE.md` section 6).

### FR-022 Resume

Vela shall resume from durable state without repeating completed
external actions blindly.

### FR-023 Execution history

Vela shall retain an auditable event timeline for each run/ticket.

### FR-024 Capacity handling

Vela shall model execution profile availability/cooldown without
embedding password/2FA automation or circumventing provider
restrictions.

### FR-025 UI spatial graph

Vela shall render the project DAG as an interactive spatial
constellation.

### FR-026 Reactive environment

Vela shall implement cursor-reactive dot-field and ambient gradient
behavior described in UI docs.

### FR-027 Efficiency

Vela shall throttle/stop expensive rendering when minimized/unfocused
and expose reduced-motion/efficiency settings.

### FR-028 Notifications

Vela shall notify the user when human action is required, a run
completes, or a terminal failure occurs.

### FR-029 Structured agent IO

Vela shall prefer schema-validated structured results for
worker/analysis outputs.

### FR-030 Policy engine

Vela shall evaluate potentially destructive/sensitive actions before
execution.

## 4. Non-functional requirements

-   **Reliability:** orchestration state must survive process
    termination.
-   **Safety:** no unattended destructive Git/system/credential
    operations outside explicit policy.
-   **Performance:** startup and project switching should feel
    immediate; heavy graph rendering must not block orchestration.
-   **Observability:** every state transition has reason/evidence.
-   **Portability:** architecture should permit future macOS/Linux
    support while Windows remains first-class.
-   **Provider independence:** core scheduler cannot depend on one model
    vendor.
-   **Testability:** orchestration logic must be separable from UI and
    external process adapters.
-   **Accessibility:** keyboard navigation, reduced motion, contrast,
    focus visibility, and non-color status encoding.
-   **Security:** secrets remain in OS/provider credential stores; logs
    redact sensitive values.
-   **Maintainability:** adapters isolate volatile third-party
    integrations.

## 5. Out of scope for initial release

-   Replacing the user's IDE/editor.
-   Building a general cloud CI service.
-   Storing provider passwords.
-   Automating 2FA/passkey entry.
-   Circumventing provider quotas/rate limits.
-   Arbitrary remote machine orchestration.
-   Automatically resolving product ambiguities with irreversible
    consequences.

# Approval Compatibility Requirements

### FR-031 Approval Broker

Vela shall independently classify Antigravity approval requests as
`ALLOW`, `ASK`, or `DENY` using project/run policy.

### FR-032 Approval Watchdog

Vela shall detect workers that appear blocked waiting for approval even
when no clean provider event is emitted.

### FR-033 Guarded UI approval fallback

When a request is already classified `ALLOW` and native permission
mechanisms cannot deliver the decision, Vela shall support a versioned
Antigravity UI-automation adapter to activate the appropriate approval
control. The capability is opt-in: it is used only after the user enables it
during onboarding or in Settings (FR-040, ADR-010).

### FR-034 Approval loop protection

Vela shall fingerprint repeated approval requests and transition to
`APPROVAL_STALLED` after a configurable bounded repetition threshold
rather than approving indefinitely.

### FR-035 Approval audit trail

Vela shall record request classification, reason, delivery mechanism,
outcome, and correlation identifiers without storing sensitive request
contents unnecessarily.

### FR-036 Compatibility health

Preflight shall test/detect the effective approval path for the
installed Antigravity environment and report whether unattended
execution is expected to work.

### Acceptance principle

A worker waiting on an ordinary policy-allowed Antigravity approval
prompt must not require the user to remain physically present, subject to the unattended-session
conditions of ADR-012 (an interactive desktop is required for UI-automation delivery; locked or
disconnected sessions degrade and reconcile) and to the user having enabled guarded UI automation
(ADR-010). A request
that Vela cannot confidently classify must remain blocked for human
review.

# V1 Runtime Product Invariant

### Product invariant: Antigravity-first, provider-extensible

Vela v1 SHALL treat Google Antigravity on Windows as its default, first-class, required runtime execution environment.

The following are v1 release requirements:
- Antigravity capability discovery and preflight;
- Antigravity worker/session lifecycle required by Vela's issue workflow;
- Antigravity execution of the `/implement` and `/code-review`-based workflow where supported by the installed environment/skills;
- unattended routine operation;
- effective approval handling, including Approval Broker and guarded fallback paths;
- recovery/reconciliation around Antigravity processes/sessions;
- Matt Pocock skill discovery/bootstrap required by the documented workflow;
- observable, testable Antigravity integration.

Provider abstraction SHALL remain in the architecture, but v1 SHALL NOT require implementation of Claude, Codex, or other peer runtime adapters unless scope is explicitly amended.

Using Claude, Gemini, Codex, or other agents to develop Vela does not create a shipped runtime-support requirement.

# Requirements Added by the Prompt 3 Specification Fixes

### FR-037 Antigravity capability contract

Vela shall define and enforce the Antigravity Required Capability Contract (`ADAPTERS.md`,
CAP-01..CAP-11): feature-detect each capability at runtime, report a concrete preflight `BLOCK` or
degradation per missing capability, and never substitute another provider or UI automation for a
missing capability.

### FR-038 Policy sovereignty and native permission posture

Vela's `ALLOW`/`ASK`/`DENY` policy shall remain enforceable. Vela shall not depend on or configure
unconditional native auto-execution, shall evaluate the Native Permission Posture in preflight
(`MEETS`/`DOES_NOT_MEET`/`UNKNOWN`), and shall block Autonomous mode unless it is `MEETS`
(ADR-009).

### FR-039 Approval evidence binding

Vela shall bind an approval prompt to a normalized operation only under the Evidence-Binding Rules
(ADR-009), default to `ASK` on weak or incomplete evidence, and treat post-execution mismatches as
policy violations.

### FR-040 Guarded UI automation consent

Guarded UI automation (UIA and visual tiers) shall be disabled until the user explicitly enables it
at onboarding or in Settings; the consent shall persist, be revocable, and be journaled (ADR-010).

### FR-041 Unattended session management

During an active autonomous run Vela may hold the system awake when required, supports display-off,
does not bypass locked/disconnected/secure desktops, pauses UI-automation-dependent work safely, and
reconciles when an interactive session returns (ADR-012).

### FR-042 Background operation and continuation

Background operation, tray access (reopen, status, Stop All), login auto-start, and reboot
continuation shall be opt-in and governed by ADR-012; after reboot Vela shall reconcile before
resuming.

### FR-043 Repository trust

Newly imported repositories shall be `UNTRUSTED`; Build shall require explicit user trust; repository,
issue, and `AGENTS.md` text shall be untrusted input to Vela policy (ADR-013).

### FR-044 Worktree provisioning

Vela shall provision each worktree under the provisioning contract (`GIT_WORKFLOW.md`): user-confirmed
commands only, no automatic secret copying, declared cache and resource-key handling, and retention
and cleanup rules.

### FR-045 Run completion and promotion

Run completion shall follow ADR-014: default PR with human-controlled merge, local-ready outcome
without a remote, a recorded `run_base_sha` as final-review fixed point, and no silent merge of the
default branch.

### FR-046 State-store integrity and recovery

Vela shall check its state store at startup, back it up before migration, handle corruption and failed
migration without silently guessing, and never auto-resume from a reconstructed inventory
(`RECOVERY.md`, `PERSISTENCE.md`).

### FR-047 Installation and updates

Vela shall install on a clean Windows environment without development tooling, handle its web-view
runtime dependency, ship signed installers and updates, back up its state before an upgrade, roll back
a failed migration, and never apply an update during an active run (`RECOVERY.md`).

### FR-048 Renderer hardening

The renderer shall display untrusted content inertly under a strict Content Security Policy and shall
reach the core only through an allowlisted, least-privilege, validated IPC surface
(`SYSTEM_ARCHITECTURE.md` section 5).

## V1 runtime requirement identifiers

The v1 release requirements in "V1 Runtime Product Invariant" map to requirement IDs for traceability:

| V1 requirement | Requirement IDs |
|---|---|
| Antigravity capability discovery and preflight | FR-002, FR-036, FR-037, FR-038 |
| Antigravity worker/session lifecycle | FR-009, FR-010, FR-011, FR-037 |
| `/implement` and `/code-review` workflow | FR-011, FR-013, FR-014 |
| Unattended routine operation | FR-031..FR-036, FR-038..FR-042 |
| Approval handling (Broker, fallback) | FR-031..FR-036, FR-039, FR-040 |
| Recovery/reconciliation for Antigravity | FR-019, FR-022, FR-037, FR-046 |
| Skill discovery/bootstrap | FR-004 |
| Observable, testable integration | FR-023, FR-029, plus `REQUIREMENTS_TRACEABILITY.md` |
