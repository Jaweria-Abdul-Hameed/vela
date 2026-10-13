import { Canvas, useThree } from '@react-three/fiber';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import * as THREE from 'three';
import {
  RESET_VIEW,
  ease,
  fit,
  focusOn,
  keyboardView,
  panBy,
  settled,
  toScreen,
  zoomAt,
  type View,
  type Viewport,
} from './camera';
import { NODE_VISUALS, type NodeState, type Shape } from './visuals';

export interface GraphNode {
  id: string;
  x: number;
  y: number;
  label: string;
  state: NodeState;
}

export interface GraphEdge {
  from: string;
  to: string;
  origin: 'authored' | 'analyst';
}

export interface GraphCanvasProps {
  nodes: GraphNode[];
  edges: GraphEdge[];
  selected?: string | null;
  onSelect?: (id: string | null) => void;
  reducedMotion?: boolean;
  /** Pixel size; defaults to the container's size. */
  size?: Viewport;
}

const GLYPH: Record<Shape, string> = {
  ring: '◯',
  dot: '●',
  diamond: '◆',
  hex: '⬢',
  square: '■',
  cross: '✕',
};

function tokenColor(name: string): THREE.Color {
  const raw = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return new THREE.Color(raw || 'gray');
}

/** The R3F layer: edges and nodes in graph units, moved by the camera as a group. Only the group's transform changes with the view. */
function Layer({
  nodes,
  edges,
  view,
  selected,
}: {
  nodes: GraphNode[];
  edges: GraphEdge[];
  view: View;
  selected?: string | null;
}) {
  const { invalidate } = useThree();
  const nodeById = useMemo(() => new Map(nodes.map((n) => [n.id, n])), [nodes]);

  const edgeGeometry = useMemo(() => {
    const pts: number[] = [];
    for (const e of edges) {
      const a = nodeById.get(e.from);
      const b = nodeById.get(e.to);
      if (a && b) pts.push(a.x, a.y, 0, b.x, b.y, 0);
    }
    const g = new THREE.BufferGeometry();
    g.setAttribute('position', new THREE.Float32BufferAttribute(pts, 3));
    return g;
  }, [edges, nodeById]);

  const nodeMesh = useMemo(() => {
    const geometry = new THREE.CircleGeometry(1, 20);
    const material = new THREE.MeshBasicMaterial({ transparent: true });
    const mesh = new THREE.InstancedMesh(geometry, material, Math.max(nodes.length, 1));
    mesh.frustumCulled = false;
    const m = new THREE.Matrix4();
    nodes.forEach((n, i) => {
      const v = NODE_VISUALS[n.state];
      const radius = 7 + v.light * 6 + (n.id === selected ? 4 : 0);
      m.compose(
        new THREE.Vector3(n.x, n.y, 0.1),
        new THREE.Quaternion(),
        new THREE.Vector3(radius, radius, 1),
      );
      mesh.setMatrixAt(i, m);
      mesh.setColorAt(i, tokenColor(v.token).multiplyScalar(0.35 + v.light * 0.65));
    });
    mesh.count = nodes.length;
    mesh.instanceMatrix.needsUpdate = true;
    if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
    return mesh;
  }, [nodes, selected]);

  const edgeMaterial = useMemo(
    () =>
      new THREE.LineBasicMaterial({
        color: tokenColor('--vela-text-muted'),
        transparent: true,
        opacity: 0.45,
      }),
    [],
  );

  // Frames are requested only when something changed: the view, the graph or the selection.
  useEffect(() => {
    invalidate();
  }, [view, nodes, edges, selected, invalidate]);

  // Dispose GPU resources when the data is replaced or the layer unmounts.
  useEffect(
    () => () => {
      edgeGeometry.dispose();
    },
    [edgeGeometry],
  );
  useEffect(
    () => () => {
      nodeMesh.geometry.dispose();
      (nodeMesh.material as THREE.Material).dispose();
      nodeMesh.dispose();
    },
    [nodeMesh],
  );
  useEffect(() => () => edgeMaterial.dispose(), [edgeMaterial]);

  return (
    <group
      position={[-view.x * view.zoom, view.y * view.zoom, 0]}
      scale={[view.zoom, -view.zoom, 1]}
    >
      <lineSegments geometry={edgeGeometry} material={edgeMaterial} />
      <primitive object={nodeMesh} />
    </group>
  );
}

