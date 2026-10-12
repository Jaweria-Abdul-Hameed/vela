import { describe, expect, it } from 'vitest';
import { FrameDriver, bindVisibility, type VisibilitySource } from './driver';
import { DEFAULT_SPRING, DotSimulation, falloff, generateLattice } from './simulation';

describe('lattice', () => {
  it('is ordered, sparse and centred', () => {
    const l = generateLattice({ width: 100, height: 60, spacing: 20 });
    expect(l.length / 2).toBe(5 * 3);
    expect(l[0]).toBe(10);
    expect(l[1]).toBe(10);
    expect(l[2]! - l[0]!).toBe(20);
    expect(generateLattice({ width: 0, height: 10, spacing: 5 }).length).toBe(0);
  });

  it('falloff is smooth and bounded', () => {
    expect(falloff(0, 100)).toBe(1);
    expect(falloff(100, 100)).toBe(0);
    expect(falloff(150, 100)).toBe(0);
    let prev = 1;
    for (let d = 0; d <= 100; d += 5) {
      const v = falloff(d, 100);
      expect(v).toBeLessThanOrEqual(prev);
      prev = v;
    }
  });
});

describe('spring', () => {
  const rest = generateLattice({ width: 400, height: 400, spacing: 20 });

  it('displaces and brightens dots near the pointer, boundedly', () => {
    const sim = new DotSimulation(rest);
    sim.setPointer({ x: 200, y: 200 });
    for (let i = 0; i < 120; i += 1) sim.step(1 / 60);
    let max = 0;
    let bright = 0;
    for (let i = 0; i < sim.count; i += 1) {
      max = Math.max(max, Math.hypot(sim.offset[i * 2]!, sim.offset[i * 2 + 1]!));
      if (sim.weight[i]! > 0.5) bright += 1;
    }
    expect(max).toBeGreaterThan(1);
    expect(max).toBeLessThanOrEqual(DEFAULT_SPRING.maxDisplacement * 1.25);
    expect(bright).toBeGreaterThan(0);
    // Dots outside the radius are untouched.
    expect(sim.weight[0]).toBe(0);
    expect(sim.offset[0]).toBe(0);
  });

  it('springs back to rest and then reports no motion', () => {
    const sim = new DotSimulation(rest);
    sim.setPointer({ x: 200, y: 200 });
    for (let i = 0; i < 60; i += 1) sim.step(1 / 60);
    sim.setPointer(null);
    let steps = 0;
    while (sim.step(1 / 60) && steps < 2000) steps += 1;
    expect(steps).toBeLessThan(600);
    expect(Array.from(sim.offset).every((v) => v === 0)).toBe(true);
    expect(sim.step(1 / 60)).toBe(false);
  });

  it('is stable for large frame gaps', () => {
    const sim = new DotSimulation(rest);
    sim.setPointer({ x: 200, y: 200 });
    for (let i = 0; i < 20; i += 1) sim.step(5);
    expect(Array.from(sim.offset).every((v) => Number.isFinite(v) && Math.abs(v) < 30)).toBe(true);
  });
});

function harness() {
  let invalidations = 0;
  let work = 0;
  const driver = new FrameDriver({
    invalidate: () => {
      invalidations += 1;
    },
    advance: () => {
      work -= 1;
      return work > 0;
    },
  });
  return {
    driver,
    get invalidations() {
      return invalidations;
    },
    animate: (frames: number) => {
      work = frames;
      driver.request();
    },
    /** What the renderer does: runs a frame for each pending invalidation. */
    pump: (limit = 100) => {
      let t = 0;
      let before = -1;
      while (before !== invalidations && limit-- > 0) {
        before = invalidations;
        t += 0.016;
        driver.onFrame(t);
      }
    },
  };
}

describe('frame driver', () => {
  it('idle runs zero frame callbacks', () => {
    const h = harness();
    expect(h.driver.frameCallbacks).toBe(0);
    expect(h.invalidations).toBe(0);
  });

  it('runs frames only while the animation needs them, then stops', () => {
    const h = harness();
    h.animate(5);
    h.pump();
    expect(h.driver.frameCallbacks).toBe(5);
    const settled = h.invalidations;
    h.pump();
    expect(h.invalidations).toBe(settled);
  });

  it('coalesces requests into one pending frame', () => {
    const h = harness();
    h.animate(1);
    h.driver.request();
    h.driver.request();
    expect(h.invalidations).toBe(1);
  });

  it('a suspended driver runs no callbacks and requests nothing; resume restores it', () => {
    const h = harness();
    h.animate(3);
    h.driver.suspend();
    h.driver.onFrame(1);
    h.driver.request();
    expect(h.driver.frameCallbacks).toBe(0);
    expect(h.invalidations).toBe(1);
    h.driver.resume();
    expect(h.invalidations).toBe(2);
  });

  it('reduced motion never animates', () => {
    let moving = 0;
    const d = new FrameDriver({
      invalidate: () => {},
      advance: () => {
        moving += 1;
        return true;
      },
      reducedMotion: () => true,
    });
    d.request();
    d.onFrame(0);
    expect(moving).toBe(0);
  });
});

describe('visibility', () => {
  function source() {
    const state = { hidden: false, focused: true };
    let listener: () => void = () => {};
    const timers: { fn: () => void; ms: number; id: number }[] = [];
    let nextId = 1;
    const src: VisibilitySource = {
      hidden: () => state.hidden,
      focused: () => state.focused,
      onChange: (l) => {
        listener = l;
        return () => {};
      },
    };
    return {
      state,
      src,
      fire: () => listener(),
      timers,
      timerApi: {
        set: (fn: () => void, ms: number) => {
          const id = nextId++;
          timers.push({ fn, ms, id });
          return id;
        },
        clear: (id: unknown) => {
          const i = timers.findIndex((t) => t.id === id);
          if (i >= 0) timers.splice(i, 1);
        },
      },
    };
  }

  it('suspends at once when hidden or minimized and resumes when visible again', () => {
    const s = source();
    const h = harness();
    bindVisibility(h.driver, s.src, 1500, s.timerApi);
    s.state.hidden = true;
    s.fire();
    expect(h.driver.suspended).toBe(true);
    s.state.hidden = false;
    s.fire();
    expect(h.driver.suspended).toBe(false);
  });

  it('suspends after the interval when only blurred, and not if focus returns first', () => {
    const s = source();
    const h = harness();
    bindVisibility(h.driver, s.src, 1500, s.timerApi);
    s.state.focused = false;
    s.fire();
    expect(h.driver.suspended).toBe(false);
    expect(s.timers[0]?.ms).toBe(1500);
    s.state.focused = true;
    s.fire();
    expect(s.timers.length).toBe(0);
    s.state.focused = false;
    s.fire();
    s.timers[0]!.fn();
    expect(h.driver.suspended).toBe(true);
  });
});
