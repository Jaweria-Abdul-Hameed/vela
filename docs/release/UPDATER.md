# Updater (Z02)

Vela updates itself with the Tauri updater. This page is how an update is signed, when it may be applied, what is backed up, and how
a failed update is undone. It is **not** Windows code signing (that is Authenticode, `DEV_SIGNING.md`).

## Signature

An update artifact carries a Tauri updater signature (a minisign Ed25519 signature). Vela verifies it against the public key in its
configuration before installing; `update::verify_artifact` is the same check, tested with a tampered artifact and an artifact signed
with another key (both rejected).

## Keys and their separation

| Key | Where it lives | Used for |
|---|---|---|
| **Production** updater key | the private half in the release owner's secret store; the public half is the value H04 records | real releases only |
| **Disposable test** updater key | generated in memory (or into a temporary folder) by each test run; never committed, never reused | tests and local trials |

* The committed `tauri.conf.json` and `tauri.release.conf.json` contain **no** updater section. Tests assert this.
* `tauri.updater-test.conf.json` is the overlay that supplies a test public key and a local endpoint (`http://127.0.0.1:1421`). Its key is a
  placeholder; the test run fills it in. Nothing in the repository holds a real updater key of either kind.
* A test key is recognisable: its public-key comment says `disposable test updater key`. `update::check_release_config` makes a release
  configuration that carries such a key (or an updater section with no key) **fail closed**, and the release workflow never references the test overlay (tested).
* The final production hosting location of the update endpoint is an H04 decision and is not made here.

### Trying an update locally with a test key
```powershell
cd apps\desktop
npx tauri signer generate -w $env:TEMP\vela-test-updater.key --ci -p ""   # a throwaway keypair; delete it afterwards
# put the generated public key into a copy of src-tauri\tauri.updater-test.conf.json (the copy is not committed)
npx tauri build --config src-tauri/tauri.release.conf.json --config <your copy>
# serve latest.json and the signed installer from 127.0.0.1:1421 and let the app check
```

## When an update may be applied

`update::can_apply`: never while a run is starting, running or stopping (the update waits, and says so); never without the user's explicit
action, even when nothing is running. Paused and waiting runs do not block, and resume afterwards through the normal startup reconciliation.

## Backup and rollback

1. Before applying, the application takes a consistent copy of the state store (`StateStore::backup_now`).
2. On the next start the store takes its own backup before any migration, applies the migrations, and checks integrity (F06).
3. If a migration fails, that backup is restored and the new build refuses to run newer logic on the old store (`MigrationFailed`); the
   previous build still opens the store and sees exactly what it had. An older build also refuses a store a newer build migrated.

Tests: `tests/integration/tests/upgrade.rs` (N-1 to N keeps the data, a failed migration rolls back, the old build refuses newer state),
`apps/desktop/src-tauri/src/update/tests.rs` (signature, test-key guard, deferral rules).

## The N-1 and N installers (for Z07)

Z07 upgrades a clean machine from an N-1 installer to an N installer. Both are built the same way as any development installer
(`DEV_SIGNING.md`): check out the previous release tag and build the N-1 installer, build N from the current commit, sign both with a
development certificate, and publish them with a SHA-256 manifest. **That pair has not been built or run in this repository's CI;
the real-machine upgrade is Z07's evidence and is NOT VERIFIED until it runs.**

## WebView2 refresh

A long-running application keeps the WebView2 runtime it started with. After an update (and after the runtime itself updates), Vela offers
to restart so the new runtime is used; it never restarts a window with an active run on its own.
