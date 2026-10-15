/** Large-graph performance (U17): what keeps a 500-node graph navigable. Pure and allocation-light, so it is measurable without a
 *  GPU. Three things matter: draw only what is on screen (culling), project only as many text labels as are readable (a label per
 *  node at 500 nodes is mostly overlap), and fold a burst of events into one update per frame. A frame sampler turns slow frames
 *  into a *suggestion* for a lighter graphics mode; it never changes the setting itself. */

import { type PerformanceSignals } from '../quality';
import { type Point, type View, type Viewport, toScreen } from './camera';

export interface Placed {
  id: string;
  x: number;
  y: number;
}

/** The nodes whose centre is inside the viewport grown by `margin` pixels (so a node half off-screen still draws). */
export function cull<T extends Placed>(
  nodes: readonly T[],
  view: View,
  vp: Viewport,
  margin = 48,
): T[] {
  const out: T[] = [];
  for (const n of nodes) {
    const s: Point = toScreen(view, vp, n);
    if (s.x >= -margin && s.x <= vp.width + margin && s.y >= -margin && s.y <= vp.height + margin) {
      out.push(n);
    }
  }
  return out;
}

/** Fewer labels as zoom falls: at low zoom only the important nodes are named, and never more than `maxLabels`. */
export function labelBudget(zoom: number, maxLabels = 60): number {
  if (zoom >= 1) return maxLabels;
  if (zoom >= 0.5) return Math.round(maxLabels * 0.6);
  if (zoom >= 0.25) return Math.round(maxLabels * 0.25);
  return 8;
}

/** Which visible nodes get a text label: the selected one first, then the ones nearest the centre of the view, up to the budget.
 *  Nothing here is hidden from assistive technology: the accessible tree always lists every ticket. */
export function labelled<T extends Placed>(
  visible: readonly T[],
  view: View,
  selected: string | null,
  budget: number,
): T[] {
  if (visible.length <= budget) return [...visible];
  const dist = (n: T) => (n.x - view.x) ** 2 + (n.y - view.y) ** 2;
  const ranked = [...visible].sort((a, b) => {
    if (a.id === selected) return -1;
    if (b.id === selected) return 1;
    return dist(a) - dist(b);
  });
  return ranked.slice(0, budget);
}

/** Folds a burst of events into at most one flush per frame: later events with the same key replace earlier ones, so a ticket
 *  that changed five times in a frame is applied once, in the order keys first appeared. */
export class Batcher<T> {
  private pending = new Map<string, T>();
  private scheduled = false;

  constructor(
    private readonly flush: (batch: T[]) => void,
    private readonly schedule: (run: () => void) => void,
  ) {}

  push(key: string, value: T): void {
    this.pending.set(key, value);
    if (!this.scheduled) {
      this.scheduled = true;
      this.schedule(() => this.drain());
    }
  }

  /** Events waiting for the next frame. */
  get size(): number {
    return this.pending.size;
  }

  private drain(): void {
    this.scheduled = false;
    const batch = [...this.pending.values()];
    this.pending = new Map();
    if (batch.length > 0) this.flush(batch);
  }
}

/** Rolling frame times. `fps` is the mean over the window; `signals` feeds the quality suggestion. */
export class FrameSampler {
  private times: number[] = [];

  constructor(private readonly windowFrames = 120) {}

  /** Records one frame's duration in milliseconds. */
  record(frameMs: number): void {
    if (!Number.isFinite(frameMs) || frameMs <= 0) return;
    this.times.push(frameMs);
    if (this.times.length > this.windowFrames) this.times.shift();
  }

  get fps(): number | null {
    if (this.times.length < 10) return null;
    const mean = this.times.reduce((a, b) => a + b, 0) / this.times.length;
    return 1000 / mean;
  }

  /** The 5th-percentile frame rate (the slow tail), the number the budget is written in. */
  get p5fps(): number | null {
    if (this.times.length < 20) return null;
    const sorted = [...this.times].sort((a, b) => b - a);
    const slow = sorted[Math.max(0, Math.ceil(sorted.length * 0.05) - 1)]!;
    return 1000 / slow;
  }

  signals(batterySaver: boolean): PerformanceSignals {
    const seconds = this.times.reduce((a, b) => a + b, 0) / 1000;
    return { fps: this.fps, batterySaver, sampleSeconds: seconds };
  }
}

/** A repeatable graph for performance fixtures: `n` tickets in layers with a few cross edges. */
export function fixtureGraph(n: number): {
  nodes: Placed[];
  edges: { from: string; to: string }[];
} {
  const nodes: Placed[] = [];
  const edges: { from: string; to: string }[] = [];
  const perLayer = Math.max(1, Math.round(Math.sqrt(n)));
  for (let i = 0; i < n; i++) {
    const layer = Math.floor(i / perLayer);
    nodes.push({ id: `T-${i}`, x: (i % perLayer) * 90, y: layer * 90 });
    if (layer > 0) {
      edges.push({ from: `T-${i - perLayer}`, to: `T-${i}` });
      if (i % 7 === 0 && i % perLayer > 0)
        edges.push({ from: `T-${i - perLayer - 1}`, to: `T-${i}` });
    }
  }
  return { nodes, edges };
}
