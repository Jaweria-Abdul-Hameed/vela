// Run command group: Runs: prepare (analyze the issues), build, stop, resume and status.
// The Rust side is `apps/desktop/src-tauri/src/commands/run`; the contract test compares `commandNames` with the Rust registry.
// Wrappers take the Tauri `invoke` function as an argument so they are testable without a window.

/** Command names (dotted, as in the registry). Read by the Rust contract test: keep it a plain literal array. */
export const commandNames = ['run.prepare', 'run.build', 'run.stop', 'run.resume', 'run.status'] as const;

export type InvokeFn = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

/** `run.prepare`: Creates a run and analyzes the issues into a graph. */
export async function runPrepare<T = unknown>(invoke: InvokeFn, projectId: string): Promise<T> {
  return (await invoke('run_prepare', { projectId })) as T;
}

/** `run.build`: Approves the graph and starts building. */
export async function runBuild(invoke: InvokeFn, runId: string, expectedBase: string | null, acknowledge: boolean): Promise<void> {
  await invoke('run_build', { runId, expectedBase, acknowledge });
}

/** `run.stop`: Stop All. */
export async function runStop<T = unknown>(invoke: InvokeFn, runId: string): Promise<T> {
  return (await invoke('run_stop', { runId })) as T;
}

/** `run.resume`: Reconciles, then resumes. */
export async function runResume<T = unknown>(invoke: InvokeFn, runId: string): Promise<T> {
  return (await invoke('run_resume', { runId })) as T;
}

/** `run.status`: Everything the run screen shows. */
export async function runStatus<T = unknown>(invoke: InvokeFn, runId: string): Promise<T> {
  return (await invoke('run_status', { runId })) as T;
}
