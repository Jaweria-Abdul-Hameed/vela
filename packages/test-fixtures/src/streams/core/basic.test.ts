import { describe, expect, it } from 'vitest';
import { eventKinds } from '@vela/contracts/commands/snapshot';
import { sampleSnapshot, sampleStream, streamAfterSnapshot } from './basic';

describe('core/basic fixture', () => {
  it('is a contiguous stream of known kinds', () => {
    expect(sampleStream.map((e) => e.seq)).toEqual([1, 2, 3, 4, 5, 6]);
    for (const e of sampleStream) expect(eventKinds).toContain(e.kind);
  });

  it('splits at the snapshot position', () => {
    expect(sampleSnapshot.last_seq).toBe(2);
    expect(streamAfterSnapshot.map((e) => e.seq)).toEqual([3, 4, 5, 6]);
  });
});
