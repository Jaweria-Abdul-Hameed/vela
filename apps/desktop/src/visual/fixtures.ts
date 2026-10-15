/** Fixture streams for the visual-regression suite (U18). Each state is a fixed `RunStatus`, so a screenshot depends only on the
 *  renderer and never on timing, a core or a repository. The pages that show them are `visual.html?state=<name>`. */
import type { ConsoleApi, Project, RunStatus } from '../console/api';

export const STATES = [
  'home',
  'active-graph',
  'worker-detail',
  'conflict-warning',
  'needs-human',
  'completed',
  'reduced-motion',
] as const;

export type VisualState = (typeof STATES)[number];

export function isVisualState(value: string | null): value is VisualState {
  return value !== null && (STATES as readonly string[]).includes(value);
}

const demo: Project = {
  id: 'p1',
  name: 'demo-app',
  repo_path: 'C:/work/demo-app',
  default_branch: 'main',
  head_at_import: 'a'.repeat(40),
  last_opened_ms: 1_700_000_000_000,
};

const nodes = [
  { id: 'T-1', x: 0, y: 0 },
  { id: 'T-2', x: 220, y: -90 },
  { id: 'T-3', x: 220, y: 90 },
  { id: 'T-4', x: 440, y: 0 },
  { id: 'T-5', x: 660, y: -60 },
  { id: 'T-6', x: 660, y: 60 },
];

const edges = [
  { from: 'T-1', to: 'T-2' },
  { from: 'T-1', to: 'T-3' },
  { from: 'T-2', to: 'T-4' },
  { from: 'T-3', to: 'T-4' },
  { from: 'T-4', to: 'T-5' },
  { from: 'T-4', to: 'T-6' },
];

const worker = (ticket: string, state: unknown) => ({
  id: `w-${ticket.toLowerCase()}`,
  ticket,
  state,
  branch: `vela/run-1/${ticket.toLowerCase()}`,
  worktree: `C:/work/.vela/${ticket.toLowerCase()}`,
});

function base(over: Partial<RunStatus>): RunStatus {
  return {
    run_id: 'run-1',
    state: { ACTIVE: 'RUNNING' },
    graph: { nodes, edges, pairs: [], analyzed: true, warnings: [] },
    base: { sha: 'b'.repeat(40), branch: 'main', integration_branch: 'vela/run-1/integration' },
    workers: [],
    interventions: [],
    promotion: null,
    ...over,
  };
}

export function statusFor(state: VisualState): RunStatus | null {
  switch (state) {
    case 'home':
      return null;
    case 'active-graph':
    case 'worker-detail':
    case 'reduced-motion':
      return base({
        workers: [
          worker('T-1', 'COMPLETE'),
          worker('T-2', { ACTIVE: 'IMPLEMENTING' }),
          worker('T-3', { ACTIVE: 'FOCUSED_VALIDATION' }),
        ],
      });
    case 'conflict-warning':
      return base({
        state: { ACTIVE: 'GRAPH_READY' },
        graph: {
          nodes,
          edges,
          analyzed: true,
          warnings: [],
          pairs: [
            {
              a: 'T-2',
              b: 'T-3',
              label: 'SERIALIZED_BY_EVIDENCE',
              reasons: ['both change db/schema.sql'],
            },
            {
              a: 'T-5',
              b: 'T-6',
              label: 'UNKNOWN_SEQUENTIAL',
              reasons: ['no prediction for T-6'],
            },
          ],
        },
        base: null,
      });
    case 'needs-human':
      return base({
        workers: [
          worker('T-1', 'COMPLETE'),
          worker('T-2', { NEEDS_HUMAN: { kind: 'AMBIGUITY', resume_state: 'IMPLEMENTING' } }),
          worker('T-3', { ACTIVE: 'IMPLEMENTING' }),
        ],
        interventions: [
          {
            id: 'iv-1',
            run_id: 'run-1',
            worker_id: 'w-t-2',
            kind: 'AMBIGUITY',
            summary: 'T-2 asks for the endpoint to be both public and private.',
            evidence: ['"must be reachable by anyone"', '"must only be reachable by staff"'],
            preserved_work: 'Branch vela/run-1/t-2 and its worktree are untouched.',
            last_safe_commit: 'c'.repeat(40),
            consequences: 'Nothing else waits on T-2 yet; T-4 will wait.',
          },
        ],
      });
    case 'completed':
      return base({
        state: 'COMPLETED',
        workers: nodes.map((n) => worker(n.id, 'COMPLETE')),
        promotion: {
          mode: 'pr',
          state: 'PR_OPEN',
          pr_url: 'https://github.com/acme/demo-app/pull/42',
          note: 'A pull request is open for the integration branch.',
        },
      });
  }
}

/** The API a visual page runs against: the demo project, trusted, and the fixed status for the state. */
export function fixtureApi(state: VisualState): ConsoleApi {
  const status = statusFor(state);
  const never = () => Promise.resolve();
  return {
    projectList: () => Promise.resolve([demo]),
    projectImport: () => Promise.resolve({ project: demo, already_known: true }),
    projectRemove: never,
    projectSetTracker: never,
    trustPropose: () =>
      Promise.resolve({
        state: 'Trusted' as const,
        commands: [],
        executable_surfaces: [],
        consent_text_version: 'v1',
      }),
    trustGrant: never,
    trustRevoke: never,
    preflightRun: () => Promise.resolve({ verdict: 'Pass' as const, checks: [] }),
    runPrepare: () =>
      Promise.resolve({
        run_id: 'run-1',
        valid: true,
        tickets: nodes.length,
        edges: edges.length,
        layers: 4,
      }),
    runBuild: never,
    runStop: () => Promise.resolve({}),
    runResume: () => Promise.resolve({}),
    runStatus: () => Promise.resolve(status ?? base({})),
    interventionResolve: never,
    timelineGet: () =>
      Promise.resolve([
        { seq: 1, ts: 1_700_000_000_000, kind: 'WorkerStarted' },
        { seq: 2, ts: 1_700_000_060_000, kind: 'CheckpointCommitted' },
      ]),
    settingsGet: () => Promise.resolve({ max_parallel_workers: '3' }),
    settingsSet: never,
  };
}
