// Diagnostics command group (F04). Hand-written wrapper for the example `diagnostics.ping` command and its event.
// The Rust side is `apps/desktop/src-tauri/src/commands/diagnostics`; the contract test compares `commandNames`
// with the Rust registry. The wrapper takes the Tauri `invoke`/`listen` functions as arguments so it can be tested
// without a window; `apps/desktop/src/ipc` binds the real ones.

/** Command names (dotted, as in the registry). Read by the Rust contract test: keep it a plain literal array. */
export const commandNames = ['diagnostics.ping'] as const;

/** The single event channel the example event is emitted on. */
export const PING_EVENT = 'vela:ping';

export interface PingRequest {
  message: string;
}

export interface PingResult {
  reply: string;
  message: string;
}

export interface PingEvent {
  message: string;
  sequence: number;
}

export type InvokeFn = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
export type ListenFn = (
  event: string,
  handler: (event: { payload: unknown }) => void,
) => Promise<() => void>;

/** The IPC command name for a dotted registry name: Tauri commands are Rust function names. */
export function wireName(name: string): string {
  return name.replaceAll('.', '_');
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function isPingResult(value: unknown): value is PingResult {
  return isRecord(value) && typeof value.reply === 'string' && typeof value.message === 'string';
}

function isPingEvent(value: unknown): value is PingEvent {
  return isRecord(value) && typeof value.message === 'string' && typeof value.sequence === 'number';
}

/** Calls `diagnostics.ping`. A rejected promise carries the typed `VelaError` the core returned. */
export async function ping(invoke: InvokeFn, request: PingRequest): Promise<PingResult> {
  const result = await invoke(wireName('diagnostics.ping'), { request });
  if (!isPingResult(result)) {
    throw new Error('diagnostics.ping returned a result that does not match the contract');
  }
  return result;
}

/** Subscribes to the ping event. Malformed payloads are dropped. Resolves to the unlisten function. */
export function listenPing(
  listen: ListenFn,
  handler: (event: PingEvent) => void,
): Promise<() => void> {
  return listen(PING_EVENT, (event) => {
    if (isPingEvent(event.payload)) handler(event.payload);
  });
}
