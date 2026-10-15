import { useEffect, useMemo, useRef, useState } from 'react';
import { NODE_VISUALS, type NodeState } from '../canvas/graph/visuals';
import { SHORTCUTS } from '../a11y/audit';

export interface TreeTicket {
  id: string;
  title: string;
  state: NodeState;
}

export interface TreeEdge {
  from: string;
  to: string;
}

export interface GraphTreeProps {
  tickets: TreeTicket[];
  /** `from` must finish before `to`. */
  edges: TreeEdge[];
  selected: string | null;
  onSelect: (id: string) => void;
}

interface Row {
  ticket: TreeTicket;
  level: number;
  hasChildren: boolean;
  /** Everything the row says, for assistive technology. */
  label: string;
}

/** The hierarchy: a ticket sits under its first blocker (the order tickets are given), so every ticket appears exactly once. */
export function buildRows(
  tickets: TreeTicket[],
  edges: TreeEdge[],
  collapsed: ReadonlySet<string>,
): Row[] {
  const byId = new Map(tickets.map((t) => [t.id, t]));
  const blockers = new Map<string, string[]>();
  const dependents = new Map<string, string[]>();
  for (const e of edges) {
    if (!byId.has(e.from) || !byId.has(e.to)) continue;
    (blockers.get(e.to) ?? blockers.set(e.to, []).get(e.to)!).push(e.from);
    (dependents.get(e.from) ?? dependents.set(e.from, []).get(e.from)!).push(e.to);
  }
  const parentOf = new Map<string, string>();
  for (const t of tickets) {
    const first = blockers.get(t.id)?.[0];
    if (first) parentOf.set(t.id, first);
  }
  const children = new Map<string, TreeTicket[]>();
  for (const t of tickets) {
    const p = parentOf.get(t.id);
    if (p) (children.get(p) ?? children.set(p, []).get(p)!).push(t);
  }
  const rows: Row[] = [];
  const seen = new Set<string>();
  // A closed group hides its descendants; they must not come back through the fallback for tickets in a cycle.
  const hide = (id: string) => {
    for (const k of children.get(id) ?? []) {
      if (!seen.has(k.id)) {
        seen.add(k.id);
        hide(k.id);
      }
    }
  };
  const visit = (t: TreeTicket, level: number) => {
    if (seen.has(t.id)) return;
    seen.add(t.id);
    const kids = children.get(t.id) ?? [];
    const b = blockers.get(t.id) ?? [];
    const d = dependents.get(t.id) ?? [];
    const v = NODE_VISUALS[t.state];
    rows.push({
      ticket: t,
      level,
      hasChildren: kids.length > 0,
      label: [
        `${t.id}, ${t.title}`,
        `${v.label} (${v.shape})`,
        b.length > 0 ? `blocked by ${b.join(', ')}` : 'no blockers',
        d.length > 0 ? `unblocks ${d.join(', ')}` : 'unblocks nothing',
      ].join('. '),
    });
    if (collapsed.has(t.id)) hide(t.id);
    else for (const k of kids) visit(k, level + 1);
  };
  for (const t of tickets) if (!parentOf.has(t.id)) visit(t, 1);
  // A ticket in a cycle has no root; list it rather than lose it.
  for (const t of tickets) visit(t, 1);
  return rows;
}

/** The always-mounted hierarchical alternative to the graph: every ticket, its state in words and shape, its blockers and what it
 *  unblocks, fully operable by keyboard (arrows, Home, End, Enter, Right and Left to open and close groups). */
export function GraphTree({ tickets, edges, selected, onSelect }: GraphTreeProps) {
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());
  const [moved, setActive] = useState<string | null>(null);
  const active = moved ?? selected ?? tickets[0]?.id ?? null;
  const refs = useRef(new Map<string, HTMLLIElement>());
  const rows = useMemo(() => buildRows(tickets, edges, collapsed), [tickets, edges, collapsed]);

  const focusRow = (id: string) => {
    setActive(id);
    refs.current.get(id)?.focus();
  };

  const onKey = (e: React.KeyboardEvent, row: Row, index: number) => {
    const move = (to: Row | undefined) => {
      if (to) {
        e.preventDefault();
        focusRow(to.ticket.id);
      }
    };
    switch (e.key) {
      case 'ArrowDown':
        return move(rows[index + 1]);
      case 'ArrowUp':
        return move(rows[index - 1]);
      case 'Home':
        return move(rows[0]);
      case 'End':
        return move(rows[rows.length - 1]);
      case 'ArrowRight':
        if (row.hasChildren && collapsed.has(row.ticket.id)) {
          e.preventDefault();
          setCollapsed((c) => new Set([...c].filter((x) => x !== row.ticket.id)));
        } else if (row.hasChildren) {
          move(rows[index + 1]);
        }
        return;
      case 'ArrowLeft':
        if (row.hasChildren && !collapsed.has(row.ticket.id)) {
          e.preventDefault();
          setCollapsed((c) => new Set([...c, row.ticket.id]));
        } else {
          move([...rows.slice(0, index)].reverse().find((r) => r.level < row.level));
        }
        return;
      case 'Enter':
      case ' ':
        e.preventDefault();
        onSelect(row.ticket.id);
        return;
      default:
    }
  };

  if (tickets.length === 0) return <p>No tickets yet.</p>;
  return (
    <ul role="tree" aria-label="Tickets by dependency">
      {rows.map((row, index) => (
        <li
          key={row.ticket.id}
          role="treeitem"
          aria-level={row.level}
          aria-selected={row.ticket.id === selected}
          aria-expanded={row.hasChildren ? !collapsed.has(row.ticket.id) : undefined}
          aria-label={row.label}
          tabIndex={row.ticket.id === active ? 0 : -1}
          ref={(el) => {
            if (el) refs.current.set(row.ticket.id, el);
            else refs.current.delete(row.ticket.id);
          }}
          onKeyDown={(e) => onKey(e, row, index)}
          onFocus={() => setActive(row.ticket.id)}
          onClick={() => onSelect(row.ticket.id)}
          style={{ paddingInlineStart: `${(row.level - 1) * 1.25}rem` }}
        >
          <span aria-hidden="true">{GLYPH[NODE_VISUALS[row.ticket.state].shape]}</span>{' '}
          {row.ticket.id} {row.ticket.title} — <span>{NODE_VISUALS[row.ticket.state].label}</span>
        </li>
      ))}
    </ul>
  );
}

/** Text glyphs for the node shapes, so the shape is visible in the list too and not only in the graph. */
const GLYPH = {
  ring: '○',
  dot: '●',
  diamond: '◆',
  hex: '⬢',
  square: '■',
  cross: '✕',
} as const;

export interface ShortcutHelpProps {
  open: boolean;
  onClose: () => void;
}

/** The discoverable list of shortcuts; Escape closes it and focus returns to what opened it. */
export function ShortcutHelp({ open, onClose }: ShortcutHelpProps) {
  const closeRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (open) closeRef.current?.focus();
  }, [open]);
  if (!open) return null;
  return (
    <section
      role="dialog"
      aria-modal="false"
      aria-label="Keyboard shortcuts"
      onKeyDown={(e) => {
        if (e.key === 'Escape') onClose();
      }}
    >
      <h2>Keyboard shortcuts</h2>
      <dl>
        {SHORTCUTS.map((s) => (
          <div key={s.keys}>
            <dt>
              <kbd>{s.keys}</kbd>
            </dt>
            <dd>{s.does}</dd>
          </div>
        ))}
      </dl>
      <button type="button" ref={closeRef} onClick={onClose}>
        Close
      </button>
    </section>
  );
}
