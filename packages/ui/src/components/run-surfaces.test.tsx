// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { probeWebgl, TicketList } from '../canvas/fallback/fallback';
import { RunBar, RunSummary, Timeline, describeKind } from './run-surfaces';

afterEach(cleanup);

const bar = (over: Partial<Parameters<typeof RunBar>[0]> = {}) => ({
  status: 'Running',
  workers: 2,
  mode: 'Autonomous',
  paused: false,
  active: true,
  onPause: vi.fn(),
  onResume: vi.fn(),
  onStopAll: vi.fn(),
  ...over,
});

describe('run bar', () => {
  it('states status, workers and approvals in words', () => {
    render(<RunBar {...bar({ approval: 'waiting' })} />);
    expect(screen.getByRole('status').textContent).toBe('Running · 2 workers · Autonomous');
    expect(screen.getByText('Approvals: waiting on you')).toBeTruthy();
  });

  it('Stop all asks inline first, with the safe choice focused, and never on a stray click', () => {
    const p = bar();
    render(<RunBar {...p} />);
    fireEvent.click(screen.getByRole('button', { name: 'Stop all' }));
    expect(p.onStopAll).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Keep running' }));
    fireEvent.click(screen.getByRole('button', { name: 'Keep running' }));
    expect(screen.queryByRole('group', { name: 'Confirm Stop all' })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Stop all' }));
    fireEvent.click(screen.getByRole('button', { name: 'Yes, stop all' }));
    expect(p.onStopAll).toHaveBeenCalledTimes(1);
  });

  it('confirmation can be configured off, and the keyboard shortcut reaches it', () => {
    const p = bar({ confirmStop: false });
    render(<RunBar {...p} />);
    act(() => {
      window.dispatchEvent(
        new KeyboardEvent('keydown', { key: '.', ctrlKey: true, shiftKey: true }),
      );
    });
    expect(p.onStopAll).toHaveBeenCalledTimes(1);
  });

  it('the shortcut with confirmation on only opens the question', () => {
    const p = bar();
    render(<RunBar {...p} />);
    act(() => {
      window.dispatchEvent(
        new KeyboardEvent('keydown', { key: '.', ctrlKey: true, shiftKey: true }),
      );
    });
    expect(p.onStopAll).not.toHaveBeenCalled();
    expect(screen.getByRole('group', { name: 'Confirm Stop all' })).toBeTruthy();
  });

  it('pause and resume swap, and Stop all is disabled when nothing is active', () => {
    const p = bar({ paused: true });
    const { rerender } = render(<RunBar {...p} />);
    fireEvent.click(screen.getByRole('button', { name: 'Resume' }));
    expect(p.onResume).toHaveBeenCalled();
    rerender(<RunBar {...bar({ active: false })} />);
    expect(screen.getByRole<HTMLButtonElement>('button', { name: 'Stop all' }).disabled).toBe(true);
    expect(screen.queryByRole('button', { name: 'Pause' })).toBeNull();
  });
});

describe('timeline', () => {
  const items = Array.from({ length: 250 }, (_, i) => ({
    seq: i + 1,
    ts: 1_700_000_000_000 + i * 1000,
    kind: i === 249 ? 'ApprovalStalled' : 'WorkerStarted',
    detail: `evidence ${i + 1}`,
  }));

  it('windows a large log, newest first in view, and pages back', () => {
    render(<Timeline items={items} window={100} />);
    expect(screen.getAllByRole('listitem')).toHaveLength(100);
    expect(screen.getByText('Showing 151–250 of 250', { exact: false })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Earlier' }));
    expect(screen.getByText('Showing 51–150 of 250', { exact: false })).toBeTruthy();
  });

  it('expands a row to its evidence and renders approval audit entries', () => {
    render(<Timeline items={items} window={100} />);
    const stalled = screen.getByRole('button', { name: 'Approval stalled' });
    expect(stalled.getAttribute('aria-expanded')).toBe('false');
    fireEvent.click(stalled);
    expect(screen.getByText('evidence 250')).toBeTruthy();
  });

  it('says so when there is nothing yet; kinds read as words', () => {
    render(<Timeline items={[]} />);
    expect(screen.getByText('No events for this ticket yet.')).toBeTruthy();
    expect(describeKind('MergeCompleted')).toBe('Merge Completed');
  });
});

describe('run summary', () => {
  it('shows the result, evidence and where to find it, per outcome', () => {
    render(
      <RunSummary
        outcome="partial"
        promotion="A pull request is open."
        prUrl="https://example.test/pr/1"
        merged={2}
        total={3}
        testsPassed={9}
        testsFailed={1}
        history={[{ label: 'Run started', when: 1_700_000_000_000 }]}
      />,
    );
    expect(screen.getByRole('heading').textContent).toBe(
      'Run finished with some tickets not merged',
    );
    expect(screen.getByText('2 of 3 tickets merged. Tests: 9 passed, 1 failed.')).toBeTruthy();
    expect(screen.getByRole('link', { name: 'Open the pull request' }).getAttribute('href')).toBe(
      'https://example.test/pr/1',
    );
    expect(screen.getByRole('list', { name: 'History' })).toBeTruthy();
  });
});

describe('fallbacks', () => {
  it('classifies WebGL creation failure, throw, and software rendering', () => {
    expect(probeWebgl(() => null)).toBe('unavailable');
    expect(
      probeWebgl(() => {
        throw new Error('no');
      }),
    ).toBe('unavailable');
    const ctx = (name: string) =>
      ({
        getExtension: () => ({ UNMASKED_RENDERER_WEBGL: 1 }),
        getParameter: () => name,
      }) as unknown as WebGLRenderingContext;
    expect(probeWebgl(() => ctx('ANGLE (Google, SwiftShader)'))).toBe('software');
    expect(probeWebgl(() => ctx('NVIDIA GeForce'))).toBe('ok');
  });

  it('the list view stands in for the graph and selects by keyboard-reachable buttons', () => {
    const onSelect = vi.fn();
    render(
      <TicketList
        nodes={[{ id: 'a', label: 'A', state: 'Running' }]}
        selected={null}
        onSelect={onSelect}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'A' }));
    expect(onSelect).toHaveBeenCalledWith('a');
  });
});
