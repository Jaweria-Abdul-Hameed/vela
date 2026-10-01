# Preflight Specification

Checks are categorized PASS/WARN/BLOCK.

## Repository

-   path exists,
-   `.git`/worktree recognized,
-   default branch resolvable,
-   remote metadata,
-   working tree state,
-   unresolved merge/rebase,
-   disk space,
-   path length concerns on Windows.

## Toolchain

Infer from files: - package manager, - runtime versions, - lockfiles, -
test/lint/typecheck/build scripts.

Do not run arbitrary package scripts merely to discover them.

## Agent

-   adapter installed/reachable,
-   capability discovery,
-   permission mode sufficient for requested autonomy,
-   skills present,
-   issue tracker configured.

## Git/GitHub

-   git available,
-   author identity configured,
-   remote accessible when required,
-   auth available,
-   push permissions optionally tested non-destructively.

## Docs

Required/expected: - AGENTS.md, - product/spec context, - architecture
where project complexity requires it, - issues/tracker.

## Result

Preflight produces remediation actions and never leaves half-applied
setup without recording it.

# Effective Approval Test

Preflight must not merely read the configured Antigravity permission
setting. It must determine the available approval mechanisms and, where
a safe non-destructive probe is possible, validate effective unattended
behavior.

Report: - native permission capability, - whether routine prompts still
appear, - UI Automation/accessibility capability, - visual fallback
availability, - policy broker status, - expected unattended readiness.

If Vela cannot safely identify/deliver routine approvals, autonomous
readiness is WARN/BLOCK according to requested mode.

# Default Runtime Preflight

V1 project/run preflight must treat Antigravity capability as the default production runtime check. Generic adapter availability is not enough.

If Antigravity required capabilities are unavailable, preflight must report the concrete degraded/blocking state rather than silently selecting an unrelated provider.
