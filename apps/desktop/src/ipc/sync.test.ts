import { describe, expect, it } from 'vitest';
import type { EventBatch, SnapshotDto } from '@vela/contracts/commands/snapshot';
import type { EventEnvelope } from '@vela/contracts/generated/EventEnvelope';
import { sampleStream } from '@vela/test-fixtures/streams/core/basic';
import {
  applyEnvelope,
  initialState,
  replaceSnapshot,
  type CoreState,
  type SyncStatus,
} from '../state/reducer';
import { startSync, type SyncDeps } from './sync';

/** A fake core: a journal that grows, a snapshot of it, and a channel to emit batches on. */
function fakeCore(initial: EventEnvelope[]) {
  const journal = [...initial];
  let handler: ((batch: EventBatch) => void) | undefined;
  let snapshotCalls = 0;
  const snapshotAt = (n: number): SnapshotDto => ({ last_seq: n, schema_version: 1, projects: [] });
  return {
    journal,
    snapshotCalls: () => snapshotCalls,
    getSnapshot: () => {
      snapshotCalls += 1;
      return Promise.resolve(snapshotAt(journal.length));
    },
    listen: (h: (batch: EventBatch) => void) => {
      handler = h;
      return Promise.resolve(() => {
        handler = undefined;
      });
    },
    append: (events: EventEnvelope[], emit = true) => {
      journal.push(...events);
      if (emit) handler?.({ events });
    },
    emit: (events: EventEnvelope[]) => handler?.({ events }),
  };
}

function fakeStore() {
  let state: CoreState = initialState;
  const statuses: SyncStatus[] = [];
  return {
    get: () => state,
    statuses,
    store: {
      lastSeq: () => state.lastSeq,
      applySnapshot: (s: SnapshotDto) => {
        state = replaceSnapshot(state, s);
      },
      applyEvent: (e: EventEnvelope) => {
        state = applyEnvelope(state, e, [], () => undefined);
      },
      setStatus: (s: SyncStatus) => {
        state = { ...state, status: s };
        statuses.push(s);
      },
    },
  };
}

function deps(core: ReturnType<typeof fakeCore>, store: ReturnType<typeof fakeStore>): SyncDeps {
  return { getSnapshot: core.getSnapshot, listen: core.listen, store: store.store };
}

const tick = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('sync client', () => {
  it('bootstraps from the snapshot and then applies streamed events in order', async () => {
    const core = fakeCore(sampleStream.slice(0, 2));
    const s = fakeStore();
    await startSync(deps(core, s));
    expect(s.get().lastSeq).toBe(2);
    expect(s.get().status).toBe('ready');
    core.append(sampleStream.slice(2, 4));
    core.append(sampleStream.slice(4, 6));
    expect(s.get().lastSeq).toBe(6);
  });

  it('reloading the renderer mid-stream ends in the same state as an uninterrupted one', async () => {
    const core = fakeCore(sampleStream.slice(0, 2));
    const before = fakeStore();
    const first = await startSync(deps(core, before));
    core.append(sampleStream.slice(2, 4));
    first.stop();
    // The renderer reloads while events 5 and 6 are produced; the new client starts from a fresh store.
    core.append(sampleStream.slice(4, 6), false);
    const after = fakeStore();
    await startSync(deps(core, after));
    const uninterrupted = fakeStore();
    await startSync(deps(fakeCore(sampleStream), uninterrupted));
    expect(after.get().lastSeq).toBe(6);
    expect(after.get().lastSeq).toBe(uninterrupted.get().lastSeq);
    expect(after.get().status).toBe('ready');
  });

  it('a forced gap triggers a snapshot refetch', async () => {
    const core = fakeCore(sampleStream.slice(0, 2));
    const s = fakeStore();
    const client = await startSync(deps(core, s));
    expect(core.snapshotCalls()).toBe(1);
    // Events 3 and 4 are committed but never delivered; event 5 arrives.
    core.append(sampleStream.slice(2, 4), false);
    core.append(sampleStream.slice(4, 5));
    await tick();
    expect(core.snapshotCalls()).toBe(2);
    expect(s.get().lastSeq).toBe(5);
    expect(s.get().status).toBe('ready');
    expect(s.statuses).toContain('resyncing');
    expect(client.resyncCount()).toBe(2);
  });

  it('events that arrive while the snapshot is in flight are buffered and replayed after it', async () => {
    const core = fakeCore(sampleStream.slice(0, 2));
    const s = fakeStore();
    let release: (() => void) | undefined;
    const slow: SyncDeps = {
      ...deps(core, s),
      getSnapshot: async () => {
        const snapshot = await core.getSnapshot();
        await new Promise<void>((resolve) => {
          release = resolve;
        });
        return snapshot;
      },
    };
    const started = startSync(slow);
    await tick();
    // The snapshot was read at 2; events 3 and 4 are committed and emitted before it is applied.
    core.append(sampleStream.slice(2, 4));
    release?.();
    await started;
    expect(s.get().lastSeq).toBe(4);
  });

  it('a burst of 1000 events delivered in small batches loses nothing', async () => {
    const make = (seq: number): EventEnvelope => ({ ...sampleStream[3]!, seq });
    const core = fakeCore([]);
    const s = fakeStore();
    await startSync(deps(core, s));
    for (let i = 0; i < 1000; i += 25) {
      core.append(Array.from({ length: 25 }, (_, k) => make(i + k + 1)));
    }
    expect(s.get().lastSeq).toBe(1000);
    expect(core.snapshotCalls()).toBe(1);
  });

  it('stops listening when stopped', async () => {
    const core = fakeCore([]);
    const s = fakeStore();
    const client = await startSync(deps(core, s));
    client.stop();
    core.append([sampleStream[0]!]);
    expect(s.get().lastSeq).toBe(0);
  });
});
