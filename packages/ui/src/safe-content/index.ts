// Inert Markdown subset renderer (X02, ADR-013): everything that came from a repository, an issue, a transcript or an agent
// is untrusted, so it is rendered as React text nodes and a fixed set of elements, never as HTML. There is no raw-HTML
// path, no images, no `href`, and a link is only a button that asks the host to open the URL through the operating system
// when the user presses it, and only for https and mailto addresses.
import { createElement, Fragment, type ReactNode } from 'react';

export type Inline =
  | { type: 'text'; text: string }
  | { type: 'strong'; children: Inline[] }
  | { type: 'em'; children: Inline[] }
  | { type: 'code'; text: string }
  | { type: 'link'; text: string; url: string };

export type Block =
  | { type: 'heading'; level: 1 | 2 | 3 | 4 | 5 | 6; children: Inline[] }
  | { type: 'paragraph'; children: Inline[] }
  | { type: 'list'; items: Inline[][] }
  | { type: 'code'; text: string };

/** Link schemes that may be handed to the OS. Everything else (javascript:, data:, file:, ...) stays plain text. */
const SAFE_PROTOCOLS: ReadonlySet<string> = new Set(['https:', 'mailto:']);

/** True when `url` is an absolute https or mailto address with no embedded control characters. */
export function isSafeExternalUrl(url: string): boolean {
  // eslint-disable-next-line no-control-regex
  if (/[\u0000-\u001f\u007f\s]/.test(url)) return false;
  try {
    return SAFE_PROTOCOLS.has(new URL(url).protocol);
  } catch {
    return false;
  }
}

const MAX_INLINE_DEPTH = 4;

/** Parses inline markup: `code`, **strong**, *em* / _em_, [text](url). Anything that does not match is literal text. */
export function parseInline(source: string, depth = 0): Inline[] {
  const out: Inline[] = [];
  let text = '';
  const flush = () => {
    if (text !== '') out.push({ type: 'text', text });
    text = '';
  };
  let i = 0;
  while (i < source.length) {
    const rest = source.slice(i);
    let match: RegExpExecArray | null;
    if ((match = /^`([^`\n]+)`/.exec(rest))) {
      flush();
      out.push({ type: 'code', text: match[1] ?? '' });
      i += match[0].length;
    } else if (depth < MAX_INLINE_DEPTH && (match = /^\*\*([^\n]+?)\*\*/.exec(rest))) {
      flush();
      out.push({ type: 'strong', children: parseInline(match[1] ?? '', depth + 1) });
      i += match[0].length;
    } else if (depth < MAX_INLINE_DEPTH && (match = /^(?:\*([^*\n]+)\*|_([^_\n]+)_)/.exec(rest))) {
      flush();
      out.push({ type: 'em', children: parseInline(match[1] ?? match[2] ?? '', depth + 1) });
      i += match[0].length;
    } else if ((match = /^\[([^\]\n]*)\]\(([^)\s]*)\)/.exec(rest))) {
      const label = match[1] ?? '';
      const url = match[2] ?? '';
      if (isSafeExternalUrl(url)) {
        flush();
        out.push({ type: 'link', text: label === '' ? url : label, url });
      } else {
        // Not allowed to become a link: keep the author's text, and show the target as inert text.
        text += label === '' ? match[0] : `${label} (${url})`;
      }
      i += match[0].length;
    } else {
      text += source[i] ?? '';
      i += 1;
    }
  }
  flush();
  return out;
}

/** Parses the supported block subset: headings, paragraphs, bullet lists and fenced code. */
export function parseMarkdown(source: string): Block[] {
  const blocks: Block[] = [];
  const lines = source.replaceAll('\r\n', '\n').split('\n');
  let paragraph: string[] = [];
  let list: Inline[][] = [];
  const flushParagraph = () => {
    if (paragraph.length > 0)
      blocks.push({ type: 'paragraph', children: parseInline(paragraph.join(' ')) });
    paragraph = [];
  };
  const flushList = () => {
    if (list.length > 0) blocks.push({ type: 'list', items: list });
    list = [];
  };
  for (let n = 0; n < lines.length; n += 1) {
    const line = lines[n] ?? '';
    if (line.trimStart().startsWith('```')) {
      flushParagraph();
      flushList();
      const body: string[] = [];
      n += 1;
      while (n < lines.length && !(lines[n] ?? '').trimStart().startsWith('```')) {
        body.push(lines[n] ?? '');
        n += 1;
      }
      blocks.push({ type: 'code', text: body.join('\n') });
      continue;
    }
    const heading = /^(#{1,6})\s+(.*)$/.exec(line);
    const item = /^\s*[-*]\s+(.*)$/.exec(line);
    if (heading) {
      flushParagraph();
      flushList();
      blocks.push({
        type: 'heading',
        level: (heading[1]?.length ?? 1) as 1 | 2 | 3 | 4 | 5 | 6,
        children: parseInline(heading[2] ?? ''),
      });
    } else if (item) {
      flushParagraph();
      list.push(parseInline(item[1] ?? ''));
    } else if (line.trim() === '') {
      flushParagraph();
      flushList();
    } else {
      flushList();
      paragraph.push(line.trim());
    }
  }
  flushParagraph();
  flushList();
  return blocks;
}

export interface SafeMarkdownProps {
  source: string;
  /** Called with a safe URL when the user presses a link. The host opens it through the OS. */
  onOpenExternal?: (url: string) => void;
}

function renderInline(nodes: Inline[], onOpen: ((url: string) => void) | undefined): ReactNode {
  return nodes.map((node, index) => {
    switch (node.type) {
      case 'text':
        return createElement(Fragment, { key: index }, node.text);
      case 'code':
        return createElement('code', { key: index }, node.text);
      case 'strong':
        return createElement('strong', { key: index }, renderInline(node.children, onOpen));
      case 'em':
        return createElement('em', { key: index }, renderInline(node.children, onOpen));
      case 'link':
        return createElement(
          'button',
          {
            key: index,
            type: 'button',
            className: 'safe-link',
            title: node.url,
            onClick: () => onOpen?.(node.url),
          },
          node.text,
        );
    }
  });
}

/** Renders untrusted Markdown inertly. */
export function SafeMarkdown({ source, onOpenExternal }: SafeMarkdownProps): ReactNode {
  const blocks = parseMarkdown(source);
  return createElement(
    'div',
    { className: 'safe-markdown' },
    blocks.map((block, index) => {
      switch (block.type) {
        case 'heading':
          return createElement(
            `h${block.level}`,
            { key: index },
            renderInline(block.children, onOpenExternal),
          );
        case 'paragraph':
          return createElement('p', { key: index }, renderInline(block.children, onOpenExternal));
        case 'list':
          return createElement(
            'ul',
            { key: index },
            block.items.map((item, i) =>
              createElement('li', { key: i }, renderInline(item, onOpenExternal)),
            ),
          );
        case 'code':
          return createElement('pre', { key: index }, createElement('code', null, block.text));
      }
    }),
  );
}
