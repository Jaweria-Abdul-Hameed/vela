// Project command group: Projects: import a repository folder, list, remove, and choose where its issues live.
// The Rust side is `apps/desktop/src-tauri/src/commands/project`; the contract test compares `commandNames` with the Rust registry.
// Wrappers take the Tauri `invoke` function as an argument so they are testable without a window.

/** Command names (dotted, as in the registry). Read by the Rust contract test: keep it a plain literal array. */
export const commandNames = [
  'project.import',
  'project.list',
  'project.remove',
  'project.set_tracker',
] as const;

export type InvokeFn = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

/** `project.import`: Imports the repository in `folder`. */
export async function projectImport<T = unknown>(invoke: InvokeFn, folder: string): Promise<T> {
  return (await invoke('project_import', { folder })) as T;
}

/** `project.list`: Recent projects. */
export async function projectList<T = unknown>(invoke: InvokeFn): Promise<T> {
  return (await invoke('project_list')) as T;
}

/** `project.remove`: Removes a project from Vela (never touches the repository). */
export async function projectRemove(invoke: InvokeFn, projectId: string): Promise<void> {
  await invoke('project_remove', { projectId });
}

/** `project.set_tracker`: Chooses the issue source: `local:<folder>` or `github:<owner>/<name>`. */
export async function projectSetTracker(
  invoke: InvokeFn,
  projectId: string,
  spec: string,
): Promise<void> {
  await invoke('project_set_tracker', { projectId, spec });
}
