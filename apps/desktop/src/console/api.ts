// The console's view of the core: one object of typed calls, bound to the real Tauri `invoke` here and replaced by a fake in
// tests. Every method is a thin wrapper over a contract in `@vela/contracts`.
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import * as intervention from '@vela/contracts/commands/intervention';
import * as preflight from '@vela/contracts/commands/preflight';
import * as project from '@vela/contracts/commands/project';
import * as run from '@vela/contracts/commands/run';
import * as settings from '@vela/contracts/commands/settings';
import * as timeline from '@vela/contracts/commands/timeline';
import * as trust from '@vela/contracts/commands/trust';

export interface Project {
  id: string;
  name: string;
  repo_path: string;
  default_branch: string | null;
  head_at_import: string | null;
  last_opened_ms: number | null;
}

export interface TrustProposal {
  state: 'Trusted' | 'Untrusted';
  commands: { text: string; cwd: string | null }[];
  executable_surfaces: string[];
  consent_text_version: string;
}

export interface PreflightCheck {
  id: string;
  title: string;
  severity: 'Pass' | 'Warn' | 'Block';
  detail: string;
  remediation: string | null;
}

export interface PreflightReport {
  verdict: 'Pass' | 'Warn' | 'Block';
  checks: PreflightCheck[];
}

export interface Prepared {
  run_id: string;
  valid: boolean;
  tickets?: number;
  edges?: number;
  layers?: number;
  analyzed?: boolean;
  warnings?: string[];
  cycles?: string[];
}

export interface GraphView {
  nodes: { id: string; x: number; y: number }[];
  edges: { from: string; to: string }[];
  pairs: { a: string; b: string; label: string; reasons: string[] }[];
  analyzed: boolean;
  warnings: string[];
}

export interface WorkerView {
  id: string;
  ticket: string;
  /** A phase name, or an object for paused and human-gated states. */
  state: unknown;
  branch: string | null;
  worktree: string | null;
}

export interface InterventionRecord {
  id: string;
  run_id: string;
  worker_id: string | null;
  kind: string;
  summary: string;
  evidence: string[] | null;
  preserved_work: string;
  last_safe_commit: string | null;
  consequences: string;
}

export interface RunStatus {
  run_id: string;
  /** A state name, or an object keyed by the state name. */
  state: unknown;
  graph: GraphView | null;
  base: { sha: string; branch: string; integration_branch: string } | null;
  workers: WorkerView[];
  interventions: InterventionRecord[];
  promotion: { mode: string; state: string; pr_url: string | null; note: string } | null;
}

export interface TimelineEntry {
  seq: number;
  ts: number;
  kind: string;
}

/** What the shell reports while a folder is dragged over the window. */
export type FolderDrop = { type: 'over' } | { type: 'leave' } | { type: 'drop'; paths: string[] };

export interface ConsoleApi {
  /** Subscribes to folder drags and drops; resolves to the unsubscribe function. Absent in a plain browser. */
  onFolderDrop?: (handler: (e: FolderDrop) => void) => Promise<() => void>;
  projectList: () => Promise<Project[]>;
  projectImport: (folder: string) => Promise<{ project: Project; already_known: boolean }>;
  projectRemove: (projectId: string) => Promise<void>;
  projectSetTracker: (projectId: string, spec: string) => Promise<void>;
  trustPropose: (projectId: string) => Promise<TrustProposal>;
  trustGrant: (
    projectId: string,
    commands: { text: string; cwd?: string | null }[],
  ) => Promise<void>;
  trustRevoke: (projectId: string) => Promise<void>;
  preflightRun: (projectId: string) => Promise<PreflightReport>;
  runPrepare: (projectId: string) => Promise<Prepared>;
  runBuild: (runId: string) => Promise<void>;
  runStop: (runId: string) => Promise<unknown>;
  runResume: (runId: string) => Promise<unknown>;
  runStatus: (runId: string) => Promise<RunStatus>;
  interventionResolve: (interventionId: string, resolution: string) => Promise<void>;
  timelineGet: (runId: string, ticket: string) => Promise<TimelineEntry[]>;
  settingsGet: (projectId: string | null) => Promise<Record<string, string>>;
  settingsSet: (projectId: string | null, key: string, value: string) => Promise<void>;
}

const call = invoke as (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

/** The real API over Tauri. */
export const tauriApi: ConsoleApi = {
  onFolderDrop: async (handler) => {
    const offs = await Promise.all([
      listen('tauri://drag-enter', () => handler({ type: 'over' })),
      listen('tauri://drag-leave', () => handler({ type: 'leave' })),
      listen<{ paths: string[] }>('tauri://drag-drop', (e) =>
        handler({ type: 'drop', paths: e.payload.paths }),
      ),
    ]);
    return () => offs.forEach((off) => off());
  },
  projectList: () => project.projectList<Project[]>(call),
  projectImport: (folder) => project.projectImport(call, folder),
  projectRemove: (id) => project.projectRemove(call, id),
  projectSetTracker: (id, spec) => project.projectSetTracker(call, id, spec),
  trustPropose: (id) => trust.trustPropose<TrustProposal>(call, id),
  trustGrant: (id, commands) => trust.trustGrant(call, id, commands),
  trustRevoke: (id) => trust.trustRevoke(call, id),
  preflightRun: (id) => preflight.preflightRun<PreflightReport>(call, id),
  runPrepare: (id) => run.runPrepare<Prepared>(call, id),
  runBuild: (runId) => run.runBuild(call, runId, null, false),
  runStop: (runId) => run.runStop(call, runId),
  runResume: (runId) => run.runResume(call, runId),
  runStatus: (runId) => run.runStatus<RunStatus>(call, runId),
  interventionResolve: (id, resolution) => intervention.interventionResolve(call, id, resolution),
  timelineGet: (runId, ticket) => timeline.timelineGet<TimelineEntry[]>(call, runId, ticket),
  settingsGet: (id) => settings.settingsGet<Record<string, string>>(call, id),
  settingsSet: (id, key, value) => settings.settingsSet(call, id, key, value),
};
