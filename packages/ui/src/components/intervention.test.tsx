// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  ACTION_LABEL,
  InterventionSheet,
  VARIANTS,
  actionsFor,
  type InterventionView,
} from './index';

afterEach(cleanup);

const view = (kind: string, extra: Partial<InterventionView> = {}): InterventionView => ({
  id: 'iv-1',
  kind,
  summary: 'T-1 wants to run `npm`',
  evidence: ['source: hook-spool'],
  preservedWork: 'the worktree is untouched',
  lastSafeCommit: 'a'.repeat(40),
  consequences: 'the worker waits',
  ...extra,
});

describe('intervention sheet', () => {
  it('shows the blocking state, preserved work, last safe commit, evidence and consequences', () => {
    render(<InterventionSheet intervention={view('APPROVAL_ASK')} onAction={() => {}} />);
    expect(screen.getByRole('dialog', { name: 'Allow this operation?' })).toBeTruthy();
    expect(screen.getByText('T-1 wants to run `npm`')).toBeTruthy();
    expect(screen.getByText('the worktree is untouched')).toBeTruthy();
    expect(screen.getByText('a'.repeat(40))).toBeTruthy();
    expect(screen.getByText('source: hook-spool')).toBeTruthy();
    expect(screen.getByText('the worker waits')).toBeTruthy();
  });

  it('every kind has plain copy and a way out', () => {
    for (const [kind, v] of Object.entries(VARIANTS)) {
      expect(v.title.length, kind).toBeGreaterThan(5);
      expect(v.actions.includes('cancel') || v.actions.includes('keep_refusing'), kind).toBe(true);
    }
  });

  it('a provider policy block offers only re-check, evidence and cancel: no retry, switch-account or key action', () => {
    expect(actionsFor('PROVIDER_POLICY_BLOCK', true)).toEqual([
      'recheck_access',
      'view_evidence',
      'cancel',
    ]);
    render(<InterventionSheet intervention={view('PROVIDER_POLICY_BLOCK')} onAction={() => {}} />);
    expect(screen.getByText(/will not retry and will not work around/)).toBeTruthy();
    const labels = screen.getAllByRole('button').map((b) => b.textContent);
    expect(labels).toEqual(['Check access once', 'View evidence', 'Cancel']);
    expect(labels.join(' ').toLowerCase()).not.toMatch(/retry|switch|key/);
  });

  it('an approval ask can allow once, and offers a standing rule only with its scope shown', () => {
    expect(actionsFor('APPROVAL_ASK', false)).toEqual(['allow_once', 'keep_refusing', 'cancel']);
    const onAction = vi.fn<(a: string) => void>();
    render(
      <InterventionSheet
        intervention={view('APPROVAL_ASK', {
          ruleScope:
            'Allow `npm` (UnknownExecutable) in this project every time, until you revoke it.',
        })}
        onAction={onAction}
      />,
    );
    expect(screen.getByLabelText('Rule scope').textContent).toContain('until you revoke it');
    fireEvent.click(screen.getByRole('button', { name: ACTION_LABEL.create_rule }));
    fireEvent.click(screen.getByRole('button', { name: ACTION_LABEL.allow_once }));
    expect(onAction.mock.calls.map(([a]) => a)).toEqual(['create_rule', 'allow_once']);
  });

  it('unknown kinds fall back to a generic decision card and focus starts on the first action', () => {
    render(<InterventionSheet intervention={view('SOMETHING_NEW')} onAction={() => {}} />);
    expect(screen.getByRole('dialog', { name: 'Vela needs a decision' })).toBeTruthy();
    expect(document.activeElement).toBe(screen.getAllByRole('button')[0]);
  });
});
