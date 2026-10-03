import { describe, expect, it } from 'vitest';
import pkg from '../package.json';

describe('test-fixtures package', () => {
  it('is the private workspace package for UI fixtures', () => {
    expect(pkg.name).toBe('@vela/test-fixtures');
    expect(pkg.private).toBe(true);
  });
});
