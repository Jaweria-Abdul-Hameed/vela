// Intervention command group: Interventions: what needs a person, and resolving it.
// The Rust side is `apps/desktop/src-tauri/src/commands/intervention`; the contract test compares `commandNames` with the Rust registry.
// Wrappers take the Tauri `invoke` function as an argument so they are testable without a window.

/** Command names (dotted, as in the registry). Read by the Rust contract test: keep it a plain literal array. */
export const commandNames = ['intervention.list', 'intervention.resolve'] as const;

export type InvokeFn = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

/** `intervention.list`: Open interventions of a run. */
export async function interventionList<T = unknown>(invoke: InvokeFn, runId: string): Promise<T> {
  return (await invoke('intervention_list', { runId })) as T;
}

/** `intervention.resolve`: Resolves an intervention; the run continues. */
export async function interventionResolve(invoke: InvokeFn, interventionId: string, resolution: string): Promise<void> {
  await invoke('intervention_resolve', { interventionId, resolution });
}
