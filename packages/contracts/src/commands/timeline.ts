// Timeline command group: Timeline: the journal of one ticket.
// The Rust side is `apps/desktop/src-tauri/src/commands/timeline`; the contract test compares `commandNames` with the Rust registry.
// Wrappers take the Tauri `invoke` function as an argument so they are testable without a window.

/** Command names (dotted, as in the registry). Read by the Rust contract test: keep it a plain literal array. */
export const commandNames = ['timeline.get'] as const;

export type InvokeFn = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

/** `timeline.get`: A ticket timeline. */
export async function timelineGet<T = unknown>(invoke: InvokeFn, runId: string, ticket: string): Promise<T> {
  return (await invoke('timeline_get', { runId, ticket })) as T;
}
