import { useCallback, useEffect, useMemo, useRef, useState, type ComponentType } from 'react';
import {
  BuildReady,
  GraphicsControl,
  InterventionSheet,
  RunBar,
  Timeline,
  ProjectHome,
  TicketInspector,
} from '@vela/ui/src/components';
import type { GraphicsMode } from '@vela/ui/src/canvas/quality';
import { Glass } from '@vela/ui/src/design';
import type { GraphCanvasProps, GraphNode, NodeState } from '@vela/ui/src/canvas/graph';
import { stateOfWorker } from '@vela/ui/src/canvas/graph';
import type {
  ConsoleApi,
  PreflightReport,
  Prepared,
  Project,
  RunStatus,
  TimelineEntry,
  TrustProposal,
} from './api';

/** The name of a state that the core sends either as a plain string or as an object keyed by the state. */
export function stateName(state: unknown): string {
  if (typeof state === 'string') return state;
  if (typeof state === 'object' && state !== null) {
    const entry = Object.entries(state as Record<string, unknown>)[0];
    if (!entry) return 'UNKNOWN';
    const [key, value] = entry;
    return key === 'ACTIVE' && typeof value === 'string' ? value : key;
  }
  return 'UNKNOWN';
}

/** The visual state of each ticket from the run's workers and graph. */
export function nodeStates(status: RunStatus): Map<string, NodeState> {
  const out = new Map<string, NodeState>();
  const done = new Set<string>();
  const phases = new Map<string, string>();
  for (const w of status.workers) {
    const phase = stateName(w.state);
    phases.set(w.ticket, phase);
    if (phase === 'COMPLETE') done.add(w.ticket);
  }
  const blockers = new Map<string, string[]>();
  for (const e of status.graph?.edges ?? [])
    blockers.set(e.to, [...(blockers.get(e.to) ?? []), e.from]);
  for (const n of status.graph?.nodes ?? []) {
    const phase = phases.get(n.id);
    if (phase !== undefined) {
      out.set(n.id, stateOfWorker(phase, false));
    } else {
      const waiting = (blockers.get(n.id) ?? []).some((b) => !done.has(b));
      out.set(n.id, waiting ? 'blocked' : 'ready');
    }
  }
  return out;
}

export interface ConsoleProps {
  api: ConsoleApi;
  /** The graph renderer; tests replace the WebGL canvas. */
  Graph: ComponentType<GraphCanvasProps>;
  /** Polling interval for a running run, in milliseconds. */
  pollMs?: number;
  /** The graphics mode control, when the host has one. */
  graphics?: {
    mode: GraphicsMode;
    onChange: (mode: GraphicsMode) => void;
    suggestion?: { mode: GraphicsMode; reason: string } | null;
    onDismissSuggestion?: () => void;
  };
}

function message(error: unknown): string {
  if (typeof error === 'object' && error !== null && 'what_failed' in error) {
    const e = error as { what_failed: unknown; user_action?: unknown };
    const action = typeof e.user_action === 'string' && e.user_action ? ` ${e.user_action}.` : '';
    return `${String(e.what_failed)}.${action}`;
  }
  return error instanceof Error ? error.message : String(error);
}

