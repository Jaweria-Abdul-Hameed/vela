// The release configuration is checked as data: per-user NSIS, the hook as a sidecar, a WebView2 mode, no development material.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

const read = (p) =>
  JSON.parse(readFileSync(new URL(`../apps/desktop/src-tauri/${p}`, import.meta.url), 'utf8'));
const base = read('tauri.conf.json');
const release = read('tauri.release.conf.json');
const dev = read('tauri.dev-signing.conf.json');

test('the release bundle is a per-user NSIS installer', () => {
  assert.deepEqual(release.bundle.targets, ['nsis']);
  assert.equal(release.bundle.windows.nsis.installMode, 'currentUser');
});

test('vela-hook ships as a sidecar so it lands next to the executable', () => {
  assert.deepEqual(release.bundle.externalBin, ['binaries/vela-hook']);
});

test('WebView2 has an explicit install mode', () => {
  const mode = release.bundle.windows.webviewInstallMode;
  assert.ok(['downloadBootstrapper', 'embedBootstrapper', 'offlineInstaller'].includes(mode.type));
});

test('the base configuration never needs the sidecar, so ordinary builds work without it', () => {
  assert.deepEqual(base.bundle, {});
});

test('the development signing overlay names no real certificate and no timestamp authority secret', () => {
  assert.equal(dev.bundle.windows.certificateThumbprint, 'SET-BY-THE-BUILD');
  assert.equal(JSON.stringify(dev).match(/[0-9A-F]{40}/i), null);
});

test('the application loads its own bundle, not a development server, in production', () => {
  assert.equal(base.build.frontendDist, '../dist');
  assert.ok(!JSON.stringify(release).includes('localhost'));
});

const updaterTest = read('tauri.updater-test.conf.json');

test('the committed base and release configurations carry no updater key', () => {
  assert.equal(base.plugins?.updater, undefined);
  assert.equal(release.plugins?.updater, undefined);
});

test('the updater test overlay holds only a placeholder key and a local endpoint', () => {
  const u = updaterTest.plugins.updater;
  assert.equal(u.pubkey, 'SET-BY-THE-TEST-RUN');
  assert.ok(u.endpoints.every((e) => e.startsWith('http://127.0.0.1')));
  // A real minisign public key is base64 of "untrusted comment: ..."; none is committed.
  assert.ok(!JSON.stringify(updaterTest).includes('dW50cnVzdGVk'));
});

test('the release workflow never mixes in the updater test overlay', () => {
  const workflow = readFileSync(
    new URL('../.github/workflows/release.yml', import.meta.url),
    'utf8',
  );
  assert.ok(!workflow.includes('updater-test'));
});
