// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  BuildReady,
  ConflictCard,
  TicketInspector,
  validateGroup,
  type BuildReadyProps,
  type TicketInspection,
} from './index';

afterEach(cleanup);

const inspection: TicketInspection = {
  id: 'T-2',
  title: 'Add login form',
  status: 'being worked on',
  stage: 'IMPLEMENTING',
  branch: 'vela/run-1/t-2',
  worktree: 'C:/wt/t-2',
  dependsOn: ['T-1'],
  blocks: ['T-4'],
  tests: [
    { command: 'cargo test', passed: false, durationMs: 4200, tail: 'assertion failed: x == y' },
    { command: 'cargo clippy', passed: true, durationMs: 900 },
  ],
  findings: [{ severity: 'high', summary: 'missing null check', blocking: true }],
};

describe('ticket inspector', () => {
  it('shows status, stage, branch, worktree and dependencies up front', () => {
    render(<TicketInspector inspection={inspection} onClose={() => {}} />);
    expect(screen.getByText(/T-2 · being worked on · IMPLEMENTING/)).toBeTruthy();
    expect(screen.getByText('vela/run-1/t-2')).toBeTruthy();
    expect(screen.getByText('C:/wt/t-2')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'T-1' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'T-4' })).toBeTruthy();
  });

  it('discloses evidence, then raw output, step by step', () => {
    render(<TicketInspector inspection={inspection} onClose={() => {}} />);
    expect(screen.getByText(/1 of 2 checks passed, 1 findings/)).toBeTruthy();
    expect(screen.queryByText('assertion failed: x == y')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Show output' }));
    expect(screen.getByText('assertion failed: x == y')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Hide output' }));
    expect(screen.queryByText('assertion failed: x == y')).toBeNull();
    expect(screen.getByText(/Blocking \(high\): missing null check/)).toBeTruthy();
  });

  it('opens a dependency and closes on Escape', () => {
    const open = vi.fn();
    const close = vi.fn();
    render(<TicketInspector inspection={inspection} onClose={close} onOpenTicket={open} />);
    fireEvent.click(screen.getByRole('button', { name: 'T-1' }));
    expect(open).toHaveBeenCalledWith('T-1');
    fireEvent.keyDown(screen.getByRole('complementary'), { key: 'Escape' });
    expect(close).toHaveBeenCalled();
  });
});

const ready = (extra: Partial<BuildReadyProps> = {}): BuildReadyProps => ({
  tickets: 4,
  layers: 3,
  edges: 4,
  analysis: 'SEQUENTIAL_FALLBACK',
  checks: [
    { id: 'a', title: 'Git version', severity: 'PASS', detail: 'ok' },
    {
      id: 'b',
      title: 'Remote',
      severity: 'WARN',
      detail: 'no remote',
      remediation: 'add a remote',
    },
  ],
  pairs: [
    { a: 'T-2', b: 'T-3', label: 'unknown_sequential', reasons: ['no predicted write sets'] },
  ],
  maxWorkers: 2,
  onMaxWorkers: () => {},
  onBuild: () => {},
  ...extra,
});

describe('build-ready panel', () => {
  it('summarizes the graph and explains a serialized pair on demand', () => {
    render(<BuildReady {...ready()} />);
    expect(screen.getByText(/4 tickets in 3 steps, 4 dependencies/)).toBeTruthy();
    expect(screen.getByText(/run one after another/)).toBeTruthy();
    expect(screen.queryByText('no predicted write sets')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'T-2 and T-3' }));
    expect(screen.getByText('no predicted write sets')).toBeTruthy();
    expect(screen.getByText(/not enough evidence to run them together/)).toBeTruthy();
  });

  it('builds when there are only warnings, and the worker count is bounded', () => {
    const onBuild = vi.fn();
    const onMax = vi.fn();
    render(<BuildReady {...ready({ onBuild, onMaxWorkers: onMax })} />);
    expect(screen.getByText(/0 blocking, 1 warnings/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Build' }));
    expect(onBuild).toHaveBeenCalledOnce();
    fireEvent.change(screen.getByRole('spinbutton'), { target: { value: '99' } });
    expect(onMax).toHaveBeenLastCalledWith(8);
    fireEvent.change(screen.getByRole('spinbutton'), { target: { value: '0' } });
    expect(onMax).toHaveBeenLastCalledWith(1);
  });

  it('a blocking finding disables Build and says why', () => {
    const checks = [
      {
        id: 'x',
        title: 'Trust',
        severity: 'BLOCK' as const,
        detail: 'untrusted',
        remediation: 'trust the repository',
      },
    ];
    render(<BuildReady {...ready({ checks })} />);
    const build = screen.getByRole<HTMLButtonElement>('button', { name: 'Build' });
    expect(build.disabled).toBe(true);
    expect(screen.getByText(/To fix: trust the repository/)).toBeTruthy();
    expect(screen.getByRole('status').textContent).toContain('Fix the blocking items first');
  });
});

describe('conflict card and manual groups', () => {
  it('shows tickets, the concrete shared surfaces, reasons and the recommendation', () => {
    const onHighlight = vi.fn();
    render(
      <ConflictCard
        conflict={{
          id: 'c1',
          tickets: ['T-2', 'T-3'],
          surfaces: ['migrations/0001.sql'],
          reasons: ['both change the schema'],
          recommendation: 'run T-3 after T-2',
          detected: true,
        }}
        onHighlight={onHighlight}
      />,
    );
    expect(screen.getByText('These tickets overlap')).toBeTruthy();
    expect(screen.getByText('migrations/0001.sql')).toBeTruthy();
    expect(screen.getByText('both change the schema')).toBeTruthy();
    expect(screen.getByText('Recommended: run T-3 after T-2')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Show on the graph' }));
    expect(onHighlight).toHaveBeenCalledWith(['T-2', 'T-3']);
  });

  it('manual grouping explains what to change', () => {
    const known = new Set(['T-1', 'T-2', 'T-3']);
    const dep = (a: string, b: string) => a === 'T-1' && b === 'T-2';
    expect(validateGroup(['T-2'], known, dep).ok).toBe(false);
    expect(validateGroup(['T-2', 'T-9'], known, dep).message).toContain('T-9 is not in this graph');
    expect(validateGroup(['T-1', 'T-2'], known, dep).message).toContain('already waits');
    expect(validateGroup(['T-2', 'T-3', 'T-3'], known, dep)).toEqual({
      ok: true,
      message: 'T-2, T-3 will run one after another.',
    });
  });
});
