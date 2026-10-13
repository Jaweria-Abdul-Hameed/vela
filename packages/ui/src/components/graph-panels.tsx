import { useState } from 'react';
import { Glass } from '../design';

/** What the inspector shows about one ticket (U07). */
export interface TicketInspection {
  id: string;
  title: string;
  /** Plain status label ("being worked on"). */
  status: string;
  /** The worker's stage ("IMPLEMENTING"), when there is a worker. */
  stage?: string | undefined;
  branch?: string | undefined;
  worktree?: string | undefined;
  dependsOn: string[];
  blocks: string[];
  tests: { command: string; passed: boolean; durationMs: number; tail?: string }[];
  findings: { severity: string; summary: string; blocking: boolean }[];
}

export interface TicketInspectorProps {
  inspection: TicketInspection;
  onClose: () => void;
  onOpenTicket?: (id: string) => void;
}

/** A glass inspector with progressive disclosure: summary first, then evidence, then the raw output of one command. */
export function TicketInspector({ inspection: t, onClose, onOpenTicket }: TicketInspectorProps) {
  const [openLog, setOpenLog] = useState<string | null>(null);
  const passed = t.tests.filter((x) => x.passed).length;
  return (
    <Glass
      role="complementary"
      aria-label={`Ticket ${t.id}`}
      onKeyDown={(e) => {
        if (e.key === 'Escape') onClose();
      }}
    >
      <h2>{t.title}</h2>
      <p>
        {t.id} · {t.status}
        {t.stage ? ` · ${t.stage}` : ''}
      </p>
      {t.branch ? (
        <p>
          Branch <code>{t.branch}</code>
          {t.worktree ? (
            <>
              {' '}
              in <code>{t.worktree}</code>
            </>
          ) : null}
        </p>
      ) : null}
      <section aria-label="Dependencies">
        <h3>Dependencies</h3>
        {t.dependsOn.length === 0 && t.blocks.length === 0 ? <p>None.</p> : null}
        {t.dependsOn.length > 0 ? (
          <p>
            Waits for:{' '}
            {t.dependsOn.map((d) => (
              <button key={d} type="button" onClick={() => onOpenTicket?.(d)}>
                {d}
              </button>
            ))}
          </p>
        ) : null}
        {t.blocks.length > 0 ? (
          <p>
            Blocks:{' '}
            {t.blocks.map((d) => (
              <button key={d} type="button" onClick={() => onOpenTicket?.(d)}>
                {d}
              </button>
            ))}
          </p>
        ) : null}
      </section>
      <details>
        <summary>
          Evidence: {passed} of {t.tests.length} checks passed, {t.findings.length} findings
        </summary>
        {t.tests.length === 0 ? <p>No checks have run yet.</p> : null}
        <ul aria-label="Checks">
          {t.tests.map((x) => (
            <li key={x.command}>
              {x.passed ? 'Passed' : 'Failed'}: <code>{x.command}</code> (
              {Math.round(x.durationMs / 100) / 10}s){' '}
              {x.tail ? (
                <button
                  type="button"
                  aria-expanded={openLog === x.command}
                  onClick={() => setOpenLog(openLog === x.command ? null : x.command)}
                >
                  {openLog === x.command ? 'Hide output' : 'Show output'}
                </button>
              ) : null}
              {openLog === x.command && x.tail ? <pre>{x.tail}</pre> : null}
            </li>
          ))}
        </ul>
        <ul aria-label="Findings">
          {t.findings.map((f, i) => (
            <li key={i}>
              {f.blocking ? 'Blocking' : 'Note'} ({f.severity}): {f.summary}
            </li>
          ))}
        </ul>
      </details>
      <button type="button" onClick={onClose}>
        Close
      </button>
    </Glass>
  );
}

/** One preflight finding. */
export interface PreflightCheckView {
  id: string;
  title: string;
  severity: 'PASS' | 'WARN' | 'BLOCK';
  detail: string;
  remediation?: string | undefined;
}

/** Why two same-layer tickets are or are not allowed to run together. */
export interface PairEvidenceView {
  a: string;
  b: string;
  /** The evidence label from the core (`safe_by_explicit_separation`, `serialized_by_contract`, ...). */
  label: string;
  reasons: string[];
}

export const PAIR_LABEL: Record<string, string> = {
  safe_by_explicit_separation: 'Safe together: they touch different parts of the project.',
  safe_with_caution: 'Safe together, with some caution: same area, different files.',
  serialized_by_dependency: 'Run one after the other: one depends on the other.',
  serialized_by_contract: 'Run one after the other: they share a file, contract or resource.',
  unknown_sequential: 'Run one after the other: there is not enough evidence to run them together.',
};

export interface BuildReadyProps {
  tickets: number;
  layers: number;
  edges: number;
  analysis: 'ANALYZED' | 'SEQUENTIAL_FALLBACK';
  checks: PreflightCheckView[];
  pairs: PairEvidenceView[];
  maxWorkers: number;
  onMaxWorkers: (n: number) => void;
  onBuild: () => void;
  building?: boolean;
}

