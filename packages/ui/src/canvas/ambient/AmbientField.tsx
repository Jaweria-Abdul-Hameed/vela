import { Canvas, useThree } from '@react-three/fiber';
import { useEffect, useMemo, useRef } from 'react';
import * as THREE from 'three';
import { ambientFrameIntervalMs, profileFor } from '../quality';
import { useIdle } from './idle';

const VERTEX = `
varying vec2 vUv;
void main() {
  vUv = uv;
  gl_Position = vec4(position.xy, 0.0, 1.0);
}`;

// Three huge, soft masses drifting on slow orbits. Colours come from tokens (uniforms), nothing is hard-coded.
const FRAGMENT = `
uniform float uTime;
uniform vec3 uA;
uniform vec3 uB;
uniform vec3 uC;
varying vec2 vUv;
float mass(vec2 p, vec2 c, float r) {
  return smoothstep(r, 0.0, distance(p, c));
}
void main() {
  vec2 p = vUv;
  float t = uTime * 0.02;
  vec3 col = uA * mass(p, vec2(0.25 + 0.10 * sin(t), 0.30 + 0.08 * cos(t * 1.3)), 0.75)
           + uB * mass(p, vec2(0.75 + 0.08 * cos(t * 0.8), 0.65 + 0.10 * sin(t)), 0.80)
           + uC * mass(p, vec2(0.50 + 0.12 * sin(t * 0.6), 0.90 + 0.05 * cos(t * 1.1)), 0.70);
  gl_FragColor = vec4(col * 0.10, 1.0);
}`;

function token(name: string): THREE.Color {
  const raw = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return new THREE.Color(raw || 'black');
}

/** Renders the field and asks for another frame at the mode's low frame rate, never faster. */
function Field({ intervalMs, paused }: { intervalMs: number | null; paused: boolean }) {
  const { invalidate } = useThree();
  const material = useMemo(
    () =>
      new THREE.ShaderMaterial({
        vertexShader: VERTEX,
        fragmentShader: FRAGMENT,
        depthTest: false,
        uniforms: {
          uTime: { value: 0 },
          uA: { value: token('--vela-violet') },
          uB: { value: token('--vela-blue') },
          uC: { value: token('--vela-cyan') },
        },
      }),
    [],
  );
  const uniforms = useRef(material.uniforms as { uTime: { value: number } });
  useEffect(() => {
    // One still frame always; moving frames only while the mode allows motion and the page is visible.
    invalidate();
    if (intervalMs === null || paused) return;
    const started = performance.now();
    const id = setInterval(() => {
      if (document.hidden) return;
      uniforms.current.uTime.value = (performance.now() - started) / 1000;
      invalidate();
    }, intervalMs);
    return () => clearInterval(id);
  }, [intervalMs, paused, invalidate]);
  useEffect(() => () => material.dispose(), [material]);
  return (
    <mesh frustumCulled={false}>
      <planeGeometry args={[2, 2]} />
      <primitive object={material} attach="material" />
    </mesh>
  );
}

export interface AmbientFieldProps {
  /** The stored graphics-mode setting. */
  mode: unknown;
  reducedMotion?: boolean;
  /** True while the window is hidden, minimized or has lost focus for long enough. */
  suspended?: boolean;
}

/** Huge soft gradient masses behind the dots. Absent in Efficiency mode; still under reduced motion. */
export function AmbientField({
  mode,
  reducedMotion = false,
  suspended = false,
}: AmbientFieldProps) {
  const profile = profileFor(mode);
  // After the mode's idle time with no input the field stops asking for frames (zero animation-frame callbacks when idle).
  const idle = useIdle(profile.idleSuspendMs, profile.ambient);
  if (!profile.ambient) return null;
  return (
    <Canvas
      frameloop="demand"
      dpr={[1, profile.dprCap]}
      gl={{ alpha: false, antialias: false, powerPreference: 'low-power' }}
      aria-hidden="true"
      style={{ position: 'absolute', inset: 0, pointerEvents: 'none' }}
    >
      <Field
        intervalMs={ambientFrameIntervalMs(profile, reducedMotion)}
        paused={suspended || idle}
      />
    </Canvas>
  );
}
