import { describe, expect, it } from 'vitest';

// Every renderer skeleton module (scenes, surfaces, state, ipc) must resolve and load. Tickets add
// their content later; the module list itself is checked by scripts/check-module-skeleton.mjs.
const modules = import.meta.glob(
  ['./scenes/*.tsx', './surfaces/*/index.ts', './state/index.ts', './ipc/index.ts'],
  { eager: true },
);

describe('renderer module skeleton', () => {
  it('loads every skeleton module', () => {
    expect(Object.keys(modules).length).toBeGreaterThanOrEqual(20);
  });
});
