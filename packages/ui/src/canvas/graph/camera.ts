/** The graph camera (U05): pure view maths for pan, zoom, fit, reset and focus. Everything is in graph units; the renderer and the
 *  DOM labels both read the same {@link View}, so they cannot drift apart. */

export interface View {
  /** Graph x at the centre of the viewport. */
  x: number;
  /** Graph y at the centre of the viewport. */
  y: number;
  /** Pixels per graph unit. */
  zoom: number;
}

export interface Viewport {
  width: number;
  height: number;
}

export interface Point {
  x: number;
  y: number;
}

export const MIN_ZOOM = 0.15;
export const MAX_ZOOM = 4;
export const RESET_VIEW: View = { x: 0, y: 0, zoom: 1 };

export function clampZoom(zoom: number): number {
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, Number.isFinite(zoom) ? zoom : 1));
}

/** Graph point to screen pixels. */
export function toScreen(view: View, vp: Viewport, p: Point): Point {
  return {
    x: (p.x - view.x) * view.zoom + vp.width / 2,
    y: (p.y - view.y) * view.zoom + vp.height / 2,
  };
}

/** Screen pixels to graph point. */
export function toGraph(view: View, vp: Viewport, s: Point): Point {
  return {
    x: (s.x - vp.width / 2) / view.zoom + view.x,
    y: (s.y - vp.height / 2) / view.zoom + view.y,
  };
}

/** Zooms by `factor` keeping the graph point under `anchor` (a screen point) fixed. */
export function zoomAt(view: View, vp: Viewport, anchor: Point, factor: number): View {
  const zoom = clampZoom(view.zoom * factor);
  const before = toGraph(view, vp, anchor);
  const next: View = { ...view, zoom };
  const after = toGraph(next, vp, anchor);
  return { x: view.x + (before.x - after.x), y: view.y + (before.y - after.y), zoom };
}

/** Pans by a screen-pixel drag. */
export function panBy(view: View, dx: number, dy: number): View {
  return { ...view, x: view.x - dx / view.zoom, y: view.y - dy / view.zoom };
}

/** The view that shows every point with `margin` pixels around it. An empty graph resets. */
export function fit(points: Point[], vp: Viewport, margin = 64): View {
  if (points.length === 0) return RESET_VIEW;
  const xs = points.map((p) => p.x);
  const ys = points.map((p) => p.y);
  const [minX, maxX] = [Math.min(...xs), Math.max(...xs)];
  const [minY, maxY] = [Math.min(...ys), Math.max(...ys)];
  const w = Math.max(maxX - minX, 1);
  const h = Math.max(maxY - minY, 1);
  const zoom = clampZoom(Math.min((vp.width - margin * 2) / w, (vp.height - margin * 2) / h, 1.5));
  return { x: (minX + maxX) / 2, y: (minY + maxY) / 2, zoom };
}

/** The view that centres `p` at a comfortable zoom (node focus). */
export function focusOn(view: View, p: Point): View {
  return { x: p.x, y: p.y, zoom: clampZoom(Math.max(view.zoom, 1.2)) };
}

/** Moves `from` toward `to` by critically-damped easing; `dt` in seconds, `halfLife` in seconds. Returns `to` when close. */
export function ease(from: View, to: View, dt: number, halfLife = 0.12): View {
  const k = 1 - Math.pow(0.5, Math.max(dt, 0) / halfLife);
  const next: View = {
    x: from.x + (to.x - from.x) * k,
    y: from.y + (to.y - from.y) * k,
    zoom: from.zoom + (to.zoom - from.zoom) * k,
  };
  return settled(next, to) ? to : next;
}

export function settled(a: View, b: View): boolean {
  return (
    Math.abs(a.x - b.x) < 0.05 && Math.abs(a.y - b.y) < 0.05 && Math.abs(a.zoom - b.zoom) < 0.002
  );
}

/** Keyboard camera controls: arrows pan, plus/minus zoom, 0 resets. Returns the new view, or null for an unhandled key. */
export function keyboardView(
  view: View,
  vp: Viewport,
  key: string,
  fitTo: () => View,
): View | null {
  const step = 60;
  switch (key) {
    case 'ArrowLeft':
      return panBy(view, step, 0);
    case 'ArrowRight':
      return panBy(view, -step, 0);
    case 'ArrowUp':
      return panBy(view, 0, step);
    case 'ArrowDown':
      return panBy(view, 0, -step);
    case '+':
    case '=':
      return zoomAt(view, vp, { x: vp.width / 2, y: vp.height / 2 }, 1.2);
    case '-':
      return zoomAt(view, vp, { x: vp.width / 2, y: vp.height / 2 }, 1 / 1.2);
    case '0':
      return RESET_VIEW;
    case 'f':
      return fitTo();
    default:
      return null;
  }
}
