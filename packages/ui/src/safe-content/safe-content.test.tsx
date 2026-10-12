// @vitest-environment jsdom
import { cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { isSafeExternalUrl, parseMarkdown, SafeMarkdown } from './index';

afterEach(cleanup);

const ALLOWED_TAGS = new Set([
  'DIV',
  'P',
  'H1',
  'H2',
  'H3',
  'H4',
  'H5',
  'H6',
  'UL',
  'LI',
  'STRONG',
  'EM',
  'CODE',
  'PRE',
  'BUTTON',
]);
const FORBIDDEN_SELECTOR =
  'script, img, iframe, svg, object, embed, style, link, meta, base, form, input, video, audio, a, [href], [src], [srcdoc]';

/** Payloads that would execute or load something if the renderer treated them as HTML or followed their links. */
const CORPUS = [
  '<script>alert(1)</script>',
  '<img src=x onerror=alert(1)>',
  '<svg onload=alert(1)></svg>',
  '<iframe src="https://evil.example"></iframe>',
  '<a href="javascript:alert(1)">click</a>',
  '[click](javascript:alert(1))',
  '[click](JaVaScRiPt:alert(1))',
  '[click](  javascript:alert(1))',
  '[click](data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==)',
  '[click](file:///C:/Windows/System32/cmd.exe)',
  '[click](vbscript:msgbox(1))',
  '![tracking pixel](https://evil.example/p.png)',
  '**<b onclick=alert(1)>bold</b>**',
  '`<script>alert(1)</script>`',
  '```\n<script>alert(1)</script>\n```',
  '# <img src=x onerror=alert(1)>',
  '- <style>body{display:none}</style>',
  '<link rel="stylesheet" href="https://evil.example/x.css">',
  '<meta http-equiv="refresh" content="0;url=https://evil.example">',
  '<base href="https://evil.example/">',
  '<form action="https://evil.example"><input name=x></form>',
  '&lt;script&gt;alert(1)&lt;/script&gt;',
  '<math><mi xlink:href="javascript:alert(1)">x</mi></math>',
  '[a](https://example.com "title" onclick=alert(1))',
];

function assertInert(container: HTMLElement) {
  expect(container.querySelector(FORBIDDEN_SELECTOR)).toBeNull();
  for (const element of container.querySelectorAll('*')) {
    expect(ALLOWED_TAGS.has(element.tagName), `unexpected <${element.tagName}>`).toBe(true);
    for (const attribute of element.getAttributeNames()) {
      expect(attribute.startsWith('on'), `event attribute ${attribute}`).toBe(false);
      expect(element.getAttribute(attribute)?.toLowerCase() ?? '').not.toContain('javascript:');
    }
  }
}

describe('inert Markdown rendering', () => {
  it('renders every corpus payload as inert content', () => {
    for (const payload of CORPUS) {
      const { container, unmount } = render(<SafeMarkdown source={payload} />);
      assertInert(container);
      unmount();
    }
  });

  it('shows script and raw-HTML payloads as text, not as elements', () => {
    const { container } = render(
      <SafeMarkdown source={'<script>alert(1)</script> and <img src=x onerror=alert(1)>'} />,
    );
    expect(container.textContent).toContain('<script>alert(1)</script>');
    expect(container.textContent).toContain('<img src=x onerror=alert(1)>');
    expect(container.querySelector('script, img')).toBeNull();
  });

  it('survives randomized combinations of markup metacharacters', () => {
    let seed = 1234567;
    const next = () => {
      seed = (seed * 1103515245 + 12345) & 0x7fffffff;
      return seed;
    };
    const pieces = [
      '<',
      '>',
      '"',
      "'",
      '`',
      '**',
      '*',
      '_',
      '[',
      ']',
      '(',
      ')',
      '#',
      '- ',
      '\n',
      '```',
      'javascript:',
      'https://a.test',
      'on',
      'x=',
      '&',
      ';',
    ];
    for (let round = 0; round < 400; round += 1) {
      const length = 3 + (next() % 30);
      let source = '';
      for (let i = 0; i < length; i += 1) source += pieces[next() % pieces.length];
      const { container, unmount } = render(<SafeMarkdown source={source} />);
      assertInert(container);
      unmount();
    }
  });

  it('supports the documented subset', () => {
    const blocks = parseMarkdown(
      '# Title\n\nSome **bold** and *em* and `code`.\n\n- one\n- two\n\n```\nfn main() {}\n```',
    );
    expect(blocks.map((b) => b.type)).toEqual(['heading', 'paragraph', 'list', 'code']);
    const { container } = render(
      <SafeMarkdown source={'# Title\n\nSome **bold** and *em* and `code`.\n\n- one\n- two'} />,
    );
    expect(container.querySelector('h1')?.textContent).toBe('Title');
    expect(container.querySelector('strong')?.textContent).toBe('bold');
    expect(container.querySelector('em')?.textContent).toBe('em');
    expect(container.querySelectorAll('li')).toHaveLength(2);
  });
});

describe('external links', () => {
  it('only https and mailto addresses are links', () => {
    expect(isSafeExternalUrl('https://example.com/a?b=c')).toBe(true);
    expect(isSafeExternalUrl('mailto:someone@example.com')).toBe(true);
    for (const bad of [
      'http://example.com',
      'javascript:alert(1)',
      'data:text/html,x',
      'file:///x',
      '//evil.example',
      'example.com',
      '',
      'https://a b',
      'https://a\u0000b',
    ]) {
      expect(isSafeExternalUrl(bad), bad).toBe(false);
    }
  });

  it('a safe link opens through the host only when the user presses it', () => {
    const open = vi.fn();
    const { container } = render(
      <SafeMarkdown source={'see [the docs](https://example.com/docs)'} onOpenExternal={open} />,
    );
    expect(open).not.toHaveBeenCalled();
    const button = container.querySelector('button.safe-link');
    expect(button?.getAttribute('href')).toBeNull();
    fireEvent.click(button as Element);
    expect(open).toHaveBeenCalledExactlyOnceWith('https://example.com/docs');
  });

  it('an unsafe link target is shown as inert text and never becomes a button', () => {
    const open = vi.fn();
    const { container } = render(
      <SafeMarkdown source={'[click me](javascript:alert(1))'} onOpenExternal={open} />,
    );
    expect(container.querySelector('button')).toBeNull();
    expect(container.textContent).toContain('click me');
    expect(open).not.toHaveBeenCalled();
  });
});
