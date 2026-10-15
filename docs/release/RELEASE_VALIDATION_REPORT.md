# Release validation report (Z06)

Date: 2026-10-15. Candidate: `main` after M31, version 0.0.0 (no release tag has been or will be created from this report).

## Verdict

**NOT RELEASE-READY.** The product runs a real ticket through the real Antigravity path and its automated suites are green, but items that
need production signing material, a clean machine, the Desktop app, a reference-hardware run and an independent review have not happened.
Each is listed below as what it is. Nothing is waived: an item is PASS only with evidence.

| Count                                           | Items |
| ----------------------------------------------- | ----- |
| PASS                                            | 14    |
| FAIL (blocks release)                           | 3     |
| NOT VERIFIED (needs an environment or a person) | 15    |
| NOT APPLICABLE                                  | 1     |

## Production release-signing gate

| Check                                                                                                                       | Result                            | Evidence                                                                                                                                                                                                                                                       |
| --------------------------------------------------------------------------------------------------------------------------- | --------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| H04 (signing material) is closed                                                                                            | **FAIL**                          | H04 is open: no production Authenticode certificate and no production updater key are provisioned (`docs/research/H04_signing_research_2026-10-03.md`). `docs/release/SIGNING.md`, which this gate says must record the production identities, does not exist. |
| (A) Authenticode on the installer and `vela-hook` passes `signtool verify /pa` and chains to the production publisher       | **FAIL (cannot run)**             | Only the development profile exists (`DEV_SIGNING.md`); its throwaway certificate is expected to be untrusted.                                                                                                                                                 |
| (B) updater artifacts verify against the production updater public key; the configuration embeds it; it is not the test key | **FAIL (cannot run)**             | The committed configuration carries no updater key by design (tested); `update::check_release_config` would refuse a test key or an empty key. The production key and its custody and backup are not documented.                                               |
| No development certificate and no test updater key in the candidate                                                         | **PASS (for the committed tree)** | tests assert the base and release configuration hold no updater section and the release workflow never references the test overlay; the development certificate is created on the runner and never committed.                                                  |

## Release checklist (`RELEASE_CHECKLIST.md`)

