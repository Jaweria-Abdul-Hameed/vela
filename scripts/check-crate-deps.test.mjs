import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { after, describe, it } from 'node:test';
import {
  ALLOWED_VELA_DEPS,
  HARNESS_VELA_DEPS,
  checkDependencyDirection,
  checkHookTree,
} from './check-crate-deps.mjs';

const dep = (name, extra = {}) => ({
  name,
  kind: null,
  target: null,
  uses_default_features: true,
  features: [],
  ...extra,
});

const windowsOnly = { target: 'cfg(windows)' };
const hookDomain = dep('vela-domain', { uses_default_features: false, features: ['hook-table'] });

/** Metadata with every known member present; `overrides` replaces a member's dependency list. */
function metadata(overrides = {}) {
  const base = {
    'vela-domain': [dep('serde')],
    'vela-persistence': [dep('vela-domain')],
    'vela-process': [dep('vela-domain')],
    'vela-git': [dep('vela-domain'), dep('vela-process')],
    'vela-adapters': [dep('vela-domain'), dep('vela-process')],
    'vela-uia': [dep('vela-domain'), dep('vela-process'), dep('windows', windowsOnly)],
    'vela-hook': [dep('serde'), dep('serde_json'), hookDomain],
    'vela-core': [
      dep('vela-domain'),
      dep('vela-persistence'),
      dep('vela-testkit', { kind: 'dev' }),
    ],
    'vela-testkit': [dep('vela-domain'), dep('vela-process')],
    'vela-desktop': [
      dep('vela-domain'),
      dep('vela-persistence'),
      dep('vela-git'),
      dep('vela-adapters'),
      dep('vela-core'),
      dep('vela-uia', windowsOnly),
      dep('tauri'),
      dep('tauri-plugin-updater'),
    ],
    'vela-integration-tests': [dep('vela-core'), dep('vela-testkit')],
    'vela-e2e-tests': [dep('vela-core'), dep('vela-testkit')],
    'vela-antigravity-compat': [dep('vela-adapters'), dep('vela-testkit')],
    'fake-approval-window': [dep('wry'), dep('tao')],
    ...overrides,
  };
  return {
    packages: Object.entries(base).map(([name, dependencies]) => ({ name, dependencies })),
  };
}

const violationsFor = (overrides) => checkDependencyDirection(metadata(overrides));

describe('allowed edges', () => {
  it('accepts the frozen dependency graph', () => {
    assert.deepEqual(violationsFor({}), []);
  });

  it('accepts a dev-dependency on vela-testkit from a production crate', () => {
    const v = violationsFor({
      'vela-git': [dep('vela-domain'), dep('vela-process'), dep('vela-testkit', { kind: 'dev' })],
    });
    assert.deepEqual(v, []);
  });

  it('covers every workspace member with a policy entry', () => {
    for (const { name } of metadata().packages) {
      assert.ok(name in ALLOWED_VELA_DEPS || name in HARNESS_VELA_DEPS, `${name} has no policy`);
    }
  });
});

describe('forbidden edges', () => {
  const cases = [
    [
      'vela-core -> vela-git (the ticket example)',
      { 'vela-core': [dep('vela-domain'), dep('vela-git')] },
      /\[rule 1-3\] vela-core must not depend on vela-git/,
    ],
    [
      'vela-core -> vela-adapters',
      { 'vela-core': [dep('vela-adapters')] },
      /vela-core must not depend on vela-adapters/,
    ],
    [
      'vela-core -> vela-process',
      { 'vela-core': [dep('vela-process')] },
      /vela-core must not depend on vela-process/,
    ],
    ['vela-domain -> any Vela crate', { 'vela-domain': [dep('vela-process')] }, /\[rule 1\]/],
    [
      'a vela-domain dev-dependency on a Vela crate',
      { 'vela-domain': [dep('vela-testkit', { kind: 'dev' })] },
      /\[rule 1\]/,
    ],
    [
      'vela-git -> vela-core',
      { 'vela-git': [dep('vela-domain'), dep('vela-core')] },
      /vela-git must not depend on vela-core/,
    ],
    [
      'vela-adapters -> vela-git',
      { 'vela-adapters': [dep('vela-domain'), dep('vela-git')] },
      /vela-adapters must not depend on vela-git/,
    ],
    [
      'vela-uia -> vela-adapters',
      { 'vela-uia': [dep('vela-adapters'), dep('windows', windowsOnly)] },
      /vela-uia must not depend on vela-adapters/,
    ],
    [
      'an adapter -> vela-persistence',
      { 'vela-git': [dep('vela-persistence')] },
      /vela-git must not depend on vela-persistence/,
    ],
    [
      'vela-persistence -> vela-core',
      { 'vela-persistence': [dep('vela-domain'), dep('vela-core')] },
      /vela-persistence must not depend on vela-core/,
    ],
    [
      'a normal dependency on vela-testkit from a production crate',
      { 'vela-core': [dep('vela-testkit')] },
      /vela-core must not depend on vela-testkit/,
    ],
    ['anything -> vela-hook', { 'vela-core': [dep('vela-hook')] }, /must not depend on vela-hook/],
    [
      'anything -> vela-desktop',
      { 'vela-integration-tests': [dep('vela-desktop')] },
      /must not depend on vela-desktop/,
    ],
    [
      'Tauri outside the composition root',
      { 'vela-core': [dep('tauri')] },
      /\[rule 4\] vela-core must not depend on tauri/,
    ],
    [
      'a Tauri plugin outside the composition root',
      { 'vela-adapters': [dep('tauri-plugin-notification')] },
      /\[rule 4\]/,
    ],
    [
      'vela-hook with an extra crate',
      { 'vela-hook': [dep('serde'), dep('petgraph'), hookDomain] },
      /\[rule 5\] vela-hook must not depend on petgraph/,
    ],
    [
      'vela-hook with default features on',
      { 'vela-hook': [dep('serde'), dep('vela-domain')] },
      /\[rule 5\].*hook-table/,
    ],
    [
      'vela-hook with extra domain features',
      {
        'vela-hook': [
          dep('vela-domain', {
            uses_default_features: false,
            features: ['hook-table', 'fault-injection'],
          }),
        ],
      },
      /\[rule 5\].*hook-table/,
    ],
    [
      'a vela-process dev-dependency on vela-testkit (duplicate crate copies)',
      { 'vela-process': [dep('vela-domain'), dep('vela-testkit', { kind: 'dev' })] },
      /dev.* vela-process must not use vela-testkit/,
    ],
    [
      'a vela-uia consumer outside cfg(windows)',
      { 'vela-desktop': [dep('vela-uia'), dep('tauri')] },
      /rule 6.*vela-desktop must depend on vela-uia only under cfg.windows/,
    ],
    [
      'vela-uia depending on windows outside cfg(windows)',
      { 'vela-uia': [dep('vela-domain'), dep('windows')] },
      /\[rule 6\]/,
    ],
    [
      'an unknown (speculative provider) crate',
      { 'vela-claude-adapter': [dep('vela-domain')] },
      /vela-claude-adapter is not a known workspace member/,
    ],
    [
      'the fake approval window depending on a Vela crate',
      { 'fake-approval-window': [dep('vela-domain')] },
      /fake-approval-window must not depend on vela-domain/,
    ],
  ];
  for (const [name, overrides, pattern] of cases) {
    it(`rejects ${name}`, () => {
      const v = violationsFor(overrides);
      assert.ok(
        v.some((line) => pattern.test(line)),
        `expected ${pattern}, got ${JSON.stringify(v)}`,
      );
    });
  }
});

