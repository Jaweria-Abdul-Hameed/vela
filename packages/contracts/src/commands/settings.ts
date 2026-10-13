// Settings command group: Settings: read the effective settings and change one.
// The Rust side is `apps/desktop/src-tauri/src/commands/settings`; the contract test compares `commandNames` with the Rust registry.
// Wrappers take the Tauri `invoke` function as an argument so they are testable without a window.

/** Command names (dotted, as in the registry). Read by the Rust contract test: keep it a plain literal array. */
export const commandNames = ['settings.get', 'settings.set'] as const;

export type InvokeFn = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

/** `settings.get`: Effective settings. */
export async function settingsGet<T = unknown>(
  invoke: InvokeFn,
  projectId: string | null,
): Promise<T> {
  return (await invoke('settings_get', { projectId })) as T;
}

/** `settings.set`: Sets one setting. */
export async function settingsSet(
  invoke: InvokeFn,
  projectId: string | null,
  key: string,
  value: string,
): Promise<void> {
  await invoke('settings_set', { projectId, key, value });
}
