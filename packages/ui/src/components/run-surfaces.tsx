import { useEffect, useRef, useState } from 'react';

// ---------------------------------------------------------------- U09 run bar

export type ApprovalIndicator = 'active' | 'waiting' | 'degraded';

export interface RunBarProps {
  /** Plain-language run state, e.g. "Running", "Paused". */
  status: string;
  workers: number;
  mode: string;
  paused: boolean;
  /** The run can be stopped (any active state). */
  active: boolean;
  approval?: ApprovalIndicator;
  /** Ask before Stop All; configurable off. Never a modal: the confirmation is inline and keyboard reachable. */
  confirmStop?: boolean;
  /** Keyboard shortcut key for Stop All, pressed with Ctrl+Shift. Default `.`. */
  stopShortcut?: string;
  /** Absent when the host has no pause; the Pause button is then not offered. */
  onPause?: () => void;
  onResume: () => void;
  onStopAll: () => void;
}

const APPROVAL_TEXT: Record<ApprovalIndicator, string> = {
  active: 'Approvals: active',
  waiting: 'Approvals: waiting on you',
  degraded: 'Approvals: degraded, answer prompts yourself',
};

/** A quiet capsule: status, pause/resume, and an always reachable Stop All. */
export function RunBar({
  status,
  workers,
  mode,
  paused,
  active,
  approval = 'active',
  confirmStop = true,
  stopShortcut = '.',
  onPause,
  onResume,
  onStopAll,
}: RunBarProps) {
  const [confirming, setConfirming] = useState(false);
  const stopRef = useRef<HTMLButtonElement>(null);

  const requestStop = () => {
    if (!active) return;
    if (confirmStop) setConfirming(true);
    else onStopAll();
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey && e.key === stopShortcut) {
        e.preventDefault();
        if (!active) return;
        if (confirmStop) {
          setConfirming(true);
          stopRef.current?.focus();
        } else onStopAll();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [active, confirmStop, stopShortcut, onStopAll]);

  return (
    <section aria-label="Run controls" data-approval={approval}>
      <span role="status">
        {status} · {workers} {workers === 1 ? 'worker' : 'workers'} · {mode}
      </span>{' '}
      <span>{APPROVAL_TEXT[approval]}</span>{' '}
      {active ? (
        paused ? (
          <button type="button" onClick={onResume}>
            Resume
          </button>
        ) : onPause ? (
          <button type="button" onClick={onPause}>
            Pause
          </button>
        ) : null
      ) : null}{' '}
      <button
        ref={stopRef}
        type="button"
        disabled={!active}
        aria-keyshortcuts={`Control+Shift+${stopShortcut}`}
        onClick={requestStop}
      >
        Stop all
      </button>
      {confirming ? (
        <span role="group" aria-label="Confirm Stop all">
          {' '}
          Stop every worker now?{' '}
          <button
            type="button"
            onClick={() => {
              setConfirming(false);
              onStopAll();
            }}
          >
            Yes, stop all
          </button>{' '}
          <button type="button" autoFocus onClick={() => setConfirming(false)}>
            Keep running
          </button>
        </span>
      ) : null}
    </section>
  );
}

// ---------------------------------------------------------------- U08 timeline

export interface TimelineItem {
  seq: number;
  ts: number;
  kind: string;
  /** Evidence text from the journal record (command, test result, finding); never secret content. */
  detail?: string;
}

export interface TimelineProps {
  items: TimelineItem[];
  /** Rows rendered at once; large logs are windowed. */
  window?: number;
}

const LABELS: Record<string, string> = {
  ApprovalDetected: 'Approval prompt detected',
  ApprovalClassified: 'Approval classified',
  ApprovalDelivered: 'Approval answer delivered',
  ApprovalVerified: 'Approval answer verified',
  ApprovalStalled: 'Approval stalled',
};

export function describeKind(kind: string): string {
  return LABELS[kind] ?? kind.replace(/([a-z])([A-Z])/g, '$1 $2');
}

/** A ticket's events from the journal; each row expands to its evidence. Only a window of a large log is in the DOM. */
export function Timeline({ items, window: size = 100 }: TimelineProps) {
  const [start, setStart] = useState(() => Math.max(0, items.length - size));
  const [open, setOpen] = useState<number | null>(null);
  const from = Math.min(start, Math.max(0, items.length - size));
  const shown = items.slice(from, from + size);
  if (items.length === 0) return <p>No events for this ticket yet.</p>;
  return (
    <section aria-label="Timeline">
      <p>
        Showing {from + 1}–{from + shown.length} of {items.length}{' '}
        <button
          type="button"
          disabled={from === 0}
          onClick={() => setStart(Math.max(0, from - size))}
        >
          Earlier
        </button>{' '}
        <button
          type="button"
          disabled={from + size >= items.length}
          onClick={() => setStart(from + size)}
        >
          Later
        </button>
      </p>
      <ol>
        {shown.map((it) => (
          <li key={it.seq}>
            <button
              type="button"
              aria-expanded={open === it.seq}
              onClick={() => setOpen(open === it.seq ? null : it.seq)}
            >
              {describeKind(it.kind)}
            </button>{' '}
            <time dateTime={new Date(it.ts).toISOString()}>
              {new Date(it.ts).toLocaleTimeString()}
            </time>
            {open === it.seq ? <pre>{it.detail ?? 'No further evidence recorded.'}</pre> : null}
          </li>
        ))}
      </ol>
    </section>
  );
}

// ---------------------------------------------------------------- U16 completion summary

export type Outcome = 'success' | 'partial' | 'failed';

export interface RunSummaryProps {
  outcome: Outcome;
  promotion: string;
  prUrl?: string | null;
  branch?: string | null;
  merged: number;
  total: number;
  testsPassed: number;
  testsFailed: number;
  history: { label: string; when: number }[];
}

const HEADLINE: Record<Outcome, string> = {
  success: 'Run complete',
  partial: 'Run finished with some tickets not merged',
  failed: 'Run did not complete',
};

/** The calm end of a run: what happened, where the result is, and the evidence. */
export function RunSummary(p: RunSummaryProps) {
  return (
    <section aria-label="Run summary" data-outcome={p.outcome}>
      <h2>{HEADLINE[p.outcome]}</h2>
      <p>
        {p.merged} of {p.total} tickets merged. Tests: {p.testsPassed} passed, {p.testsFailed}{' '}
        failed.
      </p>
      <p>
        {p.promotion} {p.prUrl ? <a href={p.prUrl}>Open the pull request</a> : null}
        {!p.prUrl && p.branch ? <code>{p.branch}</code> : null}
      </p>
      {p.history.length > 0 ? (
        <ol aria-label="History">
          {p.history.map((h, i) => (
            <li key={i}>
              {h.label}{' '}
              <time dateTime={new Date(h.when).toISOString()}>
                {new Date(h.when).toLocaleString()}
              </time>
            </li>
          ))}
        </ol>
      ) : null}
    </section>
  );
}
