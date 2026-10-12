// Typed IPC bindings (F04): binds the contract wrappers in packages/contracts to the real Tauri functions.
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  listenPing,
  ping,
  type PingEvent,
  type PingRequest,
  type PingResult,
} from '@vela/contracts/commands/diagnostics';

/** Calls the core's `diagnostics.ping` command. */
export function pingCore(request: PingRequest): Promise<PingResult> {
  return ping(invoke, request);
}

/** Subscribes to the core's ping event; resolves to the unlisten function. */
export function onPingEvent(handler: (event: PingEvent) => void): Promise<() => void> {
  return listenPing(listen, handler);
}
