// Preflight command group: Preflight: a PASS/WARN/BLOCK report for a project.
// The Rust side is `apps/desktop/src-tauri/src/commands/preflight`; the contract test compares `commandNames` with the Rust registry.
// Wrappers take the Tauri `invoke` function as an argument so they are testable without a window.

/** Command names (dotted, as in the registry). Read by the Rust contract test: keep it a plain literal array. */
export const commandNames = ['preflight.run'] as const;

export type InvokeFn = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

/** `preflight.run`: Runs preflight. */
export async function preflightRun<T = unknown>(invoke: InvokeFn, projectId: string): Promise<T> {
  return (await invoke('preflight_run', { projectId })) as T;
}
