// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { GraphCanvasProps } from '@vela/ui/src/canvas/graph';
import type { ConsoleApi, Prepared, Project, RunStatus } from './api';
import { Console, nodeStates, stateName } from './Console';

afterEach(cleanup);

const project: Project = {
  id: 'p1',
  name: 'demo',
  repo_path: 'C:/work/demo',
  default_branch: 'main',
  head_at_import: 'a'.repeat(40),
  last_opened_ms: 1,
};

function status(over: Partial<RunStatus> = {}): RunStatus {
  return {
    run_id: 'run-1',
    state: { ACTIVE: 'GRAPH_READY' },
    graph: {
      nodes: [
        { id: 'T-1', x: 0, y: 0 },
        { id: 'T-2', x: 200, y: 0 },
      ],
      edges: [{ from: 'T-1', to: 'T-2' }],
      pairs: [],
      analyzed: false,
      warnings: [],
    },
    base: null,
    workers: [],
    interventions: [],
    promotion: null,
    ...over,
  };
}

function fakeApi(over: Partial<ConsoleApi> = {}): ConsoleApi {
  let imported = false;
  const api: ConsoleApi = {
    projectList: () => Promise.resolve(imported ? [project] : []),
    projectImport: () => {
      imported = true;
      return Promise.resolve({ project, already_known: false });
    },
    projectRemove: () => Promise.resolve(),
    projectSetTracker: () => Promise.resolve(),
    trustPropose: () =>
      Promise.resolve({
        state: 'Untrusted' as const,
        commands: [
          { text: 'cargo test', cwd: null },
          { text: 'cargo build', cwd: null },
        ],
        executable_surfaces: ['Cargo.toml (build scripts)'],
        consent_text_version: 'v1',
      }),
    trustGrant: vi.fn(() => Promise.resolve()),
    trustRevoke: () => Promise.resolve(),
    preflightRun: () => Promise.resolve({ verdict: 'Warn' as const, checks: [] }),
    runPrepare: vi.fn((): Promise<Prepared> =>
      Promise.resolve({ run_id: 'run-1', valid: true, tickets: 2, edges: 1, layers: 2 }),
    ),
    runBuild: vi.fn(() => Promise.resolve()),
    runStop: vi.fn(() => Promise.resolve({})),
    runResume: vi.fn(() => Promise.resolve({})),
    runStatus: () => Promise.resolve(status()),
    interventionResolve: vi.fn(() => Promise.resolve()),
    timelineGet: () => Promise.resolve([{ seq: 1, ts: 1, kind: 'WorkerStarted' }]),
    settingsGet: () => Promise.resolve({ max_parallel_workers: '3' }),
    settingsSet: vi.fn(() => Promise.resolve()),
    ...over,
  };
  return api;
}

function Graph({ nodes, onSelect }: GraphCanvasProps) {
  return (
    <ul aria-label="graph">
      {nodes.map((n) => (
        <li key={n.id}>
          <button type="button" onClick={() => onSelect?.(n.id)}>
            {n.id} {n.state}
          </button>
        </li>
      ))}
    </ul>
  );
}

async function importDemo() {
  fireEvent.change(screen.getByLabelText(/Repository folder/), {
    target: { value: 'C:/work/demo' },
  });
  fireEvent.click(screen.getByRole('button', { name: 'Import' }));
  await screen.findByRole('heading', { name: 'demo' });
}

describe('state helpers', () => {
  it('reads core state names in both shapes', () => {
    expect(stateName('COMPLETE')).toBe('COMPLETE');
    expect(stateName({ ACTIVE: 'IMPLEMENTING' })).toBe('IMPLEMENTING');
    expect(stateName({ PAUSED: { reason: 'STOP_ALL' } })).toBe('PAUSED');
    expect(stateName(null)).toBe('UNKNOWN');
  });

  it('tickets without workers are ready or blocked by their unfinished blockers', () => {
    const s = status({
      state: { ACTIVE: 'RUNNING' },
      workers: [
        { id: 'w', ticket: 'T-1', state: { ACTIVE: 'IMPLEMENTING' }, branch: null, worktree: null },
      ],
    });
    expect(nodeStates(s).get('T-1')).toBe('running');
    expect(nodeStates(s).get('T-2')).toBe('blocked');
    const done = status({
      workers: [{ id: 'w', ticket: 'T-1', state: 'COMPLETE', branch: null, worktree: null }],
    });
    expect(nodeStates(done).get('T-1')).toBe('done');
    expect(nodeStates(done).get('T-2')).toBe('ready');
  });
});

