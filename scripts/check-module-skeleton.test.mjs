import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, it } from 'node:test';
import { checkModuleSkeleton, declaredModules } from './check-module-skeleton.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));

/** @type {import('./check-module-skeleton.mjs').Skeleton} */
const skeleton = {
  rust: {
    demo: {
      src: 'crates/demo/src',
      entry: 'lib.rs',
      modules: {
        alpha: null,
        beta: { closed: true, modules: { one: null, two: null } },
        gamma: { closed: false, modules: { inner: null } },
      },
    },
  },
  typescript: { 'packages/demo': ['src/a/index.ts', 'src/b.tsx'] },
};

function write(root, rel, content = '') {
  const file = path.join(root, rel);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, content);
}

/** A complete, legal tree; `mutate(root)` then breaks it in one specific way. */
function tree(mutate) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'vela-skeleton-'));
  const src = 'crates/demo/src';
  write(root, `${src}/lib.rs`, 'pub mod alpha;\npub mod beta;\npub mod gamma;\n');
  write(root, `${src}/alpha/mod.rs`);
  write(root, `${src}/beta/mod.rs`, 'pub mod one;\npub mod two;\n');
  write(root, `${src}/beta/one/mod.rs`);
  write(root, `${src}/beta/two/mod.rs`);
  write(root, `${src}/gamma/mod.rs`, 'pub mod inner;\n');
  write(root, `${src}/gamma/inner/mod.rs`);
  write(root, 'packages/demo/src/a/index.ts', 'export {};\n');
  write(root, 'packages/demo/src/b.tsx', 'export {};\n');
  mutate?.(root, src);
  return root;
}

describe('declaredModules', () => {
  it('reads plain and visibility-qualified declarations', () => {
    assert.deepEqual(declaredModules('pub mod a;\nmod b;\npub(crate) mod c;\n'), ['a', 'b', 'c']);
  });

  it('ignores comments and cfg(test) modules', () => {
    const src =
      '// pub mod hidden;\n/* pub mod blocked; */\npub mod real;\n\n#[cfg(test)]\nmod tests {\n}\n';
    assert.deepEqual(declaredModules(src), ['real']);
  });
});

describe('checkModuleSkeleton', () => {
  it('accepts an intact skeleton', () => {
    assert.deepEqual(checkModuleSkeleton(skeleton, tree()), []);
  });

  it('detects a missing module', () => {
    const root = tree((r, src) => write(r, `${src}/lib.rs`, 'pub mod alpha;\npub mod gamma;\n'));
    const v = checkModuleSkeleton(skeleton, root);
    assert.ok(
      v.some((l) => /module beta is missing from crates\/demo\/src\/lib\.rs/.test(l)),
      JSON.stringify(v),
    );
  });

  it('detects an extra module in a crate root', () => {
    const root = tree((r, src) =>
      write(r, `${src}/lib.rs`, 'pub mod alpha;\npub mod beta;\npub mod gamma;\npub mod sneaky;\n'),
    );
    const v = checkModuleSkeleton(skeleton, root);
    assert.ok(
      v.some((l) => /extra module sneaky/.test(l)),
      JSON.stringify(v),
    );
  });

  it('detects a missing nested module in a closed group', () => {
    const root = tree((r, src) => write(r, `${src}/beta/mod.rs`, 'pub mod one;\n'));
    const v = checkModuleSkeleton(skeleton, root);
    assert.ok(
      v.some((l) => /module beta::two is missing/.test(l)),
      JSON.stringify(v),
    );
  });

  it('detects an extra module in a closed nested group', () => {
    const root = tree((r, src) =>
      write(r, `${src}/beta/mod.rs`, 'pub mod one;\npub mod two;\npub mod three;\n'),
    );
    const v = checkModuleSkeleton(skeleton, root);
    assert.ok(
      v.some((l) => /extra module beta::three/.test(l)),
      JSON.stringify(v),
    );
  });

  it('allows extra modules inside an open group but still requires the listed ones', () => {
    const open = tree((r, src) =>
      write(r, `${src}/gamma/mod.rs`, 'pub mod inner;\npub mod extra;\n'),
    );
    assert.deepEqual(checkModuleSkeleton(skeleton, open), []);
    const missing = tree((r, src) => write(r, `${src}/gamma/mod.rs`, 'pub mod extra;\n'));
    const v = checkModuleSkeleton(skeleton, missing);
    assert.ok(
      v.some((l) => /module gamma::inner is missing/.test(l)),
      JSON.stringify(v),
    );
  });

  it('detects a declared module whose file is gone', () => {
    const root = tree((r, src) => fs.rmSync(path.join(r, src, 'alpha'), { recursive: true }));
    const v = checkModuleSkeleton(skeleton, root);
    assert.ok(
      v.some((l) => /module file for alpha not found/.test(l)),
      JSON.stringify(v),
    );
  });

  it('accepts a module converted to a sibling file', () => {
    const root = tree((r, src) => {
      fs.rmSync(path.join(r, src, 'alpha'), { recursive: true });
      write(r, `${src}/alpha.rs`);
    });
    assert.deepEqual(checkModuleSkeleton(skeleton, root), []);
  });

  it('detects a missing crate entry file', () => {
    const root = tree((r, src) => fs.rmSync(path.join(r, src, 'lib.rs')));
    const v = checkModuleSkeleton(skeleton, root);
    assert.ok(
      v.some((l) => /lib\.rs does not exist/.test(l)),
      JSON.stringify(v),
    );
  });

  it('detects a missing TypeScript module', () => {
    const root = tree((r) => fs.rmSync(path.join(r, 'packages/demo/src/a/index.ts')));
    const v = checkModuleSkeleton(skeleton, root);
    assert.ok(
      v.some((l) => /module src\/a\/index\.ts is missing/.test(l)),
      JSON.stringify(v),
    );
  });
});

describe('the committed skeleton', () => {
  it('matches the repository', () => {
    const recorded = JSON.parse(fs.readFileSync(path.join(here, 'module-skeleton.json'), 'utf8'));
    assert.deepEqual(checkModuleSkeleton(recorded), []);
  });
});
