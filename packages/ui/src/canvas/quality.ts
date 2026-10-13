/** Graphics modes and what each one costs (U03). Pure data and pure decisions: the canvases read a profile; nothing here touches
 *  WebGL. The mode is always the user's choice; automatic adaptation only ever **suggests** a mode and never changes the setting. */

export type GraphicsMode = 'full' | 'balanced' | 'efficiency';

export const GRAPHICS_MODES: readonly GraphicsMode[] = ['full', 'balanced', 'efficiency'];

export interface QualityProfile {
  /** Cap on the device pixel ratio. */
  dprCap: number;
  /** Whether the ambient gradient field is drawn at all. */
  ambient: boolean;
  /** Ambient frames per second (it moves slowly, so it does not need more). */
  ambientFps: number;
  /** Pixels between neighbouring dots in the lattice. */
  dotSpacing: number;
  /** Milliseconds without input after which the canvas suspends. */
  idleSuspendMs: number;
  /** What the mode costs and gives, in words (shown in settings). */
  description: string;
}

export const QUALITY: Record<GraphicsMode, QualityProfile> = {
  full: {
    dprCap: 2,
    ambient: true,
    ambientFps: 30,
    dotSpacing: 22,
    idleSuspendMs: 20_000,
    description: 'All ambient effects, the densest dot field and a sharp canvas. Highest GPU use.',
  },
  balanced: {
    dprCap: 1.5,
    ambient: true,
    ambientFps: 20,
    dotSpacing: 28,
    idleSuspendMs: 8_000,
    description: 'The default: a slow ambient field and a lighter dot field.',
  },
  efficiency: {
    dprCap: 1,
    ambient: false,
    ambientFps: 0,
    dotSpacing: 40,
    idleSuspendMs: 2_000,
    description: 'Minimal canvas effects and aggressive idle suspension. Lowest GPU use.',
  },
};

export function isGraphicsMode(value: unknown): value is GraphicsMode {
  return typeof value === 'string' && (GRAPHICS_MODES as readonly string[]).includes(value);
}

/** The profile for a stored setting; anything unknown falls back to Balanced, the default. */
export function profileFor(value: unknown): QualityProfile {
  return QUALITY[isGraphicsMode(value) ? value : 'balanced'];
}

/** Reduced motion keeps the field still whatever the mode says. */
export function ambientFrameIntervalMs(
  profile: QualityProfile,
  reducedMotion: boolean,
): number | null {
  if (reducedMotion || !profile.ambient || profile.ambientFps <= 0) return null;
  return Math.round(1000 / profile.ambientFps);
}

export interface PerformanceSignals {
  /** Average frames per second over the last sample window, while the canvas was active. */
  fps: number | null;
  /** The machine reports it is on battery below the saver threshold. */
  batterySaver: boolean;
  /** Seconds the sample covered. */
  sampleSeconds: number;
}

/** A suggestion for a lighter mode, or null. Never applied automatically; the caller shows it and the user decides. */
export function suggest(
  current: GraphicsMode,
  s: PerformanceSignals,
): { mode: GraphicsMode; reason: string } | null {
  if (current === 'efficiency') return null;
  if (s.batterySaver) {
    return { mode: 'efficiency', reason: 'Your computer is saving battery.' };
  }
  if (s.fps !== null && s.sampleSeconds >= 10 && s.fps < 30) {
    return {
      mode: current === 'full' ? 'balanced' : 'efficiency',
      reason: `The canvas is running at about ${Math.round(s.fps)} frames per second.`,
    };
  }
  return null;
}
