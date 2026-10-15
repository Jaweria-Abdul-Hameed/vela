import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { AmbientField } from '@vela/ui/src/canvas/ambient';
import { GraphCanvas, type GraphCanvasProps } from '@vela/ui/src/canvas/graph';
import { DotField } from '@vela/ui/src/canvas/lattice';
import '@vela/ui/src/design/tokens.css';
import '@vela/ui/src/design/base.css';
import { App } from '../app';
import { Console } from '../console/Console';
import '../styles.css';
import { fixtureApi, isVisualState } from './fixtures';

/** The scenes of the visual-regression suite (U18): the real components over a fixed fixture, with motion off so a frame is the
 *  same frame every time. */
const requested = new URLSearchParams(window.location.search).get('state');
const state = isVisualState(requested) ? requested : 'home';

const idleCore = {
  ping: () => Promise.resolve({ reply: 'fixture', sequence: 0 }),
  onPing: () => Promise.resolve(() => {}),
};

function StillGraph(props: GraphCanvasProps) {
  return <GraphCanvas {...props} reducedMotion />;
}

const container = document.getElementById('root');
if (container === null) throw new Error('visual.html has no #root element');
createRoot(container).render(
  <StrictMode>
    <AmbientField mode="balanced" reducedMotion />
    <DotField quality="balanced" reducedMotion />
    <App ipc={idleCore as never}>
      <Console api={fixtureApi(state)} Graph={StillGraph} pollMs={600_000} />
    </App>
  </StrictMode>,
);
