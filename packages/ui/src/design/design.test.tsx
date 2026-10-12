// @vitest-environment jsdom
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import baseCss from './base.css?raw';
import tokensCss from './tokens.css?raw';
import { Glass, ICON_NAMES, Icon, Shell, contrastRatio, parseTokens } from './index';

afterEach(cleanup);

const tokens = parseTokens(tokensCss);

/** Every non-test source file of the ui package, as raw text. */
const all = import.meta.glob<string>('../**/*.{ts,tsx,css}', {
  query: '?raw',
  import: 'default',
  eager: true,
});
const sourceFiles = Object.entries(all).filter(([name]) => !/\.test\./.test(name));

describe('tokens', () => {
  it('meets contrast targets on the canvas', () => {
    const canvas = tokens['--vela-canvas']!;
    for (const name of [
      '--vela-text-primary',
      '--vela-text-secondary',
      '--vela-text-muted',
      '--vela-violet',
      '--vela-blue',
      '--vela-cyan',
      '--vela-success',
      '--vela-warning',
      '--vela-danger',
    ]) {
      expect(contrastRatio(tokens[name]!, canvas), name).toBeGreaterThanOrEqual(4.5);
    }
    // The focus ring needs 3:1 against the canvas (non-text contrast).
    expect(contrastRatio(tokens['--vela-focus']!, canvas)).toBeGreaterThanOrEqual(3);
  });

  it('is the single source of color: no color literals outside tokens.css', () => {
    const color = /#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(/i;
    for (const [file, text] of sourceFiles) {
      if (file.endsWith('tokens.css')) continue;
      expect(text, file).not.toMatch(color);
    }
  });

  it('applies glass only through the Glass component', () => {
    for (const [file, text] of sourceFiles) {
      if (/Glass\.tsx$|base\.css$|tokens\.css$/.test(file)) continue;
      expect(text, file).not.toMatch(/backdrop-filter|vela-glass/);
    }
  });

  it('shows focus without bloom and respects reduced motion', () => {
    const base = baseCss;
    expect(base).toMatch(/:focus-visible\s*{[^}]*outline:[^}]*var\(--vela-focus\)/);
    expect(base).toMatch(/:focus-visible\s*{[^}]*box-shadow:\s*none/);
    expect(base).toMatch(/prefers-reduced-motion: reduce/);
  });
});

describe('components', () => {
  it('shell has branding and one main region and no navigation sidebar', () => {
    render(<Shell>content</Shell>);
    expect(screen.getByRole('main').textContent).toBe('content');
    expect(screen.queryByRole('navigation')).toBeNull();
    expect(screen.getByText('Vela')).toBeTruthy();
  });

  it('glass carries its class and passes props through', () => {
    render(<Glass data-testid="g">x</Glass>);
    expect(screen.getByTestId('g').className).toContain('vela-glass');
  });

  it('icons are one family and decorative unless labelled', () => {
    expect(ICON_NAMES.length).toBeGreaterThan(5);
    const { container } = render(<Icon name="plus" />);
    expect(container.querySelector('svg')?.getAttribute('aria-hidden')).toBe('true');
    render(<Icon name="stop" label="Stop all" />);
    expect(screen.getByRole('img', { name: 'Stop all' })).toBeTruthy();
    for (const n of ICON_NAMES) {
      const { container: c } = render(<Icon name={n} />);
      const svg = c.querySelector('svg')!;
      expect(svg.getAttribute('stroke-width')).toBe('1.5');
      expect(svg.getAttribute('viewBox')).toBe('0 0 24 24');
    }
  });
});
