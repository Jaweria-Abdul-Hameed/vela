# H05: Clean-Machine Environment Research and Decisions (2026-10-03)

Date accessed: **2026-10-03**. Research and decisions only. Nothing was installed, no ISO was downloaded, no VM was created, no host setting was changed (no Windows feature, BIOS or UEFI, VBS, Memory Integrity, Smart App Control, or WSL change), and nothing was spent. **H05 is OPEN: the route is decided and the environment is NOT YET PROVISIONED.**

## Decisions (user, 2026-10-03)

-   **Route:** a local Windows 11 virtual machine under **VMware Workstation Pro** on the development machine (REF-HW-1, Windows 11 Home). This is Vela's selected environment, not the only technically valid hypervisor. A spare physical Windows machine is optional supplementary validation, not a v1 prerequisite.
-   **Guest:** the official Microsoft **Windows 11 Enterprise 90-day Evaluation**, a disposable guest. It is Enterprise, not Home; time-limited; and no licensing route for permanent infrastructure is decided. Testing on Windows Home or another edition is added deliberately if release validation requires it. No licensing right beyond Microsoft's published terms is assumed.
-   **Resources (configurable starting point, not a performance requirement):** 4 vCPU; about 4 to 6 GB guest RAM; a dynamically allocated disk with headroom for Windows, snapshots, and the N-1 and N artifacts (estimate 60 to 100 GB); virtual TPM 2.0 and Secure Boot; no nested virtualization; no GPU passthrough. The VM is a quiet-machine workload on a 16 GB host.
-   **States:** S0 pristine baseline, S1 N-1 installed (from S0), S2 Git installed (from S0), each created only when a scenario needs it.
-   **WebView2:** removal from Windows 11 is not a prerequisite; a missing-runtime path that cannot be obtained by a supported method is recorded NOT VERIFIED.
-   **Smart App Control:** not mandated on or off; its actual state is recorded for every relevant run.
-   **Artifact transfer:** SHA-256 manifest and artifact built outside the guest; read-only virtual disk or ISO; verification in the guest with built-in Windows facilities; no permanent shared folder, drag-and-drop, or clipboard dependency on the baseline; network off unless a scenario needs it; revert after each scenario.
-   **Graph:** H05 no longer blocks Z01; the new ticket Z07 owns the environment-dependent evidence (see `docs/issues/graph/AUDIT_PROMPT7.md`).

## Evidence labels

| Fact | Source | Label |
|---|---|---|
| Windows Sandbox is not supported on Windows Home | Microsoft Learn, Windows Sandbox overview (page dated 2026-03-29): "Windows Sandbox is currently not supported on Windows Home edition." | Verified (primary) |
| The Hyper-V role cannot be installed on Windows 11 Home | Microsoft Learn, Install Hyper-V (page dated 2026-02-16): "The Hyper-V role can't be installed on Windows 10 Home or Windows 11 Home." | Verified (primary) |
| VMware Workstation Pro is free for commercial, educational, and personal use | Broadcom/VMware announcement (VMware by Broadcom blog, 2024-11-11), as returned by search | Verified at the level of the announcement; the current license terms were not re-read |
| With Hyper-V or the Windows Hypervisor Platform active, Workstation Pro VMs run on the Windows Hypervisor Platform; "Host VBS Mode" VMs have functional limitations versus traditional mode (details not read) | Broadcom TechDocs, "Running Workstation on a Hyper-V Enabled Host" | Verified (primary), details unread |
| Host state: a hypervisor is present, VBS and a security service are running, 15.6 GB RAM with 2.4 GB free during the check, 459 GB free on C: | Read-only queries, 2026-10-03 | Observed (point in time) |
| VirtualBox 7 supports a virtual TPM 2.0 and EFI Secure Boot | VirtualBox forum posts | Unverified (secondary); the manual page for Hyper-V coexistence returned HTTP 404 |
| Windows 11 guest minimums (about 4 GB RAM, 64 GB storage, TPM 2.0, Secure Boot capable) | Not re-fetched this turn | Unverified here; confirm from Microsoft's Windows 11 requirements page when provisioning |
| The Windows 11 Enterprise Evaluation is a 90-day evaluation offered for a virtual machine | Microsoft Evaluation Center page, as returned by search | Verified at the level of the page title and summary; terms not read in full |
| An unactivated or expired evaluation shuts down periodically and cannot be converted to a production edition | A Microsoft Q&A answer, as returned by search | Unverified (secondary) |
| The Evergreen WebView2 runtime "will be included as part of the Windows 11 operating system"; the distribution page documents registry-key detection and the silent bootstrapper install but no uninstall procedure | Microsoft Learn, Distribute your app and the WebView2 Runtime (page dated 2026-10-02) | Verified (primary); absence of a documented removal procedure is the finding |
| Smart App Control blocks unknown unsigned code in enforcement mode and allows code signed by a certificate chaining to the Microsoft Trusted Root Program | Microsoft Learn, Smart App Control (page dated 2025-11-18) | Verified (primary) |

## What the clean environment must prove (by acceptance scenario)

Clean install, first launch with the documented missing-prerequisite behavior, no dependence on development tooling, WebView2 distribution behavior where technically possible, development Authenticode verification, N-1 to N upgrade with backup and rollback, uninstall, and reboot, tray, auto-start, and notification behavior. Fault-point crash tests run in CI and do not need the VM. See `docs/issues/graph/tickets/Z07.md`.
