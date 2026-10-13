// The release configuration is checked as data: per-user NSIS, the hook as a sidecar, a WebView2 mode, no development material.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

const read = (p) => JSON.parse(readFileSync(new URL(`../apps/desktop/src-tauri/${p}`, import.meta.url), 'utf8'));
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
