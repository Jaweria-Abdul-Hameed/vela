import { describe, expect, it } from 'vitest';
import pkg from '../package.json';

describe('contracts package', () => {
  it('exposes generated types through the wildcard export and no hand-edited barrel', () => {
    expect(pkg.exports).toEqual({ './generated/*': './src/generated/*.ts' });
    expect('main' in pkg).toBe(false);
  });
});
