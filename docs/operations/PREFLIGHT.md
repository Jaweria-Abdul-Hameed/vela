# Preflight Specification

Checks are categorized PASS/WARN/BLOCK. The severity of each check is defined in the table at the
end of this document. A result containing any `BLOCK` cannot advance the run
(`ORCHESTRATION_ENGINE.md` section 1.1).

## Repository

-   path exists,
-   `.git`/worktree recognized,
-   default branch resolvable,
-   remote metadata,
-   working tree state,
-   unresolved merge/rebase,
-   disk space (estimated need for planned concurrent worktrees),
-   path length concerns on Windows,
-   cloud-synced location (see Windows checks),
-   local-versus-remote default-branch divergence.

## Trust

-   repository trust state (`UNTRUSTED` by default; ADR-013). An untrusted repository may be
    analyzed as data only; Build requires `TRUSTED`.

## Toolchain

Infer from files: - package manager, - runtime versions, - lockfiles, -
test/lint/typecheck/build scripts.

Do not run arbitrary package scripts merely to discover them. Inference is by reading manifests as
data; no repository-controlled code is executed during preflight.

## Agent

-   adapter installed/reachable,
-   capability discovery against the Antigravity Required Capability Contract
    (`ADAPTERS.md`, CAP-01..CAP-11),
-   permission mode sufficient for requested autonomy (Effective Posture Check below),
-   skills present,
-   issue tracker configured.

### Antigravity installation and authentication (ADR-016)

-   whether the official `agy` binary is installed, its version, and whether it self-updates in the background
    (a version change during a run is a recoverable divergence; see `RECOVERY.md`);
-   whether the Desktop app is installed and its version, when the secondary surface is in scope;
-   authentication state is learned **only through Antigravity's own supported interfaces**. Vela never reads,
    copies, or persists Antigravity credentials or tokens, including the Windows Credential Manager entries.
    When authentication is missing, preflight reports `AUTH_REQUIRED` and asks the user to authenticate through
    Antigravity's supported flow.

## Git/GitHub

-   git available,
-   author identity configured,
-   remote accessible when required,
-   auth available,
-   push permissions optionally tested non-destructively,
-   `core.autocrlf` and `.gitattributes` state reported (to avoid whole-file line-ending churn).

## Docs

Required/expected: - AGENTS.md, - product/spec context, - architecture
where project complexity requires it, - issues/tracker. Project-supplied `AGENTS.md` and
documentation are read as untrusted input (ADR-013).

## Windows checks

-   repository path under a known cloud-sync provider (OneDrive, Dropbox, Google Drive, iCloud):
    `WARN` for the repository, `BLOCK` for the planned worktree root;
-   computed maximum path length for the planned worktree layout versus the Windows limit; Vela
    applies deterministic branch-slug truncation with a hash suffix so ref and path names stay
    bounded and case-insensitively unique;
-   long-path configuration state;
-   unattended-session conditions (ADR-012): lock-screen policy that will interrupt UI-automation
    delivery, sleep/lid actions, and whether Vela may hold the system awake. Vela reports these and
    never changes the user's lock or power policy persistently.

## Safe bootstrap actions

Preflight never modifies the primary checkout's working tree, index, or branch. The only actions
it may take without further user confirmation are: creating Vela-managed directories outside the
repository, writing Vela application data, and read-only network fetches when policy permits. Any
other bootstrap action (including installing skills, see `MATT_POCKOCK_SKILLS.md`) requires an
explicit user-approved step, is recorded, and if it changes repository-tracked files it does so on
a separate Vela-created branch and commit, never on the user's current branch or dirty tree.

## Result

Preflight produces remediation actions and never leaves half-applied
setup without recording it.

# Effective Approval Test

Preflight must not merely read the configured Antigravity permission
setting. It must determine the available approval mechanisms and, where
a safe non-destructive probe is possible, validate effective unattended
behavior.

Report: - native permission capability, - **Effective Posture Check result**
(`MEETS`, `DOES_NOT_MEET`, `UNKNOWN`; ADR-009), - whether routine prompts still
appear, - whether guarded UI automation has been enabled (ADR-010) and is
capable (UI Automation/accessibility; visual fallback availability), - policy
broker status, - interactive-session conditions (ADR-012), - expected
unattended readiness.

If Vela cannot safely identify/deliver routine approvals, or the posture is not
`MEETS`, Autonomous readiness is `BLOCK`; Supervised readiness is `WARN` with the
acknowledgement defined in `AUTONOMY_MODES.md`.

# Default Runtime Preflight

V1 project/run preflight must treat Antigravity capability as the default production runtime check. Generic adapter availability is not enough.

If Antigravity required capabilities are unavailable, preflight must report the concrete degraded/blocking state rather than silently selecting an unrelated provider.

# Severity Table

| Check | `BLOCK` | `WARN` |
|---|---|---|
| Repository path / `.git` | missing or not a repository | |
| Default branch | not resolvable | |
| Merge/rebase in progress | present | |
| Working tree | | dirty primary checkout (worktrees still usable) |
| Local vs remote default branch | | diverged: requires explicit acknowledgement of the recorded base SHA |
| Disk space | insufficient for planned worktrees | low margin |
| Trust | `UNTRUSTED` blocks Build (analysis allowed) | |
| Git available / author identity | missing | |
| Remote accessible / auth | | missing when promotion mode is `pr` (run can proceed to local-ready) |
| Antigravity Required capability (CAP-01..07) | missing, with the outcome in the "If absent" column of `ADAPTERS.md` (CAP-07 absent only reduces concurrency to one) | |
| CAP-08, CAP-09, CAP-10 | | missing (reduced capability reported) |
| Posture (CAP-11, ADR-009) | `DOES_NOT_MEET` / `UNKNOWN` for Autonomous | same for Supervised, with acknowledgement |
| UI automation not enabled (ADR-010) | | reduced unattended readiness |
| Skills missing | after the offered install fails or is declined | before the offer |
| Issue tracker not configured | | |
| `AGENTS.md` / spec context missing | | |
| Worktree root under cloud sync | blocked | repository under cloud sync |
| Path length exceeds limit | | after truncation still exceeds |
| Unattended-session conditions (ADR-012) | | any condition that will interrupt unattended operation |
