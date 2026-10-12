// The renderer's view of the core (F05): a pure reducer over snapshots and journal events. It is deterministic (the same
// snapshot and stream always give the same state), ignores event kinds it does not know with a logged warning, and never
// applies an event twice. Feature tickets add behavior by dropping a slice into `slices/` (see `slices.ts`), never by
// editing this file.
import {
  eventKinds,
  type ProjectSummary,
  type SnapshotDto,
} from '@vela/contracts/commands/snapshot';
import type { EventEnvelope } from '@vela/contracts/generated/EventEnvelope';
import type { EventKind } from '@vela/contracts/generated/EventKind';

export type SyncStatus = 'booting' | 'ready' | 'resyncing';

/** How many recent envelopes the state keeps (a bounded tail for timelines and tests). */
export const RECENT_LIMIT = 200;

export interface CoreState {
  status: SyncStatus;
  /** The journal position the state reflects. */
  lastSeq: number;
  schemaVersion: number;
  projects: ProjectSummary[];
  /** How many events of each known kind were applied since the snapshot. */
  counts: Partial<Record<EventKind, number>>;
  recent: EventEnvelope[];
  /** Per-feature state owned by slices, keyed by slice name. */
  slices: Record<string, unknown>;
}

export const initialState: CoreState = {
  status: 'booting',
  lastSeq: 0,
  schemaVersion: 0,
  projects: [],
  counts: {},
  recent: [],
  slices: {},
};

/** A feature slice: the kinds it reacts to and how. */
export interface Slice {
  name: string;
  kinds: readonly EventKind[];
  reduce: (sliceState: unknown, event: EventEnvelope) => unknown;
}

export type Warn = (message: string) => void;

const known: ReadonlySet<string> = new Set(eventKinds);

/** Replaces everything with the snapshot (used at start and after a gap). */
export function replaceSnapshot(state: CoreState, snapshot: SnapshotDto): CoreState {
  return {
    ...state,
    lastSeq: snapshot.last_seq,
    schemaVersion: snapshot.schema_version,
    projects: snapshot.projects,
    counts: {},
    recent: [],
    slices: {},
  };
}

/**
 * Applies one journal event. Events at or before `lastSeq` are duplicates and change nothing. An unknown kind advances
 * `lastSeq` (so the stream stays contiguous) but has no other effect and is reported through `warn`.
 */
export function applyEnvelope(
  state: CoreState,
  envelope: EventEnvelope,
  slices: readonly Slice[] = [],
  warn: Warn = (message) => console.warn(message),
): CoreState {
  if (envelope.seq <= state.lastSeq) return state;
  if (!known.has(envelope.kind)) {
    warn(`ignoring unknown event kind "${String(envelope.kind)}" at seq ${envelope.seq}`);
    return { ...state, lastSeq: envelope.seq };
  }
  let nextSlices = state.slices;
  for (const slice of slices) {
    if (slice.kinds.includes(envelope.kind)) {
      nextSlices = { ...nextSlices, [slice.name]: slice.reduce(nextSlices[slice.name], envelope) };
    }
  }
  return {
    ...state,
    lastSeq: envelope.seq,
    counts: { ...state.counts, [envelope.kind]: (state.counts[envelope.kind] ?? 0) + 1 },
    recent: [...state.recent, envelope].slice(-RECENT_LIMIT),
    slices: nextSlices,
  };
}
