import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { describe, it } from 'node:test';
import { applyAllowlist, loadRules, parseDiff, scanFile, scanRange } from './scan-secrets.mjs';

const rules = loadRules();
// Built at run time so this test file itself never contains a matching literal.
const fakeAwsKey = ['AKIA', 'IOSFODNN7EXAMPLE'].join('');
const fakeGithubToken = ['ghp', '_', 'a'.repeat(36)].join('');
const fakeAssignment = ['pass', 'word = "', 'a'.repeat(24), '"'].join('');
const fakeKeyBlock = ['-----BEGIN RSA ', 'PRIVATE KEY-----'].join('');

function lines(...texts) {
  return texts.map((text, i) => ({ line: i + 1, text }));
}

describe('secret rules', () => {
  it('flag credential-like file names and allow the example files', () => {
    for (const bad of [
      '.env',
      'apps/x/.env.local',
      'cert.pem',
      'deploy/server.key',
      'home/.gemini/oauth_creds.json',
      'id_rsa',
    ]) {
      assert.ok(scanFile(rules, bad, []).length > 0, bad);
    }
    for (const ok of ['.env.example', 'README.md', 'src/environment.ts', 'docs/gemini/notes.md']) {
      assert.deepEqual(scanFile(rules, ok, []), [], ok);
    }
  });

  it('flag known token shapes and key blocks in added lines', () => {
    const found = scanFile(
      rules,
      'src/a.ts',
      lines(`const k = "${fakeAwsKey}";`, fakeGithubToken, fakeKeyBlock, fakeAssignment),
    );
    assert.deepEqual(found.map((f) => f.rule).sort(), [
      'aws-access-key',
      'generic-secret-assignment',
      'github-token',
      'private-key-block',
    ]);
    assert.deepEqual(
      scanFile(rules, 'src/a.ts', lines('const answer = 42;', 'let token = nextToken();')),
      [],
    );
  });

  it('parse a unified diff into added lines with line numbers', () => {
    const diff = [
      'diff --git a/x b/x',
      '--- a/x',
      '+++ b/x',
      '@@ -0,0 +5,2 @@',
      '+one',
      '+two',
    ].join('\n');
    assert.deepEqual(parseDiff(diff), [
      {
        file: 'x',
        lines: [
          { line: 5, text: 'one' },
          { line: 6, text: 'two' },
        ],
      },
    ]);
  });
});

describe('allowlist for secrets', () => {
  it('suppresses only a justified, unexpired, matching entry', () => {
    const findings = [{ rule: 'env-file', file: 'tests/fixtures/.env', line: 0 }];
    const entry = {
      id: 'env-file',
      scope: 'secrets',
      path: '^tests/fixtures/',
      justification: 'a fixture with fake values only',
      expires: '2099-01-01',
    };
    assert.deepEqual(applyAllowlist(findings, { entries: [entry] }), []);
    assert.equal(
      applyAllowlist([{ rule: 'env-file', file: 'src/.env', line: 0 }], { entries: [entry] })
        .length,
      1,
    );
    assert.throws(() =>
      applyAllowlist(findings, { entries: [{ ...entry, expires: '2001-01-01' }] }),
    );
    assert.throws(() => applyAllowlist(findings, { entries: [{ ...entry, justification: '' }] }));
  });
});

describe('scanning a real range', () => {
  it('fails a branch that adds a fake secret and passes a clean one', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'vela-scan-'));
    const git = (...args) =>
      execFileSync('git', ['-c', 'user.name=t', '-c', 'user.email=t@example.com', ...args], {
        cwd: dir,
        encoding: 'utf8',
      });
    try {
      git('init', '-q');
      fs.writeFileSync(path.join(dir, 'a.txt'), 'hello\n');
      git('add', '.');
      git('commit', '-q', '-m', 'base');
      fs.writeFileSync(path.join(dir, 'b.txt'), 'clean content\n');
      git('add', '.');
      git('commit', '-q', '-m', 'clean');
      assert.deepEqual(scanRange('HEAD~1..HEAD', dir), []);
      fs.writeFileSync(path.join(dir, 'c.txt'), `key = ${fakeAwsKey}\n`);
      git('add', '.');
      git('commit', '-q', '-m', 'leak');
      const found = scanRange('HEAD~1..HEAD', dir);
      assert.equal(found.length, 1);
      assert.equal(found[0].rule, 'aws-access-key');
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });
});
