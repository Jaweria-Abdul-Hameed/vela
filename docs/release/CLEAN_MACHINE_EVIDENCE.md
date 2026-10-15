# Clean-machine installer and lifecycle validation (Z07)

**Status: NOT RUN. No clean Windows 11 virtual machine (the H05 environment) was available to the build, so every scenario below is NOT VERIFIED.
What exists is the procedure and the in-guest scripts, tested on the build host.**

## In-guest scripts (built-in Windows facilities only)

Under `tests/e2e/clean-machine/`, run with `powershell -NoProfile -ExecutionPolicy Bypass -File <script>`:

| Script | Purpose | Self-tested on the build host |
|---|---|---|
| `verify-hashes.ps1 -Manifest <sha256 manifest> -Root <dir>` | verifies the artifact manifest in the guest (match, changed, missing) | yes (`selftest.mjs`) |
| `list-software.ps1 [-FromJson <file>]` | installed-software listing from the Uninstall keys; flags Rust, Node.js, Visual Studio or Build Tools | yes |
| `signature-report.ps1 -Path <files>` | `Get-AuthenticodeSignature` status, signer and thumbprint for the installer and the bundled `vela-hook` | parses; needs a signed file, exercised in the guest |

## Environment record (to fill per run)

Windows build and edition (Enterprise Evaluation, not Home), snapshot (S0 pristine, S1 N-1 installed, S2 prerequisites), WebView2 runtime
version, Smart App Control and SmartScreen state (recorded, never mandated), network state, guest tools, date.

## Scenarios and their status

| Scenario | Status |
|---|---|
| S0: pristine install of the development-signed installer (hash manifest verified in the guest) | NOT VERIFIED |
| S0: first launch with no Git and no Antigravity: preflight reports the missing prerequisites with remediation, no crash | NOT VERIFIED on a clean guest. Covered on the build machine by the preflight tests (S04, K10). |
| S0: installed-software listing shows no Rust, Node or Visual Studio tooling needed or present | NOT VERIFIED (script ready) |
| S0: development Authenticode verification of the installer and `vela-hook` | NOT VERIFIED (script ready; a development signature is expected to be reported as not trusted) |
| S0: login auto-start entry exists, the app launches at login after a guest reboot, the entry is removed at uninstall | NOT VERIFIED |
| S0: uninstall removes files, shortcuts and the auto-start entry; repositories untouched; Vela data kept by default | NOT VERIFIED |
| S1: N-1 to N upgrade keeps the state store, migrates after a backup, rolls back a failed migration (two real installer artifacts) | NOT VERIFIED on real installers. The store behaviour is proven by `tests/integration/tests/upgrade.rs`; the N-1 and N installer pair has not been built (see `UPDATER.md`). |
| WebView2: runtime version; bootstrapper, embedded and offline modes | NOT VERIFIED here. The configured mode is the Evergreen download bootstrapper (`tauri.release.conf.json`). A genuinely WebView2-absent guest is not a prerequisite and unsupported removal is forbidden: the missing-runtime path is recorded as NOT VERIFIED, never as passed. |

## Artifact transfer

The artifact and its SHA-256 manifest are created outside the guest and moved in by a read-only virtual disk or ISO; hashes are verified in the
guest with `verify-hashes.ps1`; no shared folder, drag-and-drop or clipboard; the network is off unless a scenario needs it; revert to the right
snapshot after each scenario.

## Not in this ticket

Tray, background operation, hide-on-close, reboot continuation of a real run, notifications with the window closed and termination of the
running app at uninstall are Z06's evidence.
