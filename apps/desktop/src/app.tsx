// Walking-skeleton renderer (F04): proves the typed command round trip and the event channel. It is not product UI;
// scenes and surfaces register themselves later (SHARED_SURFACE_PROTOCOL.md section 7).
import { useEffect, useState } from 'react';
import type { PingEvent, PingRequest, PingResult } from '@vela/contracts/commands/diagnostics';
import { onPingEvent, pingCore } from './ipc';

/** The core operations the skeleton uses, injectable so the component is testable without a window. */
export interface CoreIpc {
  ping: (request: PingRequest) => Promise<PingResult>;
  onPing: (handler: (event: PingEvent) => void) => Promise<() => void>;
}

const defaultIpc: CoreIpc = { ping: pingCore, onPing: onPingEvent };

const GREETING = 'hello from the renderer';

function errorText(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === 'object' && error !== null && 'what_failed' in error) {
    return String(error.what_failed);
  }
  return String(error);
}

export function App({ ipc = defaultIpc }: { ipc?: CoreIpc }) {
  const [result, setResult] = useState<PingResult | null>(null);
  const [event, setEvent] = useState<PingEvent | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    // Subscribe before calling, so the event emitted by the command is not missed.
    void ipc
      .onPing((next) => {
        if (active) setEvent(next);
      })
      .then((stop) => {
        if (active) unlisten = stop;
        else stop();
        return ipc.ping({ message: GREETING });
      })
      .then((next) => {
        if (active) setResult(next);
      })
      .catch((failure: unknown) => {
        if (active) setError(errorText(failure));
      });
    return () => {
      active = false;
      unlisten?.();
    };
  }, [ipc]);

  return (
    <main className="shell">
      <h1>Vela</h1>
      {error === null ? null : <p role="alert">Core unavailable: {error}</p>}
      <p role="status" aria-label="Core connection">
        {result === null ? 'Connecting to the core' : `Core replied: ${result.reply}`}
      </p>
      <p role="status" aria-label="Core event">
        {event === null ? 'Waiting for the core event' : `Event ${event.sequence} received`}
      </p>
    </main>
  );
}
