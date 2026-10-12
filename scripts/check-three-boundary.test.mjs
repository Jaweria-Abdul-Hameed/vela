// The Three.js import boundary (F05) is an ESLint rule; this test proves the rule fires outside packages/ui/src/canvas.
// Real file paths are used so typed linting can resolve them; the linted text is replaced by the test.
import assert from 'node:assert/strict';
import { ESLint } from 'eslint';
import { describe, it } from 'node:test';

const config = await import('../eslint.config.js');

async function messages(filePath, code) {
  const eslint = new ESLint({
    overrideConfigFile: true,
    overrideConfig: config.default,
    cwd: process.cwd(),
  });
  const [result] = await eslint.lintText(code, { filePath });
  return result.messages.filter((m) => m.ruleId === 'no-restricted-imports');
}

describe('Three.js import boundary', () => {
  it('rejects three and @react-three imports outside the canvas module', async () => {
    assert.equal(
      (
        await messages(
          'apps/desktop/src/registry.ts',
          "import * as THREE from 'three';\nexport { THREE };",
        )
      ).length,
      1,
    );
    assert.equal(
      (
        await messages(
          'packages/ui/src/a11y/index.ts',
          "import { Canvas } from '@react-three/fiber';\nexport { Canvas };",
        )
      ).length,
      1,
    );
    assert.equal(
      (
        await messages(
          'apps/desktop/src/state/store.ts',
          "import { Vector3 } from 'three/src/math/Vector3.js';\nexport { Vector3 };",
        )
      ).length,
      1,
    );
  });

  it('allows them inside packages/ui/src/canvas', async () => {
    assert.equal(
      (
        await messages(
          'packages/ui/src/canvas/quality.ts',
          "import * as THREE from 'three';\nexport { THREE };",
        )
      ).length,
      0,
    );
  });
});