/** Vela's working screen: projects, trust, preflight, the plan, the run and what needs you. */
export function Console({ api, Graph, pollMs = 1500, graphics }: ConsoleProps) {
  const [projects, setProjects] = useState<Project[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);
  const [dropActive, setDropActive] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [proposal, setProposal] = useState<TrustProposal | null>(null);
  const [chosen, setChosen] = useState<Set<string>>(new Set());
  const [report, setReport] = useState<PreflightReport | null>(null);
  const [prepared, setPrepared] = useState<Prepared | null>(null);
  const [runId, setRunId] = useState<string | null>(null);
  const [status, setStatus] = useState<RunStatus | null>(null);
  const [building, setBuilding] = useState(false);
  const [maxWorkers, setMaxWorkers] = useState(2);
  const [ticket, setTicket] = useState<string | null>(null);
  const [timeline, setTimeline] = useState<TimelineEntry[]>([]);
  const [tracker, setTracker] = useState('');

  const guard = useCallback(async <T,>(work: () => Promise<T>): Promise<T | undefined> => {
    try {
      setError(null);
      return await work();
    } catch (e) {
      setError(message(e));
      return undefined;
    }
  }, []);

  const refreshProjects = useCallback(async () => {
    const list = await guard(() => api.projectList());
    if (list) setProjects(list);
  }, [api, guard]);

  useEffect(() => {
    let alive = true;
    api
      .projectList()
      .then((list) => {
        if (alive) setProjects(list);
      })
      .catch((e: unknown) => {
        if (alive) setError(message(e));
      });
    return () => {
      alive = false;
    };
  }, [api]);

  // Follow the run: poll while it can still change.
  useEffect(() => {
    if (!runId) return;
    let alive = true;
    const tick = async () => {
      try {
        const s = await api.runStatus(runId);
        if (alive) setStatus(s);
      } catch (e) {
        if (alive) setError(message(e));
      }
    };
    void tick();
    const id = setInterval(() => void tick(), pollMs);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, [api, runId, pollMs]);

  const choose = async (id: string) => {
    setSelected(id);
    setRunId(null);
    setStatus(null);
    setPrepared(null);
    setReport(null);
    setTicket(null);
    const [p, s] = await Promise.all([
      guard(() => api.trustPropose(id)),
      guard(() => api.settingsGet(id)),
    ]);
    if (p) {
      setProposal(p);
      setChosen(new Set(p.commands.map((c) => c.text)));
    }
    if (s) setMaxWorkers(Number(s['max_parallel_workers'] ?? 2) || 2);
  };

  const doImport = async (path: string) => {
    setImporting(true);
    try {
      const r = await guard(() => api.projectImport(path));
      if (r) {
        await refreshProjects();
        await choose(r.project.id);
      }
    } finally {
      setImporting(false);
    }
  };

  // A folder dropped anywhere on the window is imported (the shell reports drops as events, with real paths).
  const importDropped = useRef(doImport);
  useEffect(() => {
    importDropped.current = doImport;
  });
  useEffect(() => {
    let off: (() => void) | undefined;
    let alive = true;
    void api
      .onFolderDrop?.((e) => {
        if (e.type === 'over') setDropActive(true);
        else if (e.type === 'leave') setDropActive(false);
        else {
          setDropActive(false);
          const first = e.paths[0];
          if (first) void importDropped.current(first);
        }
      })
      .then((stop) => {
        if (alive) off = stop;
        else stop();
      });
    return () => {
      alive = false;
      off?.();
    };
  }, [api]);

  const project = projects.find((p) => p.id === selected) ?? null;
  const states = useMemo(
    () => (status ? nodeStates(status) : new Map<string, NodeState>()),
    [status],
  );
  const nodes: GraphNode[] = useMemo(
    () =>
      (status?.graph?.nodes ?? []).map((n) => ({
        id: n.id,
        x: n.x,
        y: n.y,
        label: n.id,
        state: states.get(n.id) ?? 'blocked',
      })),
    [status, states],
  );
  const runName = status ? stateName(status.state) : null;
  const running = runName === 'RUNNING';

  const openTicket = async (id: string | null) => {
    setTicket(id);
    if (id && runId) {
      const t = await guard(() => api.timelineGet(runId, id));
      if (t) setTimeline(t);
    }
  };

  const inspected = ticket && status ? status.workers.find((w) => w.ticket === ticket) : undefined;

  return (
    <div>
      {error ? <p role="alert">{error}</p> : null}

      {graphics ? (
        <GraphicsControl
          mode={graphics.mode}
          onChange={graphics.onChange}
          suggestion={graphics.suggestion ?? null}
          onDismissSuggestion={graphics.onDismissSuggestion ?? (() => {})}
        />
      ) : null}

      <ProjectHome
        projects={projects.map((p) => ({ id: p.id, name: p.name, repoPath: p.repo_path }))}
        selectedId={selected}
        busy={importing}
        dropActive={dropActive}
        onImport={(path) => void doImport(path)}
        onOpen={(id) => void choose(id)}
        onRemove={(id) =>
          void guard(async () => {
            await api.projectRemove(id);
            if (selected === id) setSelected(null);
            await refreshProjects();
          })
        }
      />

      {project && proposal ? (
        <section aria-label="Repository">
          <h2>{project.name}</h2>
          {proposal.state === 'Untrusted' ? (
            <Glass role="group" aria-label="Trust this repository">
              <h3>This repository is data only</h3>
              <p>
                Vela can read it, but it will not build or run anything in it until you trust it.
                Trusting lets these commands run without asking:
              </p>
              <ul>
                {proposal.commands.map((c) => (
                  <li key={c.text}>
                    <label>
                      <input
                        type="checkbox"
                        checked={chosen.has(c.text)}
                        onChange={(e) => {
                          const next = new Set(chosen);
                          if (e.target.checked) next.add(c.text);
                          else next.delete(c.text);
                          setChosen(next);
                        }}
                      />{' '}
                      <code>{c.text}</code>
                    </label>
                  </li>
                ))}
              </ul>
              {proposal.executable_surfaces.length > 0 ? (
                <p>It also contains: {proposal.executable_surfaces.join(', ')}.</p>
              ) : null}
              <button
                type="button"
                onClick={() =>
                  void guard(async () => {
                    await api.trustGrant(
                      project.id,
                      proposal.commands.filter((c) => chosen.has(c.text)),
                    );
                    await choose(project.id);
                  })
                }
              >
                Trust this repository
              </button>
            </Glass>
          ) : (
            <p>
              Trusted.{' '}
              <button
                type="button"
                onClick={() =>
                  void guard(async () => {
                    await api.trustRevoke(project.id);
                    await choose(project.id);
                  })
                }
              >
                Revoke trust
              </button>
            </p>
          )}
          <label>
            Issue source{' '}
            <input
              value={tracker}
              onChange={(e) => setTracker(e.target.value)}
              placeholder="local:issues or github:owner/name"
            />
          </label>{' '}
          <button
            type="button"
            disabled={tracker.trim() === ''}
            onClick={() => void guard(() => api.projectSetTracker(project.id, tracker.trim()))}
          >
            Use this source
          </button>
          <p>
            <button
              type="button"
              onClick={() => void guard(async () => setReport(await api.preflightRun(project.id)))}
            >
              Check readiness
            </button>{' '}
            <button
              type="button"
              disabled={proposal.state !== 'Trusted'}
              onClick={() =>
                void guard(async () => {
                  const p = await api.runPrepare(project.id);
                  setPrepared(p);
                  setRunId(p.run_id);
                })
              }
            >
              Plan a build
            </button>
          </p>
          {report ? (
            <p role="status">
              Readiness: {report.verdict}.{' '}
              {report.checks.filter((c) => c.severity === 'Block').length} blocking,{' '}
              {report.checks.filter((c) => c.severity === 'Warn').length} warnings.
            </p>
          ) : null}
        </section>
      ) : null}

      {prepared && !prepared.valid ? (
        <Glass role="alert">
          <h3>The issues contain a cycle</h3>
          <ul>
            {(prepared.cycles ?? []).map((c) => (
              <li key={c}>{c}</li>
            ))}
          </ul>
          <p>Remove one dependency in the tracker, then plan again.</p>
        </Glass>
      ) : null}

      {status && prepared?.valid && runName === 'GRAPH_READY' ? (
        <BuildReady
          tickets={prepared.tickets ?? 0}
          layers={prepared.layers ?? 0}
          edges={prepared.edges ?? 0}
          analysis={status.graph?.analyzed ? 'ANALYZED' : 'SEQUENTIAL_FALLBACK'}
          checks={(report?.checks ?? []).map((c) => ({
            id: c.id,
            title: c.title,
            severity: c.severity.toUpperCase() as 'PASS' | 'WARN' | 'BLOCK',
            detail: c.detail,
            remediation: c.remediation ?? undefined,
          }))}
          pairs={(status.graph?.pairs ?? []).map((p) => ({
            a: p.a,
            b: p.b,
            label: p.label,
            reasons: p.reasons,
          }))}
          maxWorkers={maxWorkers}
          onMaxWorkers={(n) => {
            setMaxWorkers(n);
            void guard(() => api.settingsSet(selected, 'max_parallel_workers', String(n)));
          }}
          building={building}
          onBuild={() =>
            void guard(async () => {
              setBuilding(true);
              try {
                await api.runBuild(status.run_id);
              } finally {
                setBuilding(false);
              }
            })
          }
        />
      ) : null}

      {status && nodes.length > 0 ? (
        <section aria-label="Run">
          <h2>Run: {runName}</h2>
          <div style={{ height: 420 }}>
            <Graph
              nodes={nodes}
              edges={(status.graph?.edges ?? []).map((e) => ({
                from: e.from,
                to: e.to,
                origin: 'authored' as const,
              }))}
              selected={ticket}
              onSelect={(id) => void openTicket(id)}
            />
          </div>
          <RunBar
            status={runName
              .toLowerCase()
              .replaceAll('_', ' ')
              .replace(/^./, (c) => c.toUpperCase())}
            workers={status.workers.length}
            mode={status.promotion?.mode ?? 'Pull request'}
            paused={runName === 'PAUSED'}
            active={running || runName === 'PAUSED'}
            approval={status.interventions.length > 0 ? 'waiting' : 'active'}
            onResume={() => void guard(() => api.runResume(status.run_id))}
            onStopAll={() => void guard(() => api.runStop(status.run_id))}
          />
          {status.promotion ? (
            <p role="status">
              Result: {status.promotion.state.toLowerCase().replace('_', ' ')}.{' '}
              {status.promotion.note}{' '}
              {status.promotion.pr_url ? (
                <a href={status.promotion.pr_url}>Open the pull request</a>
              ) : null}
            </p>
          ) : null}
        </section>
      ) : null}

      {status && inspected && timeline.length > 0 ? <Timeline items={timeline} /> : null}

      {status && inspected ? (
        <TicketInspector
          inspection={{
            id: inspected.ticket,
            title: inspected.ticket,
            status: stateName(inspected.state).toLowerCase().replaceAll('_', ' '),
            stage: stateName(inspected.state),
            branch: inspected.branch ?? undefined,
            worktree: inspected.worktree ?? undefined,
            dependsOn: (status.graph?.edges ?? [])
              .filter((e) => e.to === inspected.ticket)
              .map((e) => e.from),
            blocks: (status.graph?.edges ?? [])
              .filter((e) => e.from === inspected.ticket)
              .map((e) => e.to),
            tests: [],
            findings: timeline.map((t) => ({ severity: 'info', summary: t.kind, blocking: false })),
          }}
          onClose={() => setTicket(null)}
          onOpenTicket={(id) => void openTicket(id)}
        />
      ) : null}

      {status?.interventions.map((iv) => (
        <InterventionSheet
          key={iv.id}
          intervention={{
            id: iv.id,
            kind: iv.kind,
            summary: iv.summary,
            evidence: iv.evidence ?? [],
            preservedWork: iv.preserved_work,
            lastSafeCommit: iv.last_safe_commit ?? undefined,
            consequences: iv.consequences,
          }}
          onAction={(action) => {
            if (action === 'cancel') void guard(() => api.runStop(status.run_id));
            else if (action === 'view_evidence')
              void openTicket(status.workers.find((w) => w.id === iv.worker_id)?.ticket ?? null);
            else void guard(() => api.interventionResolve(iv.id, action));
          }}
        />
      ))}
    </div>
  );
}
