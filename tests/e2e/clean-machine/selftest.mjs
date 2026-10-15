// Self-test of the in-guest scripts on the build host: the hash-manifest and software-listing logic (Z07). The signature report
// needs a signed file and is exercised on the guest.
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtempSync, writeFileSync, mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

const here = import.meta.dirname;
const ps = (script, args) =>
  spawnSync('powershell', ['-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', join(here, script), ...args], {
    encoding: 'utf8',
  });

test('verify-hashes passes for matching files and fails on a changed or missing one', { skip: process.platform !== 'win32' }, () => {
  const root = mkdtempSync(join(tmpdir(), 'vela-cm-'));
  mkdirSync(join(root, 'sub'));
  writeFileSync(join(root, 'a.bin'), 'alpha');
  writeFileSync(join(root, 'sub', 'b.bin'), 'beta');
  const h = (s) => createHash('sha256').update(s).digest('hex');
  const manifest = join(root, 'manifest.sha256');
  writeFileSync(manifest, `${h('alpha')}  a.bin\n# comment\n${h('beta')} *sub/b.bin\n`);
  const ok = ps('verify-hashes.ps1', ['-Manifest', manifest, '-Root', root]);
  assert.equal(ok.status, 0, ok.stdout + ok.stderr);
  assert.equal(JSON.parse(ok.stdout).verified, true);

  writeFileSync(join(root, 'a.bin'), 'tampered');
  const bad = ps('verify-hashes.ps1', ['-Manifest', manifest, '-Root', root]);
  assert.equal(bad.status, 1);
  assert.equal(JSON.parse(bad.stdout).files[0].status, 'MISMATCH');

  writeFileSync(manifest, `${h('alpha')}  gone.bin\n`);
  const missing = ps('verify-hashes.ps1', ['-Manifest', manifest, '-Root', root]);
  assert.equal(missing.status, 1);
  assert.equal(JSON.parse(missing.stdout).files[0].status, 'MISSING');
});

test('list-software flags development tooling and passes a clean listing', { skip: process.platform !== 'win32' }, () => {
  const dir = mkdtempSync(join(tmpdir(), 'vela-cm-'));
  const clean = join(dir, 'clean.json');
  writeFileSync(clean, JSON.stringify([{ name: 'Vela' }, { name: 'Microsoft Edge' }, { name: 'Git' }]));
  const ok = ps('list-software.ps1', ['-FromJson', clean]);
  assert.equal(ok.status, 0, ok.stdout + ok.stderr);
  assert.equal(JSON.parse(ok.stdout).clean, true);

  const dirty = join(dir, 'dirty.json');
  writeFileSync(
    dirty,
    JSON.stringify([{ name: 'Vela' }, { name: 'Rustup 1.28' }, { name: 'Node.js' }, { name: 'Visual Studio Build Tools 2022' }]),
  );
  const bad = ps('list-software.ps1', ['-FromJson', dirty]);
  assert.equal(bad.status, 1);
  assert.equal(JSON.parse(bad.stdout).development_tooling.length, 3);
});

test('the scripts parse as PowerShell', { skip: process.platform !== 'win32' }, () => {
  for (const s of ['verify-hashes.ps1', 'list-software.ps1', 'signature-report.ps1']) {
    const out = execFileSync(
      'powershell',
      ['-NoProfile', '-NonInteractive', '-Command', `$e=$null;[void][System.Management.Automation.Language.Parser]::ParseFile('${join(here, s)}',[ref]$null,[ref]$e);$e.Count`],
      { encoding: 'utf8' },
    );
    assert.equal(out.trim(), '0', s);
  }
});
