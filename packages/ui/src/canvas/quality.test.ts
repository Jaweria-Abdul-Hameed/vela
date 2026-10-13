import { describe, expect, it } from 'vitest';
import {
  GRAPHICS_MODES,
  QUALITY,
  ambientFrameIntervalMs,
  isGraphicsMode,
  profileFor,
  suggest,
} from './quality';

describe('graphics modes', () => {
  it('cost goes down from Full to Balanced to Efficiency', () => {
    const [f, b, e] = GRAPHICS_MODES.map((m) => QUALITY[m]);
    expect(f!.dprCap).toBeGreaterThan(b!.dprCap);
    expect(b!.dprCap).toBeGreaterThan(e!.dprCap);
    expect(f!.dotSpacing).toBeLessThan(b!.dotSpacing);
    expect(b!.dotSpacing).toBeLessThan(e!.dotSpacing);
    expect(f!.idleSuspendMs).toBeGreaterThan(e!.idleSuspendMs);
    expect(e!.ambient).toBe(false);
  });

  it('every mode documents its effect on cost', () => {
    for (const m of GRAPHICS_MODES) expect(QUALITY[m].description.length).toBeGreaterThan(20);
  });

  it('unknown stored values fall back to Balanced, the default', () => {
    expect(profileFor('turbo')).toBe(QUALITY.balanced);
    expect(profileFor(undefined)).toBe(QUALITY.balanced);
    expect(isGraphicsMode('full')).toBe(true);
    expect(isGraphicsMode('Full')).toBe(false);
  });

  it('the ambient field never animates under reduced motion or in Efficiency mode', () => {
    expect(ambientFrameIntervalMs(QUALITY.full, true)).toBeNull();
    expect(ambientFrameIntervalMs(QUALITY.efficiency, false)).toBeNull();
    expect(ambientFrameIntervalMs(QUALITY.full, false)).toBe(33);
    expect(ambientFrameIntervalMs(QUALITY.balanced, false)).toBe(50);
  });
});

describe('suggestions', () => {
  it('suggest a lighter mode when frames are slow for long enough, never the same or a heavier one', () => {
    expect(suggest('full', { fps: 20, batterySaver: false, sampleSeconds: 30 })?.mode).toBe(
      'balanced',
    );
    expect(suggest('balanced', { fps: 20, batterySaver: false, sampleSeconds: 30 })?.mode).toBe(
      'efficiency',
    );
    expect(suggest('efficiency', { fps: 5, batterySaver: true, sampleSeconds: 60 })).toBeNull();
  });

  it('wait for enough evidence and for a real problem', () => {
    expect(suggest('full', { fps: 10, batterySaver: false, sampleSeconds: 2 })).toBeNull();
    expect(suggest('full', { fps: 58, batterySaver: false, sampleSeconds: 60 })).toBeNull();
    expect(suggest('full', { fps: null, batterySaver: false, sampleSeconds: 60 })).toBeNull();
  });

  it('battery saver suggests Efficiency and says why', () => {
    const s = suggest('balanced', { fps: 60, batterySaver: true, sampleSeconds: 1 });
    expect(s?.mode).toBe('efficiency');
    expect(s?.reason).toContain('battery');
  });
});
