import { describe, expect, it } from 'vitest';

// Every UI package skeleton module must resolve and load. Tickets add their content later; the module
// list itself is checked by scripts/check-module-skeleton.mjs.
const modules = import.meta.glob(
  [
    './design/index.ts',
    './components/index.ts',
    './canvas/*/index.ts',
    './canvas/quality.ts',
    './a11y/index.ts',
    './safe-content/index.ts',
  ],
  { eager: true },
);

const expected = [
  './design/index.ts',
  './components/index.ts',
  ...['ambient', 'graph', 'effects', 'focus', 'fallback'].map((n) => `./canvas/${n}/index.ts`),
  './canvas/quality.ts',
  './a11y/index.ts',
  './safe-content/index.ts',
];

describe('ui module skeleton', () => {
  it('loads every skeleton module by name', () => {
    expect(Object.keys(modules)).toEqual(expect.arrayContaining(expected));
  });
});