describe('hook dependency tree (rule 5, resolved)', () => {
  it('accepts serde-only trees', () => {
    assert.deepEqual(
      checkHookTree(
        ['vela-hook v0.0.0', 'serde v1.0.229', 'serde_json v1.0.151', 'vela-domain v0.0.0'].join(
          '\n',
        ),
      ),
      [],
    );
  });

  for (const crate of ['petgraph', 'rusqlite', 'ts-rs', 'tauri', 'tauri-plugin-updater']) {
    it(`rejects ${crate} anywhere in the tree`, () => {
      const v = checkHookTree(
        ['vela-hook v0.0.0', 'vela-domain v0.0.0', `${crate} v1.0.0`].join('\n'),
      );
      assert.equal(v.length, 1);
      assert.match(v[0], new RegExp(`contains ${crate}`));
    });
  }
});

describe('CLI against a real cargo workspace', () => {
  const script = path.join(path.dirname(fileURLToPath(import.meta.url)), 'check-crate-deps.mjs');
  // Skipping is allowed on a developer machine without cargo, never in CI (the illegal-edge demonstration is acceptance evidence).
  const cargoFound = spawnSync('cargo', ['--version']).status === 0;
  if (!cargoFound && process.env.CI) throw new Error('cargo must be available in CI');
  const cargoMissing = !cargoFound && 'cargo not on PATH';

  /** Creates a throwaway workspace whose vela-core depends on `coreDeps`; returns its manifest path. */
  const roots = [];
  after(() => {
    for (const r of roots) fs.rmSync(r, { recursive: true, force: true });
  });

  function workspace(coreDeps) {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'vela-depcheck-'));
    roots.push(root);
    const names = ['vela-domain', 'vela-persistence', 'vela-process', 'vela-git', 'vela-core'];
    fs.writeFileSync(
      path.join(root, 'Cargo.toml'),
      `[workspace]\nresolver = "3"\nmembers = [${names.map((n) => `"${n}"`).join(', ')}]\n`,
    );
    for (const name of names) {
      fs.mkdirSync(path.join(root, name, 'src'), { recursive: true });
      const deps = name === 'vela-core' ? coreDeps : [];
      const lines = deps.map((d) => `${d} = { path = "../${d}" }`).join('\n');
      fs.writeFileSync(
        path.join(root, name, 'Cargo.toml'),
        `[package]\nname = "${name}"\nversion = "0.0.0"\nedition = "2024"\n\n[dependencies]\n${lines}\n`,
      );
      fs.writeFileSync(path.join(root, name, 'src', 'lib.rs'), '');
    }
    return path.join(root, 'Cargo.toml');
  }

  it('passes a legal workspace', { skip: cargoMissing }, () => {
    const manifest = workspace(['vela-domain', 'vela-persistence']);
    const out = execFileSync('node', [script, '--manifest-path', manifest], { encoding: 'utf8' });
    assert.match(out, /crate dependency check passed/);
  });

  it('fails when vela-core depends on vela-git', { skip: cargoMissing }, () => {
    const manifest = workspace(['vela-domain', 'vela-git']);
    const result = spawnSync('node', [script, '--manifest-path', manifest], { encoding: 'utf8' });
    assert.equal(result.status, 1);
    assert.match(result.stderr, /vela-core must not depend on vela-git/);
  });
});
