# Context

## User workflow this product must support

The user prepares detailed Markdown context and/or GitHub issues before
autonomous execution. Work is generally organized as tracer-bullet
tickets. For each ticket the desired rhythm is:

1.  fresh conversation/session,
2.  `/implement`,
3.  tests written/run during implementation,
4.  durable commit checkpoint,
5.  `/code-review`,
6.  fix actionable findings,
7.  rerun affected tests,
8.  review again until the configured review exit policy is satisfied,
9.  push,
10. merge through the defined integration workflow,
11. open a fresh session for the next issue.

Independent tickets may execute simultaneously only after dependency and
conflict analysis establishes that concurrency is safe. Dependent or
semantically overlapping work remains sequential.

The user wants unattended execution: they should not need to remain in
front of the laptop approving ordinary commands. This must be achieved
with supported permission settings/policies where possible, not an
indiscriminate clicker.

The finished product must be a seamless installable desktop application,
not something the user manually serves on localhost.

## Visual intent

The strongest visual reference is the Google Stitch landing experience
supplied during product discovery: - near-black canvas, - sparse
luminous dot grid, - cursor-reactive dots, - enormous soft
blue/violet/cyan gradient fields, - restrained glass surfaces, -
generous negative space, - large clean typography, - subtle depth and
parallax, - motion that feels physical, - spatial rather than
dashboard-like information architecture.

The application should make agent execution and dependency flow feel
like a living constellation.

# Canonical Runtime Scope

The canonical runtime scope for Vela v1 is **Google Antigravity on Windows**.

Interpret all provider-independent architecture through this rule:

> **Antigravity-first, provider-extensible.**

`AgentAdapter` is an architectural seam. `AntigravityAdapter` is the required v1 implementation. Other runtime adapters are future extension points unless explicitly brought into scope by an approved specification change.

Do not infer runtime support merely because Claude, Gemini, Codex, or another agent is used during Vela's own development.
