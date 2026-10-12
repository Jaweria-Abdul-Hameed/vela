// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  CommandPalette,
  CONSERVATIVE_CHOICES,
  OnboardingSheet,
  SettingsSectionRegistry,
  SettingsSheet,
  createSettingsRegistry,
  filterActions,
  type PaletteAction,
} from './index';

afterEach(cleanup);

const action = (id: string, title: string, keywords: string[] = []): PaletteAction => ({
  id,
  title,
  keywords,
  run: vi.fn(),
});

describe('command palette', () => {
  const actions = [
    action('settings', 'Open settings', ['preferences']),
    action('stop', 'Stop all', ['halt']),
    action('diag', 'Export diagnostics'),
  ];

  it('filters by title and keyword, title-prefix first', () => {
    expect(filterActions(actions, '').length).toBe(3);
    expect(filterActions(actions, 'halt').map((a) => a.id)).toEqual(['stop']);
    expect(filterActions(actions, 'open set').map((a) => a.id)).toEqual(['settings']);
    expect(filterActions(actions, 'zzz')).toEqual([]);
    expect(filterActions(actions, 's').map((a) => a.id)[0]).toBe('stop');
  });

  it('runs the selected action with the keyboard and closes', () => {
    const onClose = vi.fn();
    render(<CommandPalette actions={actions} onClose={onClose} />);
    const input = screen.getByRole('combobox');
    expect(document.activeElement).toBe(input);
    fireEvent.keyDown(input, { key: 'ArrowDown' });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(actions[1]!.run).toHaveBeenCalledOnce();
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('shows an empty state and closes on Escape', () => {
    const onClose = vi.fn();
    render(<CommandPalette actions={actions} onClose={onClose} />);
    const input = screen.getByRole('combobox');
    fireEvent.change(input, { target: { value: 'nothing' } });
    expect(screen.getByText('No matching command')).toBeTruthy();
    fireEvent.keyDown(input, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });
});

describe('settings sheet', () => {
  it('lists the sections of the screen inventory in order', () => {
    const titles = createSettingsRegistry()
      .list()
      .map((s) => s.title);
    expect(titles[0]).toBe('General');
    expect(titles).toContain('Notifications');
    expect(titles.length).toBe(9);
  });

  it('lets later tickets register a section without touching the sheet', () => {
    const registry = createSettingsRegistry();
    registry.register({
      id: 'automation',
      title: 'Automation',
      order: 45,
      render: () => <p>consent</p>,
    });
    const order = registry.list().map((s) => s.id);
    expect(order.indexOf('automation')).toBeGreaterThan(order.indexOf('autonomy'));
    expect(order.indexOf('automation')).toBeLessThan(order.indexOf('git'));
    render(<SettingsSheet registry={registry} onClose={() => {}} />);
    fireEvent.click(screen.getByRole('tab', { name: 'Automation' }));
    expect(screen.getByText('consent')).toBeTruthy();
  });

  it('is an accessible dialog that closes on Escape', () => {
    const onClose = vi.fn();
    render(<SettingsSheet registry={createSettingsRegistry()} onClose={onClose} />);
    const dialog = screen.getByRole('dialog', { name: 'Settings' });
    expect(screen.getAllByRole('tab').length).toBe(9);
    expect(screen.getByRole('tab', { selected: true }).textContent).toBe('General');
    fireEvent.keyDown(dialog, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('an empty registry renders without sections', () => {
    render(<SettingsSheet registry={new SettingsSectionRegistry()} onClose={() => {}} />);
    expect(screen.queryAllByRole('tab').length).toBe(0);
  });
});

describe('onboarding', () => {
  const consents = [
    { key: 'ui_automation' as const, version: 'v1', text: 'Let Vela click Allow' },
    { key: 'background' as const, version: 'v1', text: 'Keep running' },
    { key: 'autostart' as const, version: 'v2', text: 'Start at login' },
  ];

  it('starts with every choice off and submits the conservative answer as is', () => {
    const onComplete = vi.fn();
    render(<OnboardingSheet consents={consents} onComplete={onComplete} />);
    for (const box of screen.getAllByRole('checkbox'))
      expect((box as HTMLInputElement).checked).toBe(false);
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
    expect(onComplete).toHaveBeenCalledWith(CONSERVATIVE_CHOICES, {
      ui_automation: 'v1',
      background: 'v1',
      autostart: 'v2',
    });
  });

  it('records only what the user turned on', () => {
    const onComplete = vi.fn();
    render(<OnboardingSheet consents={consents} onComplete={onComplete} />);
    fireEvent.click(screen.getByLabelText('Keep running'));
    fireEvent.change(screen.getByRole('combobox'), { target: { value: 'auto_safe' } });
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
    expect(onComplete.mock.calls[0]![0]).toEqual({
      background: true,
      autostart: false,
      uiAutomation: false,
      recoveryContinuation: 'auto_safe',
    });
  });
});
