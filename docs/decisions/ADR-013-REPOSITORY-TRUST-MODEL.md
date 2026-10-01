# ADR-013: Repository Trust Model

## Status

Accepted (2026-10-02).

## Context

Prompt 2 finding SA-15: auto-allow covered "project test and build commands" while the threat
model lists malicious test scripts, install scripts, hooks, and untrusted issue text. Human
decision (2026-10-02): newly imported repositories are untrusted by default; trust is explicit;
Vela policy still applies to trusted repositories; repository text is untrusted input.

## Decision

1. **Default UNTRUSTED.** Every newly imported repository is `UNTRUSTED`.
2. **UNTRUSTED capabilities.** Vela may inspect it as data only: read files, parse Markdown and
   manifests, run Git read operations hardened so that repository-controlled configuration cannot
   execute code (hooks path, fsmonitor, external diff/textconv/filter drivers and similar). Vela
   does not run repository scripts, package scripts, build or test commands, installs, Git hooks,
   or provisioning commands, and does not start workers, in an untrusted repository. Preflight and
   graph analysis that require only data inspection are allowed. **Build is not permitted.**
3. **Establishing trust.** Only an explicit user action in Vela's UI establishes `TRUSTED`. The
   trust sheet lists what becomes executable (package scripts, build/test commands, hooks,
   provisioning commands). Trust is recorded (project, repository identity, timestamp, consent
   text version) and journaled, is revocable, and is part of the run policy snapshot.
4. **Trust does not grant authority over policy.** In a trusted repository Vela policy,
   workspace, and security constraints still apply. Repository text, issue text, and
   project-supplied `AGENTS.md` are untrusted input to Vela: they may inform agents but never
   change Vela policy, allow lists, trust state, or approval classification.
5. **Auto-allow requires a confirmed command profile.** A command is eligible for
   test/lint/typecheck/build auto-allow only when (a) the repository is `TRUSTED`, (b) the exact
   normalized command and working directory are in the project's user-confirmed command profile,
   and (c) the operation is within the assigned worktree. Being named "test" or "build" confers
   nothing. Everything else is `ASK` by default.
6. **Execution-defining files** (package manifests with scripts, build and CI configuration, Git
   hooks, provisioning definitions) modified by an agent are high-risk write surfaces: they are
   listed in the completion report and treated as security-relevant in review.

## Consequences

- Preflight gains a trust check; `PRODUCT_SPEC.md` FR-043 and AT-020 cover it.
- `SECURITY_AND_PERMISSIONS.md` auto-allow examples are qualified by this ADR.
- Provisioning (FR-044) and checkpoint hooks (ADR-015) run only in trusted repositories.
