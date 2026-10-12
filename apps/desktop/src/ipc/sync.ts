// The renderer's synchronization with the core (F08): bootstrap, gap detection and resynchronization.
//
// The client subscribes to the event channel first, then fetches the snapshot, so no event can fall between the two.
// Events that arrive while a snapshot is being fetched are buffered and replayed after it (those at or before the
// snapshot position are dropped). A gap in `seq` while running triggers a snapshot refetch. Reloading the renderer is just
// starting a new client against a fresh store: the result is the same state.
import type { EventBatch, SnapshotDto } from '@vela/contracts/commands/snapshot';
import type { EventEnvelope } from '@vela/contracts/generated/EventEnvelope';
import type { SyncStatus } from '../state/reducer';

/** What the client needs from the outside: the core, and the store it keeps in sync. */
export interface SyncDeps {
  getSnapshot: () => Promise<SnapshotDto>;
  listen: (handler: (batch: EventBatch) => void) => Promise<() => void>;
  store: {
    lastSeq: () => number;
    applySnapshot: (snapshot: SnapshotDto) => void;
    applyEvent: (envelope: EventEnvelope) => void;
    setStatus: (status: SyncStatus) => void;
  };
  /** How many times a resync may repeat when new gaps appear while replaying. Defaults to 3. */
  maxResyncAttempts?: number;
}

export interface SyncClient {
  /** Stops listening. */
  stop: () => void;
  /** Refetches the snapshot and replays buffered events (also called automatically on a gap). */
  resync: () => Promise<void>;
  /** How many resyncs ran (the initial bootstrap counts as one). */
  resyncCount: () => number;
}

export async function startSync(deps: SyncDeps): Promise<SyncClient> {
  const { store } = deps;
  const maxAttempts = deps.maxResyncAttempts ?? 3;
  let buffer: EventEnvelope[] = [];
  let busy = false;
  let resyncs = 0;
  let stopped = false;

  /** Applies events in order; returns false at the first gap. Duplicates are skipped. */
  function replay(events: EventEnvelope[]): boolean {
    for (const event of [...events].sort((a, b) => a.seq - b.seq)) {
      const last = store.lastSeq();
      if (event.seq <= last) continue;
      if (event.seq !== last + 1) return false;
      store.applyEvent(event);
    }
    return true;
  }

  async function resync(): Promise<void> {
    if (busy) return;
    busy = true;
    try {
      store.setStatus(resyncs === 0 ? 'booting' : 'resyncing');
      for (let attempt = 0; attempt < maxAttempts && !stopped; attempt += 1) {
        resyncs += 1;
        // Events that arrive from now until the snapshot is applied are kept and replayed after it.
        buffer = [];
        const snapshot = await deps.getSnapshot();
        store.applySnapshot(snapshot);
        const pending = buffer;
        buffer = [];
        if (replay(pending)) {
          store.setStatus('ready');
          return;
        }
      }
      // Still gapped after the allowed attempts: the next batch retriggers a resync.
      store.setStatus('resyncing');
    } finally {
      busy = false;
    }
  }

  const unlisten = await deps.listen((batch) => {
    if (stopped) return;
    if (busy) {
      buffer.push(...batch.events);
      return;
    }
    // A gap: the events that exposed it were committed before the snapshot below is read, so the snapshot covers them.
    if (!replay(batch.events)) void resync();
  });

  await resync();
  return {
    stop: () => {
      stopped = true;
      unlisten();
    },
    resync,
    resyncCount: () => resyncs,
  };
}
