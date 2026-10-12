// Typed IPC bindings (F04, F08): binds the contract wrappers in packages/contracts to the real Tauri functions.
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  listenPing,
  ping,
  type PingEvent,
  type PingRequest,
  type PingResult,
} from '@vela/contracts/commands/diagnostics';
import {
  getSnapshot,
  listenEvents,
  type EventBatch,
  type SnapshotDto,
} from '@vela/contracts/commands/snapshot';

export { startSync, type SyncClient, type SyncDeps } from './sync';

/** Calls the core's `diagnostics.ping` command. */
export function pingCore(request: PingRequest): Promise<PingResult> {
  return ping(invoke, request);
}

/** Subscribes to the core's ping event; resolves to the unlisten function. */
export function onPingEvent(handler: (event: PingEvent) => void): Promise<() => void> {
  return listenPing(listen, handler);
}

/** Calls the core's `snapshot.get` command. */
export function snapshotCore(): Promise<SnapshotDto> {
  return getSnapshot(invoke);
}

/** Subscribes to the core's journal event channel; resolves to the unlisten function. */
export function onCoreEvents(handler: (batch: EventBatch) => void): Promise<() => void> {
  return listenEvents(listen, handler);
}
