import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { IdleTimer } from './idle';

describe('IdleTimer', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('goes idle after the quiet time and wakes on the first input', () => {
    const seen: boolean[] = [];
    const t = new IdleTimer(8000, (i) => seen.push(i));
    t.start();
    vi.advanceTimersByTime(7999);
    expect(seen).toEqual([]);
    vi.advanceTimersByTime(1);
    expect(seen).toEqual([true]);
    t.poke();
    expect(seen).toEqual([true, false]);
    vi.advanceTimersByTime(8000);
    expect(seen).toEqual([true, false, true]);
  });

  it('input keeps it awake and costs nothing while it is awake', () => {
    const seen: boolean[] = [];
    const t = new IdleTimer(1000, (i) => seen.push(i));
    t.start();
    for (let i = 0; i < 10; i++) {
      vi.advanceTimersByTime(900);
      t.poke();
    }
    expect(seen).toEqual([]);
    t.stop();
    vi.advanceTimersByTime(5000);
    expect(seen).toEqual([]);
  });
});
