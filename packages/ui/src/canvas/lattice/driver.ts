/** Demand-driven frame control for the lattice (U02): frames happen only while something moves, and never while hidden. */

export interface FrameDriverOptions {
  /** Asks the renderer for one more frame (R3F `invalidate`). */
  invalidate: () => void;
  /** Advances the animation; returns true while it needs another frame. */
  advance: (dtSeconds: number) => boolean;
  /** Reduced motion: no springing, the field is static. */
  reducedMotion?: () => boolean;
}

export class FrameDriver {
  private suspendedFlag = false;
  private pending = false;
  private callbacks = 0;
  private last: number | null = null;

  constructor(private readonly options: FrameDriverOptions) {}

  /** Number of frame callbacks run so far (the accounting the idle and minimized tests read). */
  get frameCallbacks(): number {
    return this.callbacks;
  }

  get suspended(): boolean {
    return this.suspendedFlag;
  }

  /** Something changed (pointer moved, scene set): ensure a frame will run. A suspended or already-queued driver ignores it. */
  request(): void {
    if (this.suspendedFlag || this.pending) return;
    this.pending = true;
    this.options.invalidate();
  }

  /** The renderer's frame callback. `nowSeconds` is a monotonic clock. */
  onFrame(nowSeconds: number): void {
    this.pending = false;
    if (this.suspendedFlag) return;
    this.callbacks += 1;
    const dt = this.last === null ? 1 / 60 : nowSeconds - this.last;
    this.last = nowSeconds;
    const moving = this.options.reducedMotion?.() ? false : this.options.advance(dt);
    if (moving) this.request();
    else this.last = null;
  }

  /** Stops all frames until `resume`. */
  suspend(): void {
    this.suspendedFlag = true;
    this.pending = false;
    this.last = null;
  }

  resume(): void {
    if (!this.suspendedFlag) return;
    this.suspendedFlag = false;
    this.request();
  }
}

export interface VisibilitySource {
  hidden: () => boolean;
  focused: () => boolean;
  /** Registers a change listener; returns its disposer. */
  onChange: (listener: () => void) => () => void;
}

/** Suspends the driver when the window is hidden or minimized at once, and after `blurDelayMs` when only unfocused. */
export function bindVisibility(
  driver: FrameDriver,
  source: VisibilitySource,
  blurDelayMs = 1500,
  timers: { set: (fn: () => void, ms: number) => unknown; clear: (id: unknown) => void } = {
    set: (fn, ms) => setTimeout(fn, ms),
    clear: (id) => clearTimeout(id as ReturnType<typeof setTimeout>),
  },
): () => void {
  let blurTimer: unknown = null;
  const update = () => {
    if (blurTimer !== null) {
      timers.clear(blurTimer);
      blurTimer = null;
    }
    if (source.hidden()) driver.suspend();
    else if (source.focused()) driver.resume();
    else {
      blurTimer = timers.set(() => {
        blurTimer = null;
        if (!source.focused() || source.hidden()) driver.suspend();
      }, blurDelayMs);
    }
  };
  update();
  const off = source.onChange(update);
  return () => {
    off();
    if (blurTimer !== null) timers.clear(blurTimer);
  };
}
