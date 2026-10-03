import { describe, expect, it } from 'vitest';

// Every renderer skeleton module (scenes, surfaces, state, ipc) must resolve and load. Tickets add
// their content later; the module list itself is checked by scripts/check-module-skeleton.mjs.
const modules = import.meta.glob(
  ['./scenes/*.tsx', './surfaces/*/index.ts', './state/index.ts', './ipc/index.ts'],
  { eager: true },
);

const expected = [
  ...['home', 'universe', 'focus', 'completion'].map((n) => `./scenes/${n}.tsx`),
  ...[
    'projects',
    'trust',
    'preflight',
    'analysis',
    'buildready',
    'inspector',
    'timeline',
    'evidence',
    'intervention',
    'runbar',
    'settings',
    'onboarding',
    'palette',
    'conflict',
  ].map((n) => `./surfaces/${n}/index.ts`),
  './state/index.ts',
  './ipc/index.ts',
];

describe('renderer module skeleton', () => {
  it('loads every skeleton module by name', () => {
    expect(Object.keys(modules)).toEqual(expect.arrayContaining(expected));
  });
});