/** Before Build: what will run, whether the project is ready, how many workers, and why any pair was serialized. */
export function BuildReady(p: BuildReadyProps) {
  const [openPair, setOpenPair] = useState<string | null>(null);
  const blockers = p.checks.filter((c) => c.severity === 'BLOCK');
  const warnings = p.checks.filter((c) => c.severity === 'WARN');
  const canBuild = blockers.length === 0 && !p.building && p.tickets > 0;
  return (
    <Glass role="region" aria-label="Ready to build">
      <h2>Ready to build?</h2>
      <p>
        {p.tickets} tickets in {p.layers} steps, {p.edges} dependencies.{' '}
        {p.analysis === 'ANALYZED'
          ? 'Parallel work was planned from predicted file changes.'
          : 'No change prediction was available, so tickets in the same step run one after another.'}
      </p>
      <section aria-label="Preflight">
        <h3>
          Preflight: {blockers.length} blocking, {warnings.length} warnings
        </h3>
        <ul>
          {p.checks
            .filter((c) => c.severity !== 'PASS')
            .map((c) => (
              <li key={c.id}>
                <strong>{c.severity === 'BLOCK' ? 'Blocking' : 'Warning'}:</strong> {c.title}.{' '}
                {c.detail}
                {c.remediation ? ` To fix: ${c.remediation}.` : ''}
              </li>
            ))}
        </ul>
      </section>
      <label>
        Workers at once{' '}
        <input
          type="number"
          min={1}
          max={8}
          value={p.maxWorkers}
          onChange={(e) => p.onMaxWorkers(Math.min(8, Math.max(1, Number(e.target.value) || 1)))}
        />
      </label>
      <section aria-label="Why tickets are serialized">
        <h3>Which tickets can run together</h3>
        {p.pairs.length === 0 ? <p>Every step has a single ticket.</p> : null}
        <ul>
          {p.pairs.map((pair) => {
            const key = `${pair.a}|${pair.b}`;
            return (
              <li key={key}>
                <button
                  type="button"
                  aria-expanded={openPair === key}
                  onClick={() => setOpenPair(openPair === key ? null : key)}
                >
                  {pair.a} and {pair.b}
                </button>
                {openPair === key ? (
                  <div>
                    <p>{PAIR_LABEL[pair.label] ?? pair.label}</p>
                    <ul>
                      {pair.reasons.map((r, i) => (
                        <li key={i}>{r}</li>
                      ))}
                    </ul>
                  </div>
                ) : null}
              </li>
            );
          })}
        </ul>
      </section>
      <button type="button" disabled={!canBuild} onClick={p.onBuild}>
        {p.building ? 'Starting…' : 'Build'}
      </button>
      {!canBuild && blockers.length > 0 ? <p role="status">Fix the blocking items first.</p> : null}
    </Glass>
  );
}

/** A detected or proposed conflict between tickets (U12). */
export interface ConflictView {
  id: string;
  tickets: string[];
  /** The concrete shared surfaces (files, contracts, resources). */
  surfaces: string[];
  reasons: string[];
  recommendation: string;
  detected: boolean;
}

export function ConflictCard({
  conflict: c,
  onHighlight,
}: {
  conflict: ConflictView;
  onHighlight?: (tickets: string[]) => void;
}) {
  return (
    <Glass role="group" aria-label={`Conflict between ${c.tickets.join(', ')}`}>
      <h3>{c.detected ? 'These tickets overlap' : 'Proposed grouping'}</h3>
      <p>Tickets: {c.tickets.join(', ')}</p>
      <ul aria-label="Shared surfaces">
        {c.surfaces.map((s) => (
          <li key={s}>
            <code>{s}</code>
          </li>
        ))}
      </ul>
      <ul aria-label="Reasons">
        {c.reasons.map((r, i) => (
          <li key={i}>{r}</li>
        ))}
      </ul>
      <p>Recommended: {c.recommendation}</p>
      <button type="button" onClick={() => onHighlight?.(c.tickets)}>
        Show on the graph
      </button>
    </Glass>
  );
}

/** Checks a manual serialization group before it is accepted. The message says what to change, never just "invalid". */
export function validateGroup(
  tickets: string[],
  known: ReadonlySet<string>,
  dependsOn: (a: string, b: string) => boolean,
): { ok: boolean; message: string } {
  const unique = [...new Set(tickets)];
  if (unique.length < 2)
    return { ok: false, message: 'Choose at least two tickets to run one after another.' };
  const unknown = unique.filter((t) => !known.has(t));
  if (unknown.length > 0)
    return {
      ok: false,
      message: `${unknown.join(', ')} ${unknown.length === 1 ? 'is' : 'are'} not in this graph.`,
    };
  for (const a of unique) {
    for (const b of unique) {
      if (a !== b && dependsOn(a, b)) {
        return {
          ok: false,
          message: `${b} already waits for ${a}, so they cannot overlap. Remove one of them from the group.`,
        };
      }
    }
  }
  return { ok: true, message: `${unique.join(', ')} will run one after another.` };
}
