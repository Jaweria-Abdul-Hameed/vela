import { describe, expect, it } from 'vitest';
import {
  MAX_ZOOM,
  MIN_ZOOM,
  RESET_VIEW,
  ease,
  fit,
  focusOn,
  keyboardView,
  panBy,
  settled,
  toGraph,
  toScreen,
  zoomAt,
  type View,
} from './camera';
import {
  NODE_VISUALS,
  distinguishable,
  effectFor,
  playable,
  stateOfWorker,
  type NodeState,
} from './visuals';

const vp = { width: 800, height: 600 };

describe('camera', () => {
  it('screen and graph coordinates round-trip', () => {
    const view: View = { x: 120, y: -40, zoom: 1.7 };
    const p = { x: 33, y: 81 };
    const back = toGraph(view, vp, toScreen(view, vp, p));
    expect(back.x).toBeCloseTo(p.x);
    expect(back.y).toBeCloseTo(p.y);
  });

  it('zooming keeps the point under the pointer fixed and stays within bounds', () => {
    const anchor = { x: 600, y: 200 };
    const before = toGraph(RESET_VIEW, vp, anchor);
    const zoomed = zoomAt(RESET_VIEW, vp, anchor, 1.5);
    const after = toGraph(zoomed, vp, anchor);
    expect(after.x).toBeCloseTo(before.x);
    expect(after.y).toBeCloseTo(before.y);
    expect(zoomAt(RESET_VIEW, vp, anchor, 1000).zoom).toBe(MAX_ZOOM);
    expect(zoomAt(RESET_VIEW, vp, anchor, 0.00001).zoom).toBe(MIN_ZOOM);
    expect(zoomAt({ ...RESET_VIEW, zoom: Number.NaN }, vp, anchor, 1).zoom).toBeGreaterThan(0);
  });

  it('dragging moves the graph with the pointer', () => {
    const v = panBy({ x: 0, y: 0, zoom: 2 }, 100, -50);
    expect(v).toEqual({ x: -50, y: 25, zoom: 2 });
  });

  it('fit shows every node inside the margin, and an empty graph resets', () => {
    const pts = [
      { x: -300, y: -100 },
      { x: 500, y: 200 },
      { x: 0, y: 0 },
    ];
    const v = fit(pts, vp, 64);
    for (const p of pts) {
      const s = toScreen(v, vp, p);
      expect(s.x).toBeGreaterThanOrEqual(63);
      expect(s.x).toBeLessThanOrEqual(vp.width - 63);
      expect(s.y).toBeGreaterThanOrEqual(63);
      expect(s.y).toBeLessThanOrEqual(vp.height - 63);
    }
    expect(fit([], vp)).toEqual(RESET_VIEW);
    expect(fit([{ x: 5, y: 5 }], vp).zoom).toBeLessThanOrEqual(1.5);
  });

  it('easing converges and ends exactly on the target', () => {
    const to: View = { x: 200, y: 100, zoom: 2 };
    let v: View = RESET_VIEW;
    let steps = 0;
    while (!settled(v, to) && steps < 400) {
      v = ease(v, to, 1 / 60);
      steps += 1;
    }
    expect(v).toEqual(to);
    expect(steps).toBeLessThan(200);
  });

  it('keyboard controls pan, zoom, reset and fit; other keys are left alone', () => {
    const start: View = { x: 0, y: 0, zoom: 1 };
    const fitTo = () => ({ x: 9, y: 9, zoom: 0.5 });
    expect(keyboardView(start, vp, 'ArrowRight', fitTo)!.x).toBeGreaterThan(0);
    expect(keyboardView(start, vp, 'ArrowUp', fitTo)!.y).toBeLessThan(0);
    expect(keyboardView(start, vp, '+', fitTo)!.zoom).toBeGreaterThan(1);
    expect(keyboardView(start, vp, '-', fitTo)!.zoom).toBeLessThan(1);
    expect(keyboardView({ x: 5, y: 5, zoom: 3 }, vp, '0', fitTo)).toEqual(RESET_VIEW);
    expect(keyboardView(start, vp, 'f', fitTo)).toEqual(fitTo());
    expect(keyboardView(start, vp, 'q', fitTo)).toBeNull();
  });

  it('focusing a node centres it at a comfortable zoom', () => {
    expect(focusOn({ x: 0, y: 0, zoom: 0.4 }, { x: 70, y: 80 })).toEqual({
      x: 70,
      y: 80,
      zoom: 1.2,
    });
    expect(focusOn({ x: 0, y: 0, zoom: 2 }, { x: 1, y: 2 }).zoom).toBe(2);
  });
});

describe('node visuals', () => {
  const states = Object.keys(NODE_VISUALS) as NodeState[];

  it('every state is distinguishable from every other without colour', () => {
    for (const a of states) {
      for (const b of states) {
        if (a === b) continue;
        // Two states may share a shape and light, but never shape, label and motion all at once.
        expect(distinguishable(a, b), `${a} vs ${b}`).toBe(true);
      }
    }
  });

  it('colours are design tokens, never literals', () => {
    for (const v of Object.values(NODE_VISUALS)) expect(v.token).toMatch(/^--vela-[a-z-]+$/);
  });

  it('a needs-human node holds a steady light and does not flash', () => {
    expect(NODE_VISUALS.needs_human.pulse).toBe(false);
    const e = effectFor('HumanActionRequired')!;
    expect(e.kind).toBe('attention');
    expect(e.durationMs).toBe(0);
  });

  it('every mapped event has a reduced-motion alternative that is not an animation', () => {
    const kinds = [
      'WorkerStarted',
      'TestGateFinished',
      'ReviewStarted',
      'ReviewFindingRaised',
      'ReviewFindingResolved',
      'MergeStarted',
      'MergeCompleted',
      'TicketReady',
      'HumanActionRequired',
    ];
    for (const k of kinds) {
      const e = effectFor(k)!;
      expect(e, k).toBeTruthy();
      expect(e.announce.length).toBeGreaterThan(3);
      const reduced = playable(e, true);
      expect(['fade', 'static_mark']).toContain(reduced.kind);
      expect(reduced.durationMs).toBeLessThanOrEqual(200);
      expect(playable(e, false).kind).toBe(e.kind);
    }
    expect(effectFor('SomethingElse')).toBeNull();
    expect(effectFor('TestGateFinished', { pass: false })!.kind).toBe('marker');
    expect(effectFor('TestGateFinished', { pass: true })!.kind).toBe('cool_wave');
  });

  it('worker phases map to node states', () => {
    expect(stateOfWorker('IMPLEMENTING', false)).toBe('running');
    expect(stateOfWorker('IMPLEMENTING', true)).toBe('waiting_approval');
    expect(stateOfWorker('REVIEWING', false)).toBe('reviewing');
    expect(stateOfWorker('MERGING', false)).toBe('merging');
    expect(stateOfWorker('COMPLETE', false)).toBe('done');
    expect(stateOfWorker('FAILED', false)).toBe('failed');
    expect(stateOfWorker('NEEDS_HUMAN', false)).toBe('needs_human');
  });
});