describe('console', () => {
  it('starts empty and imports a repository', async () => {
    render(<Console api={fakeApi()} Graph={Graph} />);
    expect(await screen.findByText(/No projects yet/)).toBeTruthy();
    expect(screen.getByRole<HTMLButtonElement>('button', { name: 'Import' }).disabled).toBe(true);
    await importDemo();
    expect(screen.getByText('This repository is data only')).toBeTruthy();
  });

  it('trusts only the commands the user kept', async () => {
    const api = fakeApi();
    render(<Console api={api} Graph={Graph} />);
    await importDemo();
    fireEvent.click(screen.getByRole('checkbox', { name: /cargo build/ }));
    fireEvent.click(screen.getByRole('button', { name: 'Trust this repository' }));
    await waitFor(() =>
      expect(api.trustGrant).toHaveBeenCalledWith('p1', [{ text: 'cargo test', cwd: null }]),
    );
  });

  it('planning a build is only possible for a trusted repository', async () => {
    render(<Console api={fakeApi()} Graph={Graph} />);
    await importDemo();
    expect(screen.getByRole<HTMLButtonElement>('button', { name: 'Plan a build' }).disabled).toBe(
      true,
    );
  });

  it('plans, shows the graph with tickets as ready or blocked, and builds', async () => {
    const api = fakeApi({
      trustPropose: () =>
        Promise.resolve({
          state: 'Trusted' as const,
          commands: [],
          executable_surfaces: [],
          consent_text_version: 'v1',
        }),
    });
    render(<Console api={api} Graph={Graph} pollMs={20} />);
    await importDemo();
    fireEvent.click(screen.getByRole('button', { name: 'Plan a build' }));
    expect(await screen.findByText(/2 tickets in 2 steps, 1 dependencies/)).toBeTruthy();
    expect(await screen.findByText('T-1 ready')).toBeTruthy();
    expect(screen.getByText('T-2 blocked')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Build' }));
    await waitFor(() => expect(api.runBuild).toHaveBeenCalledWith('run-1'));
  });

  it('shows a cycle instead of a plan', async () => {
    const api = fakeApi({
      trustPropose: () =>
        Promise.resolve({
          state: 'Trusted' as const,
          commands: [],
          executable_surfaces: [],
          consent_text_version: 'v1',
        }),
      runPrepare: () =>
        Promise.resolve({ run_id: 'run-1', valid: false, cycles: ['T-1 -> T-2 -> T-1'] }),
    });
    render(<Console api={api} Graph={Graph} pollMs={20} />);
    await importDemo();
    fireEvent.click(screen.getByRole('button', { name: 'Plan a build' }));
    expect(await screen.findByText('The issues contain a cycle')).toBeTruthy();
    expect(screen.getByText('T-1 -> T-2 -> T-1')).toBeTruthy();
  });

  it('follows a running run, offers Stop all, and shows what needs the person', async () => {
    const running = status({
      state: { ACTIVE: 'RUNNING' },
      workers: [
        {
          id: 'w-1',
          ticket: 'T-1',
          state: { NEEDS_HUMAN: { kind: 'AMBIGUITY' } },
          branch: 'vela/run-1/t-1',
          worktree: 'C:/wt/t-1',
        },
      ],
      interventions: [
        {
          id: 'iv-1',
          run_id: 'run-1',
          worker_id: 'w-1',
          kind: 'AMBIGUITY',
          summary: 'the ticket contradicts itself',
          evidence: ['x'],
          preserved_work: 'untouched',
          last_safe_commit: null,
          consequences: 'it waits',
        },
      ],
    });
    const api = fakeApi({
      trustPropose: () =>
        Promise.resolve({
          state: 'Trusted' as const,
          commands: [],
          executable_surfaces: [],
          consent_text_version: 'v1',
        }),
      runStatus: () => Promise.resolve(running),
    });
    render(<Console api={api} Graph={Graph} pollMs={20} />);
    await importDemo();
    fireEvent.click(screen.getByRole('button', { name: 'Plan a build' }));
    expect(await screen.findByText('running')).toBeTruthy();
    expect(await screen.findByText('The ticket needs a decision')).toBeTruthy();
    expect(screen.getByText('T-1 needs_human')).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: 'Resolve and continue' }));
    await waitFor(() => expect(api.interventionResolve).toHaveBeenCalledWith('iv-1', 'resolve'));
    fireEvent.click(screen.getByRole('button', { name: 'Stop all' }));
    fireEvent.click(screen.getByRole('button', { name: 'Yes, stop all' }));
    await waitFor(() => expect(api.runStop).toHaveBeenCalledWith('run-1'));
  });

  it('a paused run offers Resume and a failure is shown with what to do', async () => {
    const paused = status({ state: { PAUSED: { reason: 'STOP_ALL', resume_state: 'RUNNING' } } });
    const api = fakeApi({
      trustPropose: () =>
        Promise.resolve({
          state: 'Trusted' as const,
          commands: [],
          executable_surfaces: [],
          consent_text_version: 'v1',
        }),
      runStatus: () => Promise.resolve(paused),
      runResume: () =>
        Promise.reject(
          Object.assign(new Error('failed'), {
            what_failed: 'reconciliation found a change',
            user_action: 'resolve the open items',
          }),
        ),
    });
    render(<Console api={api} Graph={Graph} pollMs={20} />);
    await importDemo();
    fireEvent.click(screen.getByRole('button', { name: 'Plan a build' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Resume' }));
    expect((await screen.findByRole('alert')).textContent).toBe(
      'reconciliation found a change. resolve the open items.',
    );
  });

  it('removing a project only removes it from Vela', async () => {
    const api = fakeApi({ projectRemove: vi.fn(() => Promise.resolve()) });
    render(<Console api={api} Graph={Graph} />);
    await importDemo();
    fireEvent.click(screen.getByRole('button', { name: 'Remove demo' }));
    await waitFor(() => expect(api.projectRemove).toHaveBeenCalledWith('p1'));
  });
});
