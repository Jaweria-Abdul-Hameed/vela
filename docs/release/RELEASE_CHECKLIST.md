# Release Checklist

-   [ ] all in-scope requirements traced and verified
-   [ ] unit/integration/e2e suites green
-   [ ] crash-recovery matrix green
-   [ ] security review complete
-   [ ] secret scanning clean
-   [ ] installer tested on clean Windows environment
-   [ ] upgrade/migration tested from previous release
-   [ ] graphics Full/Balanced/Efficiency tested
-   [ ] reduced motion tested
-   [ ] keyboard accessibility tested
-   [ ] 100/500-node graph performance measured
-   [ ] background/minimized resource use measured
-   [ ] Antigravity capability matrix revalidated
-   [ ] Matt Pocock skills integration contract revalidated
-   [ ] GitHub outage behavior tested
-   [ ] Stop All tested at every worker stage
-   [ ] diagnostic export redaction verified
-   [ ] changelog prepared

## Additions (Prompt 3)

-   [ ] reference hardware recorded; performance budgets measured against it
-   [ ] Native Permission Posture verified in the real environment (ADR-009)
-   [ ] guarded UI automation consent flow and revocation tested (ADR-010)
-   [ ] locked/disconnected-session degradation and reconciliation tested (ADR-012)
-   [ ] background operation, tray, login auto-start, and reboot continuation tested (ADR-012)
-   [ ] untrusted-repository behavior and trust flow tested (ADR-013)
-   [ ] merge lane conflict and validation-failure paths tested (ADR-011)
-   [ ] promotion behavior with and without a remote tested (ADR-014)
-   [ ] state-store corruption, failed-migration, and update-during-run behavior tested
-   [ ] clean-machine install, first launch (including missing-prerequisite behavior), upgrade, and uninstall verified on the H05 clean environment (Windows 11 Enterprise Evaluation guest, not Home; every run records the Windows build and edition and the Smart App Control and SmartScreen state); each unverified item is marked NOT VERIFIED with its reason
-   [ ] web-view runtime: the configured bootstrapper, embedded, and offline distribution behavior verified where technically possible; the missing-runtime path is PASS only if a supported absent state was obtained, otherwise recorded as NOT VERIFIED with the reason (never silently passed or waived)
-   [ ] production Windows Authenticode signature verified on the installer and the bundled `vela-hook`
        (`signtool verify /pa`), chaining to the production identity recorded in `docs/release/SIGNING.md`;
        no development certificate present
-   [ ] updater artifacts verified against the production updater public key recorded in
        `docs/release/SIGNING.md`; no test updater key present; custody and backup of the production updater
        private key documented
-   [ ] Stitch reference asset present and fidelity review completed against it
