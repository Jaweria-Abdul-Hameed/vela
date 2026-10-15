// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { NODE_VISUALS, type NodeState } from '../canvas/graph/visuals';
import { GraphTree, ShortcutHelp, buildRows } from '../components/graph-tree';
import {
  MAX_FLASHES_PER_SECOND,
  PULSE_PERIOD_MS,
  SHORTCUTS,
  describeState,
  flashesPerSecond,
  hueOnlyPairs,
} from './audit';

afterEach(cleanup);

const tickets = [
  { id: 'T-1', title: 'Schema', state: 'done' as NodeState },
  { id: 'T-2', title: 'API', state: 'running' as NodeState },
  { id: 'T-3', title: 'UI', state: 'blocked' as NodeState },
  { id: 'T-4', title: 'Docs', state: 'ready' as NodeState },
];
const edges = [
  { from: 'T-1', to: 'T-2' },
  { from: 'T-2', to: 'T-3' },
  { from: 'T-1', to: 'T-3' },
];

describe('status is never hue only', () => {
  it('no two states share both a label and a shape', () => {
    expect(hueOnlyPairs()).toEqual([]);
  });

  it('every state is named in words and by shape', () => {
    for (const s of Object.keys(NODE_VISUALS) as NodeState[]) {
      expect(describeState(s)).toMatch(/\(\w+\)$/);
    }
  });
});

describe('no rapid flashing', () => {
  it('the running pulse is one slow cycle, far under three flashes a second', () => {
    expect(flashesPerSecond(PULSE_PERIOD_MS)).toBeLessThan(MAX_FLASHES_PER_SECOND / 3);
    expect(flashesPerSecond(0)).toBe(0);
  });
});

describe('the accessible graph tree', () => {
  it('lists every ticket once with state, blockers and dependents in its label', () => {
    render(<GraphTree tickets={tickets} edges={edges} selected={null} onSelect={() => {}} />);
    const items = screen.getAllByRole('treeitem');
    expect(items).toHaveLength(4);
    const t3 = screen.getByRole('treeitem', { name: /^T-3, UI/ });
    expect(t3.getAttribute('aria-label')).toContain('waiting on another ticket (ring)');
    expect(t3.getAttribute('aria-label')).toContain('blocked by T-2, T-1');
    expect(
      screen.getByRole('treeitem', { name: /^T-1, Schema/ }).getAttribute('aria-label'),
    ).toContain('unblocks T-2, T-3');
    expect(screen.getByRole('treeitem', { name: /^T-4, / }).getAttribute('aria-label')).toContain(
      'no blockers',
    );
  });

  it('is hierarchical: a ticket sits under its first blocker', () => {
    const rows = buildRows(tickets, edges, new Set());
    expect(rows.map((r) => [r.ticket.id, r.level])).toEqual([
      ['T-1', 1],
      ['T-2', 2],
      ['T-3', 3],
      ['T-4', 1],
    ]);
  });

  it('is operable by keyboard alone: arrows, Home, End, Right and Left, Enter', () => {
    const onSelect = vi.fn();
    render(<GraphTree tickets={tickets} edges={edges} selected={null} onSelect={onSelect} />);
    const first = screen.getAllByRole('treeitem')[0]!;
    first.focus();
    expect(first.tabIndex).toBe(0);
    expect(screen.getAllByRole('treeitem').filter((i) => i.tabIndex === 0)).toHaveLength(1);
    fireEvent.keyDown(first, { key: 'ArrowDown' });
    expect(document.activeElement).toBe(screen.getByRole('treeitem', { name: /^T-2, / }));
    fireEvent.keyDown(document.activeElement!, { key: 'End' });
    expect(document.activeElement).toBe(screen.getByRole('treeitem', { name: /^T-4, / }));
    fireEvent.keyDown(document.activeElement!, { key: 'Home' });
    expect(document.activeElement).toBe(first);
    // Left closes the open group; Right opens it again.
    expect(first.getAttribute('aria-expanded')).toBe('true');
    fireEvent.keyDown(first, { key: 'ArrowLeft' });
    expect(screen.queryByRole('treeitem', { name: /^T-2, / })).toBeNull();
    expect(screen.getByRole('treeitem', { name: /^T-1, / }).getAttribute('aria-expanded')).toBe(
      'false',
    );
    fireEvent.keyDown(screen.getByRole('treeitem', { name: /^T-1, / }), { key: 'ArrowRight' });
    expect(screen.getByRole('treeitem', { name: /^T-2, / })).toBeTruthy();
    fireEvent.keyDown(screen.getByRole('treeitem', { name: /^T-2, / }), { key: 'Enter' });
    expect(onSelect).toHaveBeenCalledWith('T-2');
  });

  it('marks the selected ticket and says so when there are none', () => {
    const { rerender } = render(
      <GraphTree tickets={tickets} edges={edges} selected="T-2" onSelect={() => {}} />,
    );
    expect(screen.getByRole('treeitem', { name: /^T-2, / }).getAttribute('aria-selected')).toBe(
      'true',
    );
    rerender(<GraphTree tickets={[]} edges={[]} selected={null} onSelect={() => {}} />);
    expect(screen.getByText('No tickets yet.')).toBeTruthy();
  });
});

describe('shortcut help', () => {
  it('lists every shortcut, focuses its close button and closes on Escape', () => {
    const onClose = vi.fn();
    render(<ShortcutHelp open onClose={onClose} />);
    expect(screen.getByRole('dialog', { name: 'Keyboard shortcuts' })).toBeTruthy();
    expect(screen.getAllByRole('term')).toHaveLength(SHORTCUTS.length);
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Close' }));
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });

  it('renders nothing when closed', () => {
    render(<ShortcutHelp open={false} onClose={() => {}} />);
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('the stop-all shortcut it advertises is the one the run bar uses', () => {
    expect(SHORTCUTS.some((s) => s.keys === 'Ctrl+Shift+.' && /stop/i.test(s.does))).toBe(true);
  });
});
