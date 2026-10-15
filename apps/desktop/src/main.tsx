import { StrictMode, useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { AmbientField } from '@vela/ui/src/canvas/ambient';
import { GraphCanvas } from '@vela/ui/src/canvas/graph';
import { DotField } from '@vela/ui/src/canvas/lattice';
import { isGraphicsMode, suggest, type GraphicsMode } from '@vela/ui/src/canvas/quality';
import '@vela/ui/src/design/tokens.css';
import '@vela/ui/src/design/base.css';
import { Shell } from '@vela/ui/src/design';
import { tauriApi } from './console/api';
import { Console } from './console/Console';
import { onCoreEvents, snapshotCore, startSync } from './ipc';
import { useCoreStore } from './state';
import './styles.css';

interface BatteryLike {
  level: number;
  charging: boolean;
}

/** The canvases and the console, with the graphics mode the user chose (Balanced until it loads). */
function Root() {
  const [mode, setMode] = useState<GraphicsMode>('balanced');
  const [saver, setSaver] = useState(false);
  const [dismissed, setDismissed] = useState(false);
  const [reduced] = useState(
    () =>
      typeof window.matchMedia === 'function' &&
      window.matchMedia('(prefers-reduced-motion: reduce)').matches,
  );

  useEffect(() => {
    void tauriApi
      .settingsGet(null)
      .then((s) => {
        const stored = s['graphics_mode'];
        if (isGraphicsMode(stored)) setMode(stored);
      })
      .catch(() => {});
    const nav = navigator as Navigator & { getBattery?: () => Promise<BatteryLike> };
    void nav
      .getBattery?.()
      .then((b) => setSaver(!b.charging && b.level < 0.2))
      .catch(() => {});
  }, []);

  const change = (next: GraphicsMode) => {
    setMode(next);
    setDismissed(false);
    void tauriApi.settingsSet(null, 'graphics_mode', next).catch(() => {});
  };
  const suggestion = dismissed
    ? null
    : suggest(mode, { fps: null, batterySaver: saver, sampleSeconds: 0 });

  return (
    <>
      <AmbientField mode={mode} reducedMotion={reduced} />
      <DotField quality={mode} reducedMotion={reduced} />
      <Shell>
        <Console
          api={tauriApi}
          Graph={GraphCanvas}
          graphics={{
            mode,
            onChange: change,
            suggestion,
            onDismissSuggestion: () => setDismissed(true),
          }}
        />
      </Shell>
    </>
  );
}

const container = document.getElementById('root');
if (container === null) throw new Error('index.html has no #root element');
createRoot(container).render(
  <StrictMode>
    <Root />
  </StrictMode>,
);

// Keep the store in step with the core. Outside the Tauri shell (a plain browser during development) there is no core.
const store = useCoreStore.getState();
void startSync({
  getSnapshot: snapshotCore,
  listen: onCoreEvents,
  store: {
    lastSeq: () => useCoreStore.getState().lastSeq,
    applySnapshot: store.applySnapshot,
    applyEvent: store.applyEvent,
    setStatus: store.setStatus,
  },
}).catch((error: unknown) => {
  console.warn('the core is not reachable; running without it', error);
});