| Item                                                                           | Result                                   | Evidence                                                                                                                                     |
| ------------------------------------------------------------------------------ | ---------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| all in-scope requirements traced and verified                                  | **FAIL**                                 | `docs/testing/TRACEABILITY_REPORT.md`: many VERIFIED, but several PARTIAL and NOT VERIFIED; FR-013/FR-014 not implemented by decision        |
| unit, integration and e2e suites green                                         | PASS                                     | CI green on every merged branch (Rust on Windows, TypeScript and Playwright, Security)                                                       |
| crash-recovery matrix green                                                    | PASS                                     | `tests/e2e/tests/crash_matrix.rs`, all 25 fault points, `docs/testing/CRASH_MATRIX.md`                                                       |
| security review complete                                                       | **FAIL**                                 | the standing security suite (X01/X04) passes, but no independent whole-codebase security review was done (ADR-023: no review while building) |
| secret scanning clean                                                          | PASS                                     | `scripts/scan-secrets.mjs` over every branch's range, and the Security workflow                                                              |
| installer tested on a clean Windows environment                                | NOT VERIFIED                             | Z07: no guest available (`CLEAN_MACHINE_EVIDENCE.md`)                                                                                        |
| upgrade and migration tested from the previous release                         | PARTIAL, NOT VERIFIED on real installers | store behaviour proven (`tests/integration/tests/upgrade.rs`); no N-1 and N installer pair built                                             |
| graphics Full, Balanced, Efficiency tested                                     | PASS (non-reference)                     | unit tests of the profiles; a Playwright run in each mode (`PERFORMANCE_RESULTS_2026-10-15.md`)                                              |
| reduced motion tested                                                          | PASS                                     | unit tests, the idle-suspension tests, and the `reduced-motion` visual scene                                                                 |
| keyboard accessibility tested                                                  | PASS                                     | `GraphTree`, shortcut help, run bar and inspector keyboard tests; no screen-reader session was run                                           |
| 100/500-node graph performance measured                                        | NOT VERIFIED                             | code paths measured per frame in Node (well under budget); GPU frame rates through WebView2 on the reference machine were not measured       |
| background and minimized resource use measured                                 | NOT VERIFIED                             | idle animation-frame callbacks are 0 in all modes in a non-reference run; minimized and hidden behaviour and 60-second CPU were not measured |
| Antigravity capability matrix revalidated                                      | PASS                                     | real capability probe on `agy` 1.3.5 (A01, 2026-10-15 run)                                                                                   |
| Matt Pocock skills integration contract revalidated                            | PARTIAL                                  | the real `agy` resolved a provided skill (M09); not re-run on 1.3.5                                                                          |
| GitHub outage behavior tested                                                  | PASS (simulated)                         | `push_retrying` with injected network loss (R06), idempotent retries; a real outage was not provoked                                         |
| Stop All tested at every worker stage                                          | PASS                                     | AT-006 and the two stop fault points; worker stages covered by the matrix and R04 tests                                                      |
| diagnostic export redaction verified                                           | PASS                                     | X03 tests (re-redaction, log caps)                                                                                                           |
| changelog prepared                                                             | PASS                                     | `CHANGELOG.md`                                                                                                                               |
| reference hardware recorded; budgets measured against it                       | NOT VERIFIED                             | profile REF-HW-1 recorded; only a non-reference exploratory run exists                                                                       |
| Native Permission Posture verified in the real environment (ADR-009)           | PARTIAL                                  | probe implemented and tested; real matrix run during the A06 spikes and not repeated at release                                              |
| guarded UI automation consent flow and revocation tested (ADR-010)             | PASS                                     | onboarding consent tests; live revocation test (`PersistedConsent`)                                                                          |
| locked or disconnected session degradation tested (ADR-012)                    | PARTIAL                                  | `SessionGuard` with a fake probe; a real lock NOT VERIFIED; no component polls the guard yet                                                 |
| background, tray, login auto-start, reboot continuation tested (ADR-012)       | NOT VERIFIED                             | continuation setting proven in the matrix; tray, auto-start at login and a real reboot not run                                               |
| untrusted-repository behavior and trust flow tested (ADR-013)                  | PASS                                     | AT-020, X01                                                                                                                                  |
| merge lane conflict and validation-failure paths tested (ADR-011)              | PASS                                     | AT-018 x3                                                                                                                                    |
| promotion with and without a remote tested (ADR-014)                           | PASS                                     | AT-021 x2 (a real GitHub pull request NOT VERIFIED)                                                                                          |
| state-store corruption, failed-migration and update-during-run behavior tested | PASS (units and integration)             | AT-022, upgrade tests, `update::can_apply`                                                                                                   |
| clean-machine install, first launch, upgrade, uninstall on the H05 environment | NOT VERIFIED                             | `CLEAN_MACHINE_EVIDENCE.md`                                                                                                                  |
| web-view runtime distribution modes verified where possible                    | NOT VERIFIED                             | the configured mode is the Evergreen bootstrapper; the missing-runtime path is NOT VERIFIED                                                  |
| production Windows Authenticode signature verified                             | FAIL                                     | see the gate above                                                                                                                           |
| updater artifacts verified against the production public key                   | FAIL                                     | see the gate above                                                                                                                           |
| Stitch reference asset present and fidelity review completed against it        | PARTIAL                                  | asset present; the review was done by the implementing agent (not independent) and is recorded in `docs/ui/FIDELITY_REVIEW_2026-10-15.md`    |

Items counted in the table above: PASS 14 (the rows marked PASS or PASS with a qualifier), FAIL 3 gate rows plus the checklist FAILs, NOT VERIFIED as listed.

## Other Z06 requirements

- **Independent whole-codebase review**: NOT DONE (ADR-023). It is the single most valuable remaining step.
- **Security evidence**: X04 suite, cargo-deny (advisories, licenses, bans, sources; one real advisory was fixed during this build: rustls to 0.23.46), SBOM script, secret scanning.
- **Performance evidence**: `docs/ui/PERFORMANCE_RESULTS_2026-10-15.md`, a **non-reference exploratory** run only.
- **Changelog**: `CHANGELOG.md`.
- **Release tag**: none. The repository policy ties a tag to a release-ready verdict, which this is not.
- **Lifecycle checks on the clean environment (AT-019, AT-023)**: NOT VERIFIED (no guest). **Production-signed clean-machine behaviour**: NOT VERIFIED.

## What would make it release-ready

1. Provision production signing (H04), record it in `docs/release/SIGNING.md`, and run the production profile; verify both signatures.
2. Build the N-1 and N installers and run Z07 on the H05 guest; run the Z06 lifecycle checks there.
3. With the Desktop app and the owner present: AT-011, AT-013, AT-017, AT-025, AT-026 on the real path (Z04).
4. A reference-hardware performance run (exclusive machine, AC power) against `PERFORMANCE_BUDGET.md`.
5. An independent whole-codebase and security review, and an independent fidelity review.
6. Close the wiring gaps listed in the traceability report.
7. Resolve H01 (Google's position on external orchestration).
