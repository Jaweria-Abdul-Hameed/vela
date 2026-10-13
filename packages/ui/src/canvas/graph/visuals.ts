/** Node states and orchestration events as visuals (U06). State is carried by shape, motion, label and light together, never by
 *  colour alone, and every effect has a reduced-motion alternative that carries the same information. */

export type NodeState =
  | 'blocked'
  | 'ready'
  | 'running'
  | 'waiting_approval'
  | 'needs_human'
  | 'reviewing'
  | 'merging'
  | 'done'
  | 'failed'
  | 'cancelled';

export type Shape = 'ring' | 'dot' | 'diamond' | 'hex' | 'square' | 'cross';

export interface NodeVisual {
  shape: Shape;
  /** The name of the design token that colours it (never a colour literal). */
  token: string;
  /** Whether the node breathes (suppressed under reduced motion; the label then carries the state). */
  pulse: boolean;
  /** The label suffix read by sighted users and screen readers alike. */
  label: string;
  /** 0..1 brightness of the node's light. */
  light: number;
}

export const NODE_VISUALS: Record<NodeState, NodeVisual> = {
  blocked: {
    shape: 'ring',
    token: '--vela-text-muted',
    pulse: false,
    label: 'waiting on another ticket',
    light: 0.25,
  },
  ready: {
    shape: 'ring',
    token: '--vela-cyan',
    pulse: false,
    label: 'ready to start',
    light: 0.55,
  },
  running: {
    shape: 'dot',
    token: '--vela-blue',
    pulse: true,
    label: 'being worked on',
    light: 0.85,
  },
  waiting_approval: {
    shape: 'diamond',
    token: '--vela-warning',
    pulse: false,
    label: 'waiting for an approval',
    light: 0.8,
  },
  needs_human: {
    shape: 'diamond',
    token: '--vela-warning',
    pulse: false,
    label: 'needs you',
    light: 1,
  },
  reviewing: {
    shape: 'hex',
    token: '--vela-violet',
    pulse: true,
    label: 'under review',
    light: 0.7,
  },
  merging: { shape: 'hex', token: '--vela-cyan', pulse: true, label: 'merging', light: 0.8 },
  done: { shape: 'dot', token: '--vela-success', pulse: false, label: 'done', light: 0.6 },
  failed: { shape: 'cross', token: '--vela-danger', pulse: false, label: 'failed', light: 0.7 },
  cancelled: {
    shape: 'square',
    token: '--vela-text-muted',
    pulse: false,
    label: 'cancelled',
    light: 0.2,
  },
};

/** A state that is only distinguishable by colour from another is a design bug: shape, label or motion must differ. */
export function distinguishable(a: NodeState, b: NodeState): boolean {
  const [x, y] = [NODE_VISUALS[a], NODE_VISUALS[b]];
  return x.shape !== y.shape || x.label !== y.label || x.pulse !== y.pulse;
}

export interface Effect {
  /** What plays. */
  kind: 'ripple' | 'cool_wave' | 'ring' | 'marker' | 'edge_flow' | 'pulse' | 'emerge' | 'attention';
  durationMs: number;
  token: string;
  /** What replaces it under reduced motion: a static change that says the same thing. */
  reduced: { kind: 'fade' | 'static_mark'; durationMs: number; description: string };
  /** Short description for the activity log and screen readers. */
  announce: string;
}

const REDUCED = (
  description: string,
  kind: 'fade' | 'static_mark' = 'fade',
): Effect['reduced'] => ({
  kind,
  durationMs: 200,
  description,
});

/** Event kind to effect (MOTION_AND_3D.md "Event animation mapping"). Unlisted events produce no effect. */
export function effectFor(eventKind: string, detail?: { pass?: boolean }): Effect | null {
  switch (eventKind) {
    case 'WorkerStarted':
      return {
        kind: 'ripple',
        durationMs: 900,
        token: '--vela-blue',
        reduced: REDUCED('the node lights up'),
        announce: 'A worker started',
      };
    case 'TestGateFinished':
      return detail?.pass === false
        ? {
            kind: 'marker',
            durationMs: 600,
            token: '--vela-danger',
            reduced: REDUCED('a failure mark appears', 'static_mark'),
            announce: 'A test gate failed',
          }
        : {
            kind: 'cool_wave',
            durationMs: 700,
            token: '--vela-cyan',
            reduced: REDUCED('the node brightens briefly'),
            announce: 'A test gate passed',
          };
    case 'ReviewStarted':
      return {
        kind: 'ring',
        durationMs: 1200,
        token: '--vela-violet',
        reduced: REDUCED('a ring appears', 'static_mark'),
        announce: 'A review started',
      };
    case 'ReviewFindingRaised':
      return {
        kind: 'marker',
        durationMs: 500,
        token: '--vela-warning',
        reduced: REDUCED('a marker appears', 'static_mark'),
        announce: 'A finding was raised',
      };
    case 'ReviewFindingResolved':
      return {
        kind: 'marker',
        durationMs: 400,
        token: '--vela-success',
        reduced: REDUCED('the marker disappears'),
        announce: 'A finding was resolved',
      };
    case 'MergeStarted':
      return {
        kind: 'edge_flow',
        durationMs: 1000,
        token: '--vela-cyan',
        reduced: REDUCED('the edge brightens'),
        announce: 'A merge started',
      };
    case 'MergeCompleted':
      return {
        kind: 'pulse',
        durationMs: 800,
        token: '--vela-success',
        reduced: REDUCED('the integration node brightens'),
        announce: 'A merge completed',
      };
    case 'TicketReady':
      return {
        kind: 'emerge',
        durationMs: 700,
        token: '--vela-cyan',
        reduced: REDUCED('the node fades in'),
        announce: 'A ticket is ready',
      };
    case 'HumanActionRequired':
      // Persistent but never flashing: it holds a steady light until resolved.
      return {
        kind: 'attention',
        durationMs: 0,
        token: '--vela-warning',
        reduced: REDUCED('the node shows a steady mark', 'static_mark'),
        announce: 'Vela needs you',
      };
    default:
      return null;
  }
}

/** The effect to actually play: the full one, or its reduced-motion alternative. */
export function playable(
  effect: Effect,
  reducedMotion: boolean,
): { kind: string; durationMs: number; token: string } {
  return reducedMotion
    ? { kind: effect.reduced.kind, durationMs: effect.reduced.durationMs, token: effect.token }
    : { kind: effect.kind, durationMs: effect.durationMs, token: effect.token };
}

/** Maps a worker phase name from the core to the visual state. */
export function stateOfWorker(phase: string, waitingApproval: boolean): NodeState {
  if (waitingApproval) return 'waiting_approval';
  switch (phase) {
    case 'COMPLETE':
      return 'done';
    case 'FAILED':
      return 'failed';
    case 'CANCELLED':
      return 'cancelled';
    case 'NEEDS_HUMAN':
      return 'needs_human';
    case 'REVIEWING':
    case 'FIXING':
      return 'reviewing';
    case 'MERGING':
    case 'INTEGRATION_VALIDATION':
    case 'CONFLICT_RESOLUTION':
      return 'merging';
    default:
      return 'running';
  }
}
