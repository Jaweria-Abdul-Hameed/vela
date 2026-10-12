import { describe, expect, it } from 'vitest';
import {
  commandNames,
  listenPing,
  PING_EVENT,
  ping,
  wireName,
  type PingEvent,
  type PingResult,
} from './diagnostics';

describe('diagnostics command wrappers', () => {
  it('declares the example command by its dotted name and maps it to the wire name', () => {
    expect(commandNames).toEqual(['diagnostics.ping']);
    expect(wireName('diagnostics.ping')).toBe('diagnostics_ping');
    expect(wireName('project.trust.set')).toBe('project_trust_set');
  });

  it('invokes the wire name with the request and returns the typed result', async () => {
    const calls: Array<{ cmd: string; args: unknown }> = [];
    const result = await ping(
      (cmd, args) => {
        calls.push({ cmd, args });
        return Promise.resolve({ reply: 'pong', message: 'hello' } satisfies PingResult);
      },
      { message: 'hello' },
    );
    expect(result).toEqual({ reply: 'pong', message: 'hello' });
    expect(calls).toEqual([{ cmd: 'diagnostics_ping', args: { request: { message: 'hello' } } }]);
  });

  it('rejects a result that does not have the contract shape', async () => {
    await expect(ping(() => Promise.resolve({ reply: 7 }), { message: 'x' })).rejects.toThrow(
      /diagnostics\.ping/,
    );
  });

  it('delivers typed events and drops malformed payloads', async () => {
    let handler: ((event: { payload: unknown }) => void) | undefined;
    const unlisten = () => undefined;
    const received: PingEvent[] = [];
    const stop = await listenPing(
      (name, h) => {
        expect(name).toBe(PING_EVENT);
        handler = h;
        return Promise.resolve(unlisten);
      },
      (event) => received.push(event),
    );
    handler?.({ payload: { message: 'hello', sequence: 1 } });
    handler?.({ payload: { message: 3 } });
    expect(received).toEqual([{ message: 'hello', sequence: 1 }]);
    expect(stop).toBe(unlisten);
  });
});
