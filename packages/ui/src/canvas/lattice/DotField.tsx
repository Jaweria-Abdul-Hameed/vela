import { Canvas, useFrame, useThree } from '@react-three/fiber';
import { forwardRef, useEffect, useImperativeHandle, useMemo, useRef } from 'react';
import * as THREE from 'three';
import { FrameDriver, bindVisibility } from './driver';
import { DotSimulation, generateLattice } from './simulation';

/** Imperative interface of the canvas (shared contract; scene methods are filled in by U03 and later). */
export interface CanvasHandle {
  setScene: (scene: unknown) => void;
  pushEffect: (effect: unknown) => void;
  setQuality: (quality: 'full' | 'balanced' | 'efficiency') => void;
  suspend: () => void;
  resume: () => void;
  focusNode: (id: string) => void;
}

const SPACING = { full: 22, balanced: 28, efficiency: 40 } as const;

const VERTEX = `
attribute float aWeight;
uniform float uSize;
varying float vWeight;
void main() {
  vWeight = aWeight;
  gl_PointSize = uSize * (1.0 + 0.6 * aWeight);
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}`;

const FRAGMENT = `
uniform vec3 uColor;
uniform float uBase;
varying float vWeight;
void main() {
  float d = length(gl_PointCoord - vec2(0.5));
  if (d > 0.5) discard;
  gl_FragColor = vec4(uColor, uBase + (1.0 - uBase) * vWeight);
}`;

/** The theme color as a three color, read from the design tokens, never hard-coded. */
function tokenColor(name: string, fallbackToken: string): THREE.Color {
  const raw = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return new THREE.Color(raw || fallbackToken);
}

interface FieldProps {
  quality: keyof typeof SPACING;
  driverRef: React.MutableRefObject<FrameDriver | null>;
  simRef: React.MutableRefObject<DotSimulation | null>;
  reducedMotion: boolean;
}

function Field({ quality, driverRef, simRef, reducedMotion }: FieldProps) {
  const { size, invalidate, gl } = useThree();

  const sim = useMemo(() => {
    const rest = generateLattice({
      width: size.width,
      height: size.height,
      spacing: SPACING[quality],
    });
    return new DotSimulation(rest);
  }, [size.width, size.height, quality]);

  const material = useMemo(
    () =>
      new THREE.ShaderMaterial({
        vertexShader: VERTEX,
        fragmentShader: FRAGMENT,
        transparent: true,
        depthWrite: false,
        uniforms: {
          uSize: { value: 2.2 },
          uBase: { value: 0.28 },
          uColor: { value: tokenColor('--vela-text-secondary', 'gray') },
        },
      }),
    [],
  );

  const geometry = useMemo(() => {
    const g = new THREE.BufferGeometry();
    g.setAttribute('position', new THREE.BufferAttribute(new Float32Array(sim.count * 3), 3));
    g.setAttribute('aWeight', new THREE.BufferAttribute(sim.weight, 1));
    return g;
  }, [sim]);

  const write = (): void => {
    const position = geometry.getAttribute('position') as THREE.BufferAttribute;
    const out = position.array as Float32Array;
    for (let i = 0; i < sim.count; i += 1) {
      out[i * 3] = sim.rest[i * 2]! + sim.offset[i * 2]! - size.width / 2;
      out[i * 3 + 1] = -(sim.rest[i * 2 + 1]! + sim.offset[i * 2 + 1]!) + size.height / 2;
    }
    position.needsUpdate = true;
    (geometry.getAttribute('aWeight') as THREE.BufferAttribute).needsUpdate = true;
  };

  useEffect(() => {
    simRef.current = sim;
    const driver = new FrameDriver({
      invalidate,
      advance: (dt) => {
        const moving = sim.step(dt);
        write();
        return moving;
      },
      reducedMotion: () => reducedMotion,
    });
    driverRef.current = driver;
    write();
    driver.request();
    const unbind = bindVisibility(driver, {
      hidden: () => document.hidden,
      focused: () => document.hasFocus(),
      onChange: (listener) => {
        document.addEventListener('visibilitychange', listener);
        window.addEventListener('focus', listener);
        window.addEventListener('blur', listener);
        return () => {
          document.removeEventListener('visibilitychange', listener);
          window.removeEventListener('focus', listener);
          window.removeEventListener('blur', listener);
        };
      },
    });
    const canvas = gl.domElement;
    const move = (e: PointerEvent): void => {
      const r = canvas.getBoundingClientRect();
      sim.setPointer({ x: e.clientX - r.left, y: e.clientY - r.top });
      driver.request();
    };
    const leave = (): void => {
      sim.setPointer(null);
      driver.request();
    };
    window.addEventListener('pointermove', move);
    document.addEventListener('pointerleave', leave);
    return () => {
      unbind();
      window.removeEventListener('pointermove', move);
      document.removeEventListener('pointerleave', leave);
      driver.suspend();
      driverRef.current = null;
      simRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sim, reducedMotion]);

  // Dispose GPU resources when the field is replaced or unmounted.
  useEffect(() => () => material.dispose(), [material]);
  useEffect(() => () => geometry.dispose(), [geometry]);

  useFrame((state) => driverRef.current?.onFrame(state.clock.elapsedTime));

  return (
    <points frustumCulled={false}>
      <primitive object={geometry} attach="geometry" />
      <primitive object={material} attach="material" />
    </points>
  );
}

export interface DotFieldProps {
  quality?: keyof typeof SPACING;
  reducedMotion?: boolean;
}

/** The single WebGL canvas: a sparse dot lattice that renders only on demand. DOM owns all text and controls. */
export const DotField = forwardRef<CanvasHandle, DotFieldProps>(function DotField(
  { quality = 'balanced', reducedMotion = false },
  ref,
) {
  const driverRef = useRef<FrameDriver | null>(null);
  const simRef = useRef<DotSimulation | null>(null);

  useImperativeHandle(
    ref,
    () => ({
      setScene: () => {},
      pushEffect: () => {},
      setQuality: () => {},
      suspend: () => driverRef.current?.suspend(),
      resume: () => driverRef.current?.resume(),
      focusNode: () => {},
    }),
    [],
  );

  return (
    <Canvas
      frameloop="demand"
      dpr={[1, 1.5]}
      orthographic
      camera={{ position: [0, 0, 10], zoom: 1 }}
      gl={{ alpha: true, antialias: false, powerPreference: 'low-power' }}
      aria-hidden="true"
      style={{ position: 'absolute', inset: 0, pointerEvents: 'none' }}
    >
      <Field
        quality={quality}
        driverRef={driverRef}
        simRef={simRef}
        reducedMotion={reducedMotion}
      />
    </Canvas>
  );
});
