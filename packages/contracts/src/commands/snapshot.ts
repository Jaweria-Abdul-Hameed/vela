// Snapshot command group (F08): `snapshot.get` plus the single event channel. The Rust side is
// `apps/desktop/src-tauri/src/commands/snapshot` and `src/events`; the contract test compares `commandNames` with the
// Rust registry. Like every wrapper it takes the Tauri `invoke`/`listen` functions as arguments so it is testable
// without a window.
import type { EventEnvelope } from '../generated/EventEnvelope';
import type { EventKind } from '../generated/EventKind';

/** Command names (dotted, as in the registry). Read by the Rust contract test: keep it a plain literal array. */
export const commandNames = ['snapshot.get'] as const;

/** The one channel every journal event travels on. */
export const EVENTS_CHANNEL = 'vela:events';

/** Every event kind the core can emit (the closed vocabulary); a test keeps it equal to the generated `EventKind`. */
export const eventKinds = [
  'RunCreated',
  'PreflightCompleted',
  'GraphBuilt',
  'TicketReady',
  'WorkerStarted',
  'CommandStarted',
  'CommandFinished',
  'TestGateFinished',
  'CheckpointCommitted',
  'ReviewStarted',
  'ReviewFindingRaised',
  'ReviewFindingResolved',
  'WorkerCompleted',
  'MergeStarted',
  'MergeCompleted',
  'IntegrationGateFailed',
  'HumanActionRequired',
  'RunPaused',
  'RunResumed',
  'RunCompleted',
  'ProjectTrusted',
  'ProjectTrustRevoked',
  'OnboardingChoiceRecorded',
  'ApprovalAutomationConsentChanged',
  'ApprovalDetected',
  'ApprovalClassified',
  'ApprovalDeliveryAttempted',
  'ApprovalDeliveryVerified',
  'ApprovalStalled',
  'PolicyViolationDetected',
  'StopRequested',
  'RunStopping',
  'WorkerCancelled',
  'WorkerFailed',
  'WorkerPaused',
  'WorkerResumed',
  'ReconciliationStarted',
  'ReconciliationCompleted',
  'DiscrepancyDetected',
  'PushCompleted',
  'PushFailed',
  'MergeDiscarded',
  'ConflictResolutionStarted',
  'WorktreeProvisioned',
  'WorktreeRemoved',
  'ProfileCooldownStarted',
  'ProfileAvailable',
  'ProviderPolicyBlockRecorded',
  'ProviderPolicyBlockCleared',
  'PromotionCompleted',
  'StateStoreBackupCreated',
  'MigrationApplied',
  'MigrationFailed',
] as const satisfies readonly EventKind[];

export interface ProjectSummary {
  id: string;
  name: string;
  repo_path: string;
  last_opened_ms: number | null;
}

export interface SnapshotDto {
  /** The journal position the snapshot reflects. */
  last_seq: number;
  schema_version: number;
  projects: ProjectSummary[];
}

export interface EventBatch {
  events: EventEnvelope[];
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

function isProject(value: unknown): value is ProjectSummary {
  return (
    isRecord(value) &&
    typeof value.id === 'string' &&
    typeof value.name === 'string' &&
    typeof value.repo_path === 'string' &&
    (value.last_opened_ms === null || typeof value.last_opened_ms === 'number')
  );
}

function isSnapshot(value: unknown): value is SnapshotDto {
  return (
    isRecord(value) &&
    typeof value.last_seq === 'number' &&
    typeof value.schema_version === 'number' &&
    Array.isArray(value.projects) &&
    value.projects.every(isProject)
  );
}

function isEnvelope(value: unknown): value is EventEnvelope {
  return (
    isRecord(value) &&
    typeof value.seq === 'number' &&
    typeof value.kind === 'string' &&
    typeof value.schema_version === 'number'
  );
}

/** Calls `snapshot.get`. A rejected promise carries the typed `VelaError` the core returned. */
export async function getSnapshot(invoke: InvokeFn): Promise<SnapshotDto> {
  const result = await invoke(wireName('snapshot.get'));
  if (!isSnapshot(result)) {
    throw new Error('snapshot.get returned a result that does not match the contract');
  }
  return result;
}

/** Subscribes to the event channel. Malformed batches and envelopes are dropped. Resolves to the unlisten function. */
export function listenEvents(
  listen: ListenFn,
  handler: (batch: EventBatch) => void,
): Promise<() => void> {
  return listen(EVENTS_CHANNEL, (event) => {
    if (!isRecord(event.payload) || !Array.isArray(event.payload.events)) return;
    const events = event.payload.events.filter(isEnvelope);
    if (events.length > 0) handler({ events });
  });
}
