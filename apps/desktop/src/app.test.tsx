// @vitest-environment jsdom
import { act, cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { PingEvent, PingRequest, PingResult } from '@vela/contracts/commands/diagnostics';
import { App, type CoreIpc } from './app';

afterEach(cleanup);

function fakeIpc() {
  let emit: ((event: PingEvent) => void) | undefined;
  let unlistened = false;
  const requests: PingRequest[] = [];
  const ipc: CoreIpc = {
    ping: (request) => {
      requests.push(request);
      return Promise.resolve({ reply: 'pong', message: request.message } satisfies PingResult);
    },
    onPing: (handler) => {
      emit = handler;
      return Promise.resolve(() => {
        unlistened = true;
      });
    },
  };
  return {
    ipc,
    requests,
    emit: (event: PingEvent) => emit?.(event),
    wasUnlistened: () => unlistened,
  };
}

describe('renderer smoke test', () => {
  it('mounts, shows the typed ping result and receives the event', async () => {
    const fake = fakeIpc();
    render(<App ipc={fake.ipc} />);

    expect((await screen.findByRole('status', { name: 'Core connection' })).textContent).toContain(
      'Core replied: pong',
    );
    expect(fake.requests).toEqual([{ message: 'hello from the renderer' }]);

    act(() => {
      fake.emit({ message: 'hello from the renderer', sequence: 1 });
    });
    expect((await screen.findByRole('status', { name: 'Core event' })).textContent).toContain(
      'Event 1 received',
    );
  });

  it('shows the error instead of crashing when the core is unreachable', async () => {
    const ipc: CoreIpc = {
      ping: () => Promise.reject(new Error('core unavailable')),
      onPing: () => Promise.resolve(() => undefined),
    };
    render(<App ipc={ipc} />);
    expect((await screen.findByRole('alert')).textContent).toContain('core unavailable');
  });

  it('stops listening when it unmounts', async () => {
    const fake = fakeIpc();
    const view = render(<App ipc={fake.ipc} />);
    await screen.findByText('Core replied: pong');
    view.unmount();
    expect(fake.wasUnlistened()).toBe(true);
  });
});
