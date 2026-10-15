import { useEffect, useState } from 'react';

/** Tracks "no input for `ms`" without any per-frame work: one timer, reset by input. Pure of React, so it is testable with fake
 *  timers. `onChange(true)` fires when the idle time elapses, `onChange(false)` on the first input afterwards. */
export class IdleTimer {
  private timer: ReturnType<typeof setTimeout> | null = null;
  private idle = false;

  constructor(
    private readonly ms: number,
    private readonly onChange: (idle: boolean) => void,
  ) {}

  start(): void {
    this.arm();
  }

  /** Input happened. */
  poke(): void {
    if (this.idle) {
      this.idle = false;
      this.onChange(false);
    }
    this.arm();
  }

  stop(): void {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
  }

  private arm(): void {
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => {
      this.idle = true;
      this.onChange(true);
    }, this.ms);
  }
}

const INPUT_EVENTS = ['pointerdown', 'pointermove', 'keydown', 'wheel', 'touchstart'] as const;

/** True once there has been no input for `ms`; false again on the next input. */
export function useIdle(ms: number, enabled = true): boolean {
  const [idle, setIdle] = useState(false);
  useEffect(() => {
    if (!enabled) return;
    const timer = new IdleTimer(ms, setIdle);
    const poke = () => timer.poke();
    for (const e of INPUT_EVENTS) window.addEventListener(e, poke, { passive: true });
    timer.start();
    return () => {
      for (const e of INPUT_EVENTS) window.removeEventListener(e, poke);
      timer.stop();
    };
  }, [ms, enabled]);
  return enabled && idle;
}
