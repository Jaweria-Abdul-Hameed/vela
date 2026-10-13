# Development signing and the installer (Z01)

This is how the Windows installer is built, how it is signed for development, and where `vela-hook` lives once installed.

## What gets built

`tauri.release.conf.json` is the release overlay on `tauri.conf.json`: one **per-user NSIS** installer (no administrator rights), the
WebView2 runtime installed through the Evergreen **download bootstrapper** (change `webviewInstallMode.type` to `embedBootstrapper` or
`offlineInstaller` for machines without network access), and `vela-hook` as a **sidecar**.

The ordinary build (`tauri.conf.json` alone) has an empty `bundle`, so day-to-day builds and CI never need the sidecar or a certificate.

## Where `vela-hook` is

The sidecar is installed as `vela-hook.exe` **next to `Vela.exe`**. In development Cargo puts it next to the debug executable, so the
same rule works: `wiring::adapters::hook_exe_path()` returns `<directory of the running executable>\vela-hook.exe`, and the Antigravity
profile generator copies that file into each worker's isolated profile.

`node scripts/prepare-sidecar.mjs` builds the hook in release mode and copies it to
`apps/desktop/src-tauri/binaries/vela-hook-<target triple>.exe`, which is where the bundler looks for it. Run it before `tauri build`.

## Uninstall

Uninstalling removes the application, its Start menu shortcuts, the tray entry and the auto-start entry. It never touches your
repositories. Vela's data (`%LOCALAPPDATA%\Vela`: the state database, worker profiles and worktrees) is **kept** unless you tick the
data-removal box in the uninstaller (unticked by default).

## Development signing

The **development** profile of `.github/workflows/release.yml` creates a throwaway self-signed code-signing certificate on the runner
(30 days, subject `CN=Vela Development Signing (not production trust)`), builds with it through a temporary overlay, then checks that
the installer carries exactly that certificate. The certificate is not exported, not committed and not uploaded; the overlay file lives
in the runner's temporary folder. `tauri.dev-signing.conf.json` in the repository is only the shape of the overlay and contains no
certificate.

A development-signed installer is **not trusted** by Windows. To build one locally:

```powershell
node scripts\prepare-sidecar.mjs
$cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject 'CN=Vela Development Signing' -CertStoreLocation Cert:\CurrentUser\My
# write an overlay with { "bundle": { "windows": { "certificateThumbprint": "<thumbprint>", "digestAlgorithm": "sha256" } } }
cd apps\desktop
npx tauri build --config src-tauri/tauri.release.conf.json --config <your overlay>
```

### Smart App Control

On a machine with **Smart App Control** on, Windows may refuse to run unsigned or self-signed development binaries (the hook, the
installer, even freshly built test executables). That is the protection working. This project does not change that setting and it is not
a development prerequisite: if it blocks a binary, the failure is real (and the hook then fails closed: nothing it guards runs).
Use a machine or a virtual machine with it off for installer testing, or a certificate your organisation trusts.

## Production signing

The **production** profile reads the Authenticode certificate from release secrets (`AUTHENTICODE_PFX_BASE64`,
`AUTHENTICODE_PASSWORD`), supplied through H04. It fails closed when they are absent and when the certificate is self-signed. Executing
and verifying it is Z06; running the installer on a clean machine is Z07. Authenticode is separate from the updater's own signature (Z02).
