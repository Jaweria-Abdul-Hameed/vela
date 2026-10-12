import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { buildSbom, npmComponents, rustComponents } from './generate-sbom.mjs';

describe('software bill of materials', () => {
  it('lists registry crates and skips workspace members', () => {
    const metadata = {
      packages: [
        {
          name: 'serde',
          version: '1.0.0',
          source: 'registry+https://github.com/rust-lang/crates.io-index',
          license: 'MIT OR Apache-2.0',
        },
        { name: 'vela-domain', version: '0.0.0', source: null, license: null },
      ],
    };
    const components = rustComponents(metadata);
    assert.equal(components.length, 1);
    assert.equal(components[0].purl, 'pkg:cargo/serde@1.0.0');
    assert.deepEqual(components[0].licenses, [{ expression: 'MIT OR Apache-2.0' }]);
  });

  it('lists installed npm packages and skips the root and workspace links', () => {
    const lock = {
      packages: {
        '': { name: 'vela' },
        'node_modules/left-pad': { version: '1.3.0', license: 'MIT' },
        'node_modules/@scope/pkg': { version: '2.0.0' },
        'node_modules/vela-desktop': { link: true },
        'apps/desktop': { version: '0.0.0' },
      },
    };
    const components = npmComponents(lock);
    assert.deepEqual(
      components.map((c) => c.purl),
      ['pkg:npm/left-pad@1.3.0', 'pkg:npm/%40scope/pkg@2.0.0'],
    );
  });

  it('produces a sorted CycloneDX document', () => {
    const sbom = buildSbom(
      [{ type: 'library', name: 'b', version: '1', purl: 'pkg:cargo/b@1', licenses: [] }],
      [{ type: 'library', name: 'a', version: '1', purl: 'pkg:npm/a@1', licenses: [] }],
      new Date('2026-10-12T00:00:00Z'),
    );
    assert.equal(sbom.bomFormat, 'CycloneDX');
    assert.deepEqual(
      sbom.components.map((c) => c.purl),
      ['pkg:cargo/b@1', 'pkg:npm/a@1'],
    );
    assert.equal(sbom.metadata.timestamp, '2026-10-12T00:00:00.000Z');
  });
});
