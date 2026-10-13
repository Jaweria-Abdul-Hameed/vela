// Trust command group: Repository trust: what trusting would make executable, grant with a confirmed profile, revoke.
// The Rust side is `apps/desktop/src-tauri/src/commands/trust`; the contract test compares `commandNames` with the Rust registry.
// Wrappers take the Tauri `invoke` function as an argument so they are testable without a window.

/** Command names (dotted, as in the registry). Read by the Rust contract test: keep it a plain literal array. */
export const commandNames = ['trust.propose', 'trust.grant', 'trust.revoke'] as const;

export type InvokeFn = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

/** `trust.propose`: What trusting would make executable. */
export async function trustPropose<T = unknown>(invoke: InvokeFn, projectId: string): Promise<T> {
  return (await invoke('trust_propose', { projectId })) as T;
}

/** `trust.grant`: Trusts the repository with the confirmed command profile. */
export async function trustGrant(
  invoke: InvokeFn,
  projectId: string,
  commands: { text: string; cwd?: string | null }[],
): Promise<void> {
  await invoke('trust_grant', { projectId, commands });
}

/** `trust.revoke`: Revokes trust. */
export async function trustRevoke(invoke: InvokeFn, projectId: string): Promise<void> {
  await invoke('trust_revoke', { projectId });
}
