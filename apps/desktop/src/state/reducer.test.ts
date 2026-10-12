import { describe, expect, it, vi } from 'vitest';
import {
  sampleSnapshot,
  sampleStream,
  streamAfterSnapshot,
} from '@vela/test-fixtures/streams/core/basic';
import type { EventEnvelope } from '@vela/contracts/generated/EventEnvelope';
import { collectSlices } from './slice-registry';
import {
  applyEnvelope,
  initialState,
  RECENT_LIMIT,
  replaceSnapshot,
  type CoreState,
  type Slice,
} from './reducer';

function run(stream: EventEnvelope[], slices: Slice[] = []): CoreState {
  let state = replaceSnapshot(initialState, sampleSnapshot);
  for (const event of stream) state = applyEnvelope(state, event, slices, () => undefined);
  return state;
}

describe('reducer', () => {
  it('applies a recorded stream deterministically', () => {
    const a = run(streamAfterSnapshot);
    const b = run(streamAfterSnapshot);
    expect(a).toEqual(b);
    expect(a.lastSeq).toBe(6);
    expect(a.counts).toEqual({ GraphBuilt: 1, TicketReady: 3 });
    expect(a.projects).toEqual(sampleSnapshot.projects);
  });

  it('ignores duplicates and events at or before the snapshot position', () => {
    const state = run(sampleStream);
    expect(state.lastSeq).toBe(6);
    expect(state.recent.map((e) => e.seq)).toEqual([3, 4, 5, 6]);
    const again = applyEnvelope(state, sampleStream[5]!, [], () => undefined);
    expect(again).toBe(state);
  });

  it('ignores an unknown event kind with a logged warning but keeps the stream contiguous', () => {
    const warn = vi.fn();
    const before = replaceSnapshot(initialState, sampleSnapshot);
    const alien = {
      ...sampleStream[2]!,
      seq: 3,
      kind: 'FromTheFuture',
      payload: {},
    } as unknown as EventEnvelope;
    const after = applyEnvelope(before, alien, [], warn);
    expect(warn).toHaveBeenCalledTimes(1);
    expect(warn.mock.calls[0]?.[0]).toContain('FromTheFuture');
    expect(after.lastSeq).toBe(3);
    expect(after.counts).toEqual({});
    expect(after.recent).toEqual([]);
  });

  it('routes events to registered slices only for the kinds they declare', () => {
    const slice: Slice = {
      name: 'ready',
      kinds: ['TicketReady'],
      reduce: (s, e) => [...((s as number[] | undefined) ?? []), e.seq],
    };
    const state = run(streamAfterSnapshot, [slice]);
    expect(state.slices).toEqual({ ready: [4, 5, 6] });
  });

  it('keeps only a bounded tail of recent events', () => {
    let state = replaceSnapshot(initialState, { ...sampleSnapshot, last_seq: 0 });
    for (let seq = 1; seq <= RECENT_LIMIT + 50; seq += 1) {
      state = applyEnvelope(state, { ...sampleStream[3]!, seq }, [], () => undefined);
    }
    expect(state.recent).toHaveLength(RECENT_LIMIT);
    expect(state.recent[0]?.seq).toBe(51);
  });

  it('discovers slices from a module map and skips modules without one', () => {
    const slice: Slice = { name: 'x', kinds: [], reduce: (s) => s };
    expect(
      collectSlices({
        './slices/b.ts': { slice },
        './slices/a.ts': {},
        './slices/c.ts': undefined,
      }),
    ).toEqual([slice]);
  });
});
