import { describe, expect, it } from 'vitest';
import { suggest } from '../quality';
import { RESET_VIEW } from './camera';
import { Batcher, FrameSampler, cull, fixtureGraph, labelBudget, labelled } from './perf';

const vp = { width: 1200, height: 800 };

describe('500-node fixture', () => {
  const { nodes, edges } = fixtureGraph(500);

  it('is the size it says and every edge points at real nodes', () => {
    expect(nodes).toHaveLength(500);
    const ids = new Set(nodes.map((n) => n.id));
    expect(edges.every((e) => ids.has(e.from) && ids.has(e.to))).toBe(true);
  });

  it('culling draws only what is on screen at working zoom, and everything when fitted', () => {
    const near = cull(nodes, RESET_VIEW, vp);
    expect(near.length).toBeLessThan(nodes.length / 2);
    const fitted = cull(nodes, { x: 1000, y: 1000, zoom: 0.15 }, vp, 0);
    expect(fitted.length).toBeGreaterThan(near.length);
  });

  it('labels are bounded by the budget at every zoom, and the selected node is always labelled', () => {
    for (const zoom of [0.15, 0.3, 0.6, 1, 2]) {
      const view = { x: 1000, y: 1000, zoom };
      const visible = cull(nodes, view, vp, 0);
      const picked = labelled(visible, view, 'T-250', labelBudget(zoom));
      expect(picked.length).toBeLessThanOrEqual(Math.max(labelBudget(zoom), 0) || 1);
      if (visible.some((n) => n.id === 'T-250') && visible.length > labelBudget(zoom)) {
        expect(picked.some((n) => n.id === 'T-250')).toBe(true);
      }
    }
    expect(labelBudget(0.1)).toBeLessThan(labelBudget(1));
  });

  it('a frame of culling and label selection over 500 nodes stays far inside a 16 ms frame', () => {
    const view = { x: 1000, y: 1000, zoom: 0.5 };
    const runs = 200;
    const started = performance.now();
    for (let i = 0; i < runs; i++) {
      labelled(cull(nodes, view, vp), view, 'T-1', labelBudget(view.zoom));
    }
    const perFrame = (performance.now() - started) / runs;
    // The budget is 16.7 ms for everything; this part gets a small fraction of it even on a slow, busy machine.
    expect(perFrame).toBeLessThan(4);
  });
});

describe('event bursts', () => {
  it('fold into one flush per frame with the latest value per key', () => {
    const flushed: string[][] = [];
    const queue: (() => void)[] = [];
    const b = new Batcher<string>(
      (batch) => flushed.push(batch),
      (run) => queue.push(run),
    );
    for (let i = 0; i < 1000; i++) b.push(`T-${i % 10}`, `v${i}`);
    expect(b.size).toBe(10);
    expect(queue).toHaveLength(1);
    queue.shift()!();
    expect(flushed).toHaveLength(1);
    expect(flushed[0]).toHaveLength(10);
    expect(flushed[0]).toContain('v999');
    expect(flushed[0]).not.toContain('v0');
    // The next burst schedules its own frame.
    b.push('T-1', 'x');
    expect(queue).toHaveLength(1);
  });

  it('a burst of 5000 events is folded quickly', () => {
    const b = new Batcher<number>(
      () => {},
      () => {},
    );
    const started = performance.now();
    for (let i = 0; i < 5000; i++) b.push(`T-${i % 500}`, i);
    expect(performance.now() - started).toBeLessThan(100);
    expect(b.size).toBe(500);
  });
});

describe('frame sampling and suggestions', () => {
  it('needs enough frames before it says anything', () => {
    const s = new FrameSampler();
    for (let i = 0; i < 5; i++) s.record(16);
    expect(s.fps).toBeNull();
    expect(s.p5fps).toBeNull();
  });

  it('reports mean and slow-tail rates', () => {
    const s = new FrameSampler();
    for (let i = 0; i < 95; i++) s.record(10);
    for (let i = 0; i < 5; i++) s.record(50);
    expect(Math.round(s.fps!)).toBe(Math.round(1000 / 12));
    expect(Math.round(s.p5fps!)).toBe(20);
  });

  it('ignores nonsense and keeps a bounded window', () => {
    const s = new FrameSampler(30);
    for (const bad of [0, -1, NaN, Infinity * 0]) s.record(bad);
    for (let i = 0; i < 100; i++) s.record(20);
    expect(Math.round(s.fps!)).toBe(50);
  });

  it('slow frames for long enough suggest a lighter mode; the setting itself is not touched', () => {
    const s = new FrameSampler(2000);
    for (let i = 0; i < 1000; i++) s.record(50); // 20 fps for 50 s
    const suggestion = suggest('full', s.signals(false));
    expect(suggestion?.mode).toBe('balanced');
    const fast = new FrameSampler(2000);
    for (let i = 0; i < 1000; i++) fast.record(8);
    expect(suggest('full', fast.signals(false))).toBeNull();
  });
});
