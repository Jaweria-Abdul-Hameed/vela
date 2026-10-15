# Changelog

No release has been made. This file records what exists on `main` so a first release can start from an accurate list. The product is
**not release-ready** (see `docs/release/RELEASE_VALIDATION_REPORT.md`).

## Unreleased

### Added

- **Orchestration.** Projects, trust with a confirmed command profile, preflight, issue ingestion (local folder and GitHub), a dependency graph with safe-parallelization verdicts, a scheduler with a worker cap, per-ticket worktrees, Vela-owned checkpoint commits, a serial merge lane with integration gates, finalization, and promotion (pull request by default, local-ready without a remote).
- **Antigravity.** A driver for the installed `agy` (one process per turn, never trusting exit codes), isolated per-worker profiles with a fail-closed hook, capability discovery (CAP-01..12), a native-permission posture probe, skill bootstrap, capacity and provider-policy-block handling, conversation recording, and drift and orphan handling for recovery.
- **Approvals.** A policy engine (ALLOW only for the confirmed profile of a trusted repository), an approval broker, a silent-worker watchdog, scoped allow rules, and a guarded Windows UI Automation tier (single-use allow or refusal only, consent checked before every action).
- **Recovery.** An operation log with reconciliation recipes, startup reconciliation, Stop All with safe points, state-store backup, restore and a Git-derived inventory, sleep/wake and push-retry logic, reboot continuation (`ask` by default), and update gating, backup and rollback. A crash matrix kills the process at every declared fault point.
- **Interface.** A near-black canvas with an ambient light field and dot lattice, three graphics modes with documented cost, a graph canvas with an always-mounted accessible tree, run bar, timeline, inspector, intervention sheets, onboarding, command palette, settings, tray and background operation, and a visual-regression suite.
- **Quality.** End-to-end acceptance tests over real repositories, a standing security suite with hostile fixtures, a performance harness, and a first real unattended Antigravity run.

### Fixed along the way

- Worker tasks outliving a cancelled run driver; interrupted checkpoints failing the worker on restart; runs stuck in STARTING or STOPPING after a crash; no startup reconciliation; graph ticket buttons that could not be clicked in a browser; the ambient light and dot field hidden behind an opaque shell; `rustls` advisories RUSTSEC-2026-0346 to 0349.

### Known gaps (not release-ready)

See `docs/release/RELEASE_VALIDATION_REPORT.md` and `docs/testing/TRACEABILITY_REPORT.md`: no production signing, no clean-machine run, no real-Desktop approval runs, a non-reference performance run only, review disabled by decision (ADR-023/024), and several implemented components not yet started by the composition root.
