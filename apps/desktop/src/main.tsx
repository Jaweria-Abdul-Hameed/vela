import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './app';
import { onCoreEvents, snapshotCore, startSync } from './ipc';
import { useCoreStore } from './state';
import './styles.css';

const container = document.getElementById('root');
if (container === null) throw new Error('index.html has no #root element');
createRoot(container).render(
  <StrictMode>
    <App />
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
