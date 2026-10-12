import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
  commandNames,
  eventKinds,
  EVENTS_CHANNEL,
  getSnapshot,
  listenEvents,
  wireName,
  type SnapshotDto,
} from './snapshot';

const snapshot: SnapshotDto = { last_seq: 4, schema_version: 1, projects: [] };

describe('snapshot command wrappers', () => {
  it('declares the command and invokes its wire name', async () => {
    expect(commandNames).toEqual(['snapshot.get']);
    const calls: string[] = [];
    const result = await getSnapshot((cmd) => {
      calls.push(cmd);
      return Promise.resolve(snapshot);
    });
    expect(result).toEqual(snapshot);
    expect(calls).toEqual([wireName('snapshot.get')]);
  });

  it('rejects a result that does not match the contract', async () => {
    await expect(getSnapshot(() => Promise.resolve({ last_seq: 'x' }))).rejects.toThrow(
      /does not match the contract/,
    );
    await expect(
      getSnapshot(() => Promise.resolve({ ...snapshot, projects: [{ id: 1 }] })),
    ).rejects.toThrow();
  });

  it('listens on the single event channel and drops malformed batches and envelopes', async () => {
    let deliver: ((event: { payload: unknown }) => void) | undefined;
    const received: number[][] = [];
    const stop = await listenEvents(
      (channel, handler) => {
        expect(channel).toBe(EVENTS_CHANNEL);
        deliver = handler;
        return Promise.resolve(() => undefined);
      },
      (batch) => received.push(batch.events.map((e) => e.seq)),
    );
    deliver?.({ payload: null });
    deliver?.({ payload: { events: 'nope' } });
    deliver?.({ payload: { events: [{ seq: 'x' }] } });
    deliver?.({
      payload: { events: [{ seq: 1, kind: 'TicketReady', schema_version: 1 }, { nonsense: true }] },
    });
    expect(received).toEqual([[1]]);
    stop();
  });

  it('keeps the event kind list equal to the generated EventKind vocabulary', () => {
    const generated = readFileSync(new URL('../generated/EventKind.ts', import.meta.url), 'utf8');
    const union = /export type EventKind = ([^;]+);/s.exec(generated)?.[1] ?? '';
    const kinds = [...union.matchAll(/"(\w+)"/g)].map((m) => m[1]);
    expect([...eventKinds]).toEqual(kinds);
  });
});
