import { useEffect, useState } from 'react';

/** Reduced motion (U14): the OS preference or the Vela setting, whichever asks for less. */
export function useReducedMotion(setting: boolean | 'system' = 'system'): boolean {
  const query = '(prefers-reduced-motion: reduce)';
  const read = () => typeof window.matchMedia === 'function' && window.matchMedia(query).matches;
  const [os, setOs] = useState(read);
  useEffect(() => {
    if (typeof window.matchMedia !== 'function') return;
    const m = window.matchMedia(query);
    const on = () => setOs(m.matches);
    m.addEventListener('change', on);
    return () => m.removeEventListener('change', on);
  }, []);
  return setting === true || (setting === 'system' && os);
}

export type WebglVerdict = 'ok' | 'unavailable' | 'software';

/** Whether a real GPU context can be had. Software renderers (SwiftShader, llvmpipe, WARP) count as a fallback case. */
export function probeWebgl(
  create: () => WebGLRenderingContext | null = defaultContext,
): WebglVerdict {
  let gl: WebGLRenderingContext | null;
  try {
    gl = create();
  } catch {
    return 'unavailable';
  }
  if (!gl) return 'unavailable';
  const ext = gl.getExtension('WEBGL_debug_renderer_info');
  const name = ext ? String(gl.getParameter(ext.UNMASKED_RENDERER_WEBGL)) : '';
  return /swiftshader|llvmpipe|software|basic render/i.test(name) ? 'software' : 'ok';
}

function defaultContext(): WebGLRenderingContext | null {
  return document.createElement('canvas').getContext('webgl');
}

/** Tracks context loss on a canvas element so the DOM fallback can take over. */
export function useContextLoss(el: HTMLCanvasElement | null): boolean {
  const [lost, setLost] = useState(false);
  useEffect(() => {
    if (!el) return;
    const on = (e: Event) => {
      e.preventDefault();
      setLost(true);
    };
    const back = () => setLost(false);
    el.addEventListener('webglcontextlost', on);
    el.addEventListener('webglcontextrestored', back);
    return () => {
      el.removeEventListener('webglcontextlost', on);
      el.removeEventListener('webglcontextrestored', back);
    };
  }, [el]);
  return lost;
}

export interface FallbackNode {
  id: string;
  label: string;
  state: string;
}

/** The list view standing in for the graph when WebGL is unusable: every ticket, its state, selectable by keyboard. */
export function TicketList({
  nodes,
  selected,
  onSelect,
}: {
  nodes: FallbackNode[];
  selected: string | null;
  onSelect: (id: string) => void;
}) {
  return (
    <ul aria-label="Tickets (list view)">
      {nodes.map((n) => (
        <li key={n.id}>
          <button type="button" aria-pressed={n.id === selected} onClick={() => onSelect(n.id)}>
            {n.label}
          </button>{' '}
          {n.state}
        </li>
      ))}
    </ul>
  );
}
