import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { validateAllowlist } from './check-audit-allowlist.mjs';

const today = new Date('2026-10-12T00:00:00Z');
const good = {
  id: 'RUSTSEC-0000-0000',
  scope: 'rust-advisory',
  justification: 'not reachable from Vela code paths',
  expires: '2027-01-01',
};

describe('audit allowlist', () => {
  it('accepts an empty list and a complete entry', () => {
    assert.deepEqual(validateAllowlist({ entries: [] }, today), []);
    assert.deepEqual(validateAllowlist({ entries: [good] }, today), []);
  });

  it('rejects an entry without a justification', () => {
    const rest = { ...good };
    delete rest.justification;
    assert.equal(validateAllowlist({ entries: [rest] }, today).length, 1);
    assert.equal(
      validateAllowlist({ entries: [{ ...good, justification: 'ok' }] }, today).length,
      1,
    );
    assert.equal(
      validateAllowlist({ entries: [{ ...good, justification: '   ' }] }, today).length,
      1,
    );
  });

  it('rejects an entry without an expiry, with a malformed one, or past its expiry', () => {
    const rest = { ...good };
    delete rest.expires;
    assert.equal(validateAllowlist({ entries: [rest] }, today).length, 1);
    assert.equal(
      validateAllowlist({ entries: [{ ...good, expires: 'next year' }] }, today).length,
      1,
    );
    assert.equal(
      validateAllowlist({ entries: [{ ...good, expires: '2026-10-11' }] }, today).length,
      1,
    );
    assert.equal(
      validateAllowlist({ entries: [{ ...good, expires: '2026-13-45' }] }, today).length,
      1,
    );
    assert.deepEqual(
      validateAllowlist({ entries: [{ ...good, expires: '2026-10-12' }] }, today),
      [],
    );
  });

  it('rejects a missing id, an unknown scope and malformed documents', () => {
    assert.equal(validateAllowlist({ entries: [{ ...good, id: '' }] }, today).length, 1);
    assert.equal(
      validateAllowlist({ entries: [{ ...good, scope: 'everything' }] }, today).length,
      1,
    );
    assert.equal(validateAllowlist({}, today).length, 1);
    assert.equal(validateAllowlist({ entries: [null] }, today).length, 1);
    assert.equal(validateAllowlist(null, today).length, 1);
  });
});