/** The project graph as a constellation: nodes and fine edges on one canvas, pan/zoom/fit/reset, DOM labels over it. */
export function GraphCanvas({
  nodes,
  edges,
  selected = null,
  onSelect,
  reducedMotion = false,
  size,
}: GraphCanvasProps) {
  const box = useRef<HTMLDivElement>(null);
  const [measured, setMeasured] = useState<Viewport>({ width: 800, height: 600 });
  const vp = size ?? measured;
  const [view, setView] = useState<View>(RESET_VIEW);
  const [target, setTarget] = useState<View>(RESET_VIEW);

  useEffect(() => {
    const el = box.current;
    if (!el || size) return;
    const read = () =>
      setMeasured({ width: el.clientWidth || 800, height: el.clientHeight || 600 });
    read();
    const ro = new ResizeObserver(read);
    ro.observe(el);
    return () => ro.disconnect();
  }, [size]);

  const fitAll = useCallback(
    () =>
      fit(
        nodes.map((n) => ({ x: n.x, y: n.y })),
        vp,
      ),
    [nodes, vp],
  );

  // Start fitted, and re-fit when the graph itself is replaced (derived during render, not in an effect).
  const [seenNodes, setSeenNodes] = useState<GraphNode[] | null>(null);
  if (seenNodes !== nodes) {
    setSeenNodes(nodes);
    const v = fitAll();
    setTarget(v);
    setView(v);
  }

  // Ease toward the target only while it is not reached. Under reduced motion the shown view is the target itself.
  useEffect(() => {
    if (reducedMotion || settled(view, target)) return;
    let last = performance.now();
    const id = requestAnimationFrame((now) => {
      setView((v) => ease(v, target, (now - last) / 1000));
      last = now;
    });
    return () => cancelAnimationFrame(id);
  }, [view, target, reducedMotion]);
  const shown = reducedMotion ? target : view;

  const drag = useRef<{ x: number; y: number } | null>(null);

  const jump = (v: View) => {
    setTarget(v);
  };

  return (
    <div
      ref={box}
      role="application"
      aria-label="Project graph. Arrow keys pan, plus and minus zoom, zero resets, F fits."
      tabIndex={0}
      style={{
        position: 'relative',
        width: '100%',
        height: '100%',
        overflow: 'hidden',
        touchAction: 'none',
      }}
      onKeyDown={(e) => {
        const next = keyboardView(target, vp, e.key, fitAll);
        if (next) {
          e.preventDefault();
          jump(next);
        } else if (e.key === 'Escape') {
          onSelect?.(null);
        }
      }}
      onWheel={(e) => {
        const r = e.currentTarget.getBoundingClientRect();
        jump(
          zoomAt(
            target,
            vp,
            { x: e.clientX - r.left, y: e.clientY - r.top },
            e.deltaY < 0 ? 1.12 : 1 / 1.12,
          ),
        );
      }}
      onPointerDown={(e) => {
        drag.current = { x: e.clientX, y: e.clientY };
        e.currentTarget.setPointerCapture?.(e.pointerId);
      }}
      onPointerMove={(e) => {
        if (!drag.current) return;
        const [dx, dy] = [e.clientX - drag.current.x, e.clientY - drag.current.y];
        drag.current = { x: e.clientX, y: e.clientY };
        const moved = panBy(target, dx, dy);
        setTarget(moved);
        setView(moved);
      }}
      onPointerUp={() => {
        drag.current = null;
      }}
    >
      <Canvas
        frameloop="demand"
        orthographic
        dpr={[1, 1.5]}
        camera={{ position: [0, 0, 10], zoom: 1 }}
        gl={{ alpha: true, antialias: true, powerPreference: 'low-power' }}
        aria-hidden="true"
        style={{ position: 'absolute', inset: 0 }}
      >
        <Layer nodes={nodes} edges={edges} view={shown} selected={selected} />
      </Canvas>
      {/* DOM owns the text and the controls: every node is a real button, reachable by keyboard, positioned from the same view. */}
      {nodes.map((n) => {
        const p = toScreen(shown, vp, n);
        const v = NODE_VISUALS[n.state];
        return (
          <button
            key={n.id}
            type="button"
            aria-pressed={n.id === selected}
            aria-label={`${n.label}, ${v.label}`}
            onClick={() => {
              onSelect?.(n.id);
              jump(focusOn(target, n));
            }}
            style={{
              position: 'absolute',
              left: p.x,
              top: p.y,
              transform: 'translate(-50%, 14px)',
              background: 'transparent',
              border: 'none',
              color: 'var(--vela-text-primary)',
              font: 'inherit',
              fontSize: 'var(--vela-text-meta)',
              cursor: 'pointer',
              whiteSpace: 'nowrap',
            }}
          >
            <span aria-hidden="true" style={{ color: `var(${v.token})` }}>
              {GLYPH[v.shape]}
            </span>{' '}
            {n.label}
          </button>
        );
      })}
    </div>
  );
}
