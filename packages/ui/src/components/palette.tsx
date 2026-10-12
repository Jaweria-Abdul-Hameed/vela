import { useEffect, useMemo, useRef, useState } from 'react';
import { Glass } from '../design';

export interface PaletteAction {
  id: string;
  title: string;
  keywords?: string[];
  /** Runs the action; the palette closes afterwards. */
  run: () => void;
}

/** Ranks actions for a query: every query word must appear in the title or keywords; title-prefix matches come first. */
export function filterActions(actions: PaletteAction[], query: string): PaletteAction[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return actions;
  const scored: Array<[number, PaletteAction]> = [];
  for (const action of actions) {
    const title = action.title.toLowerCase();
    const haystack = [title, ...(action.keywords ?? []).map((k) => k.toLowerCase())].join(' ');
    if (!words.every((w) => haystack.includes(w))) continue;
    const rank = title.startsWith(words[0]!) ? 0 : title.includes(words[0]!) ? 1 : 2;
    scored.push([rank, action]);
  }
  return scored.sort((a, b) => a[0] - b[0]).map(([, a]) => a);
}

export interface CommandPaletteProps {
  actions: PaletteAction[];
  onClose: () => void;
}

/** Keyboard command palette: type to filter, arrows to move, Enter to run, Escape to close. */
export function CommandPalette({ actions, onClose }: CommandPaletteProps) {
  const [query, setQuery] = useState('');
  const [index, setIndex] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const results = useMemo(() => filterActions(actions, query), [actions, query]);
  useEffect(() => {
    input.current?.focus();
  }, []);
  const selected = Math.min(index, Math.max(results.length - 1, 0));

  const run = (action: PaletteAction | undefined) => {
    if (!action) return;
    action.run();
    onClose();
  };

  return (
    <Glass role="dialog" aria-modal="true" aria-label="Command palette">
      <input
        ref={input}
        role="combobox"
        aria-expanded="true"
        aria-controls="palette-list"
        aria-activedescendant={results[selected] ? `palette-${results[selected].id}` : undefined}
        aria-label="Type a command"
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
          setIndex(0);
        }}
        onKeyDown={(e) => {
          if (e.key === 'ArrowDown') {
            e.preventDefault();
            setIndex(Math.min(selected + 1, results.length - 1));
          } else if (e.key === 'ArrowUp') {
            e.preventDefault();
            setIndex(Math.max(selected - 1, 0));
          } else if (e.key === 'Enter') {
            e.preventDefault();
            run(results[selected]);
          } else if (e.key === 'Escape') {
            onClose();
          }
        }}
      />
      <ul role="listbox" id="palette-list">
        {results.length === 0 ? <li role="presentation">No matching command</li> : null}
        {results.map((a, i) => (
          <li
            key={a.id}
            id={`palette-${a.id}`}
            role="option"
            aria-selected={i === selected}
            onClick={() => run(a)}
          >
            {a.title}
          </li>
        ))}
      </ul>
    </Glass>
  );
}
