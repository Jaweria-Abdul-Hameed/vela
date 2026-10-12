// The renderer store (F05): zustand over the pure reducer. Components read it; only the sync client writes to it.
import { create } from 'zustand';
import type { SnapshotDto } from '@vela/contracts/commands/snapshot';
import type { EventEnvelope } from '@vela/contracts/generated/EventEnvelope';
import {
  applyEnvelope,
  initialState,
  replaceSnapshot,
  type CoreState,
  type SyncStatus,
} from './reducer';
import { registeredSlices } from './slice-registry';

export interface CoreStore extends CoreState {
  applySnapshot: (snapshot: SnapshotDto) => void;
  applyEvent: (envelope: EventEnvelope) => void;
  setStatus: (status: SyncStatus) => void;
}

export const useCoreStore = create<CoreStore>()((set) => ({
  ...initialState,
  applySnapshot: (snapshot) => set((state) => replaceSnapshot(state, snapshot)),
  applyEvent: (envelope) => set((state) => applyEnvelope(state, envelope, registeredSlices)),
  setStatus: (status) => set({ status }),
}));
