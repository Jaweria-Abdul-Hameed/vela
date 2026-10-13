// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { GraphicsControl, ProjectHome } from './index';

afterEach(cleanup);

const projects = [{ id: 'p1', name: 'demo', repoPath: 'C:/work/demo' }];

describe('project home', () => {
  it('invites a drop when empty and imports a typed path', () => {
    const onImport = vi.fn();
    render(
      <ProjectHome
        projects={[]}
        selectedId={null}
        onImport={onImport}
        onOpen={() => {}}
        onRemove={() => {}}
      />,
    );
    expect(screen.getByRole('heading', { name: 'Start with a repository' })).toBeTruthy();
    expect(screen.getByText(/No projects yet/)).toBeTruthy();
    const importButton = screen.getByRole<HTMLButtonElement>('button', { name: 'Import' });
    expect(importButton.disabled).toBe(true);
    fireEvent.change(screen.getByLabelText(/Repository folder/), {
      target: { value: '  C:/work/app  ' },
    });
    fireEvent.click(importButton);
    expect(onImport).toHaveBeenCalledWith('C:/work/app');
    expect(screen.getByLabelText<HTMLInputElement>(/Repository folder/).value).toBe('');
  });

  it('shows recent projects as anchors that open and remove', () => {
    const onOpen = vi.fn();
    const onRemove = vi.fn();
    render(
      <ProjectHome
        projects={projects}
        selectedId="p1"
        onImport={() => {}}
        onOpen={onOpen}
        onRemove={onRemove}
      />,
    );
    expect(screen.getByRole('button', { name: 'demo' }).getAttribute('aria-pressed')).toBe('true');
    fireEvent.click(screen.getByRole('button', { name: 'demo' }));
    fireEvent.click(screen.getByRole('button', { name: 'Remove demo' }));
    expect(onOpen).toHaveBeenCalledWith('p1');
    expect(onRemove).toHaveBeenCalledWith('p1');
  });

  it('shows the loading and drop states', () => {
    const { rerender } = render(
      <ProjectHome
        projects={projects}
        selectedId={null}
        busy
        onImport={() => {}}
        onOpen={() => {}}
        onRemove={() => {}}
      />,
    );
    expect(
      screen.getByRole<HTMLButtonElement>('button', { name: 'Importing\u2026' }).disabled,
    ).toBe(true);
    rerender(
      <ProjectHome
        projects={projects}
        selectedId={null}
        dropActive
        onImport={() => {}}
        onOpen={() => {}}
        onRemove={() => {}}
      />,
    );
    expect(screen.getByText('Release to import this folder.')).toBeTruthy();
  });
});

describe('graphics control', () => {
  it('changes the mode and says what it costs', () => {
    const onChange = vi.fn();
    render(<GraphicsControl mode="balanced" onChange={onChange} />);
    expect(screen.getByText(/The default/)).toBeTruthy();
    fireEvent.change(screen.getByLabelText('Graphics'), { target: { value: 'efficiency' } });
    expect(onChange).toHaveBeenCalledWith('efficiency');
  });

  it('a suggestion is only a suggestion: the setting changes only when accepted', () => {
    const onChange = vi.fn();
    const dismiss = vi.fn();
    render(
      <GraphicsControl
        mode="full"
        onChange={onChange}
        suggestion={{
          mode: 'balanced',
          reason: 'The canvas is running at about 20 frames per second.',
        }}
        onDismissSuggestion={dismiss}
      />,
    );
    expect(screen.getByRole('status').textContent).toContain('Switch to Balanced?');
    expect(screen.getByLabelText<HTMLSelectElement>('Graphics').value).toBe('full');
    expect(onChange).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Keep Full' }));
    expect(dismiss).toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Switch' }));
    expect(onChange).toHaveBeenCalledWith('balanced');
  });
});
