/** Dot lattice layout and the pointer spring (U02). Pure and CPU-side so it is testable without WebGL. */

export interface LatticeOptions {
  width: number;
  height: number;
  /** Distance between neighbouring dots, in CSS pixels. */
  spacing: number;
}

/** Ordered, sparse lattice: row-major `x, y` pairs, centred in the area. */
export function generateLattice({ width, height, spacing }: LatticeOptions): Float32Array {
  if (!(spacing > 0) || width <= 0 || height <= 0) return new Float32Array(0);
  const cols = Math.max(1, Math.floor(width / spacing));
  const rows = Math.max(1, Math.floor(height / spacing));
  const out = new Float32Array(cols * rows * 2);
  const x0 = (width - (cols - 1) * spacing) / 2;
  const y0 = (height - (rows - 1) * spacing) / 2;
  let i = 0;
  for (let r = 0; r < rows; r += 1) {
    for (let c = 0; c < cols; c += 1) {
      out[i++] = x0 + c * spacing;
      out[i++] = y0 + r * spacing;
    }
  }
  return out;
}

export interface SpringOptions {
  /** Pointer influence radius in pixels (bounded). */
  radius: number;
  /** Largest displacement of any dot in pixels (bounded). */
  maxDisplacement: number;
  stiffness: number;
  damping: number;
  /** Below this displacement and speed a dot is at rest. */
  epsilon: number;
}

export const DEFAULT_SPRING: SpringOptions = {
  radius: 140,
  maxDisplacement: 9,
  stiffness: 90,
  damping: 13,
  epsilon: 0.01,
};

/** Smooth falloff: 1 at the pointer, 0 at `radius`, with zero slope at both ends. */
export function falloff(distance: number, radius: number): number {
  if (distance >= radius) return 0;
  const t = 1 - distance / radius;
  return t * t * (3 - 2 * t);
}

/** Dot positions with spring state. Only dots near the pointer or still moving are touched each step. */
export class DotSimulation {
  readonly count: number;
  /** Current displacement from rest, `dx, dy` per dot. */
  readonly offset: Float32Array;
  /** Brightness weight per dot, 0 to 1 (base opacity is the renderer's). */
  readonly weight: Float32Array;
  private readonly velocity: Float32Array;
  private readonly moving: Uint8Array;
  private pointer: { x: number; y: number } | null = null;

  constructor(
    readonly rest: Float32Array,
    private readonly spring: SpringOptions = DEFAULT_SPRING,
  ) {
    this.count = rest.length / 2;
    this.offset = new Float32Array(rest.length);
    this.velocity = new Float32Array(rest.length);
    this.weight = new Float32Array(this.count);
    this.moving = new Uint8Array(this.count);
  }

  /** The pointer position in lattice coordinates, or null when it left the canvas. */
  setPointer(point: { x: number; y: number } | null): void {
    this.pointer = point;
  }

  /** Advances the springs by `dt` seconds. Returns true while anything is still moving or influenced. */
  step(dtSeconds: number): boolean {
    const dt = Math.min(Math.max(dtSeconds, 0), 1 / 30);
    const { radius, maxDisplacement, stiffness, damping, epsilon } = this.spring;
    let active = false;
    for (let i = 0; i < this.count; i += 1) {
      const ix = i * 2;
      let tx = 0;
      let ty = 0;
      let w = 0;
      if (this.pointer) {
        const dx = this.rest[ix]! - this.pointer.x;
        const dy = this.rest[ix + 1]! - this.pointer.y;
        const d = Math.hypot(dx, dy);
        w = falloff(d, radius);
        if (w > 0 && d > 1e-6) {
          tx = (dx / d) * w * maxDisplacement;
          ty = (dy / d) * w * maxDisplacement;
        }
      }
      this.weight[i] = w;
      if (w === 0 && this.moving[i] === 0) continue;
      const ox = this.offset[ix]!;
      const oy = this.offset[ix + 1]!;
      let vx = this.velocity[ix]!;
      let vy = this.velocity[ix + 1]!;
      vx += (stiffness * (tx - ox) - damping * vx) * dt;
      vy += (stiffness * (ty - oy) - damping * vy) * dt;
      const nx = ox + vx * dt;
      const ny = oy + vy * dt;
      const resting =
        w === 0 &&
        Math.abs(nx) < epsilon &&
        Math.abs(ny) < epsilon &&
        Math.abs(vx) < epsilon &&
        Math.abs(vy) < epsilon;
      if (resting) {
        this.offset[ix] = 0;
        this.offset[ix + 1] = 0;
        this.velocity[ix] = 0;
        this.velocity[ix + 1] = 0;
        this.moving[i] = 0;
      } else {
        this.offset[ix] = nx;
        this.offset[ix + 1] = ny;
        this.velocity[ix] = vx;
        this.velocity[ix + 1] = vy;
        this.moving[i] = 1;
        active = true;
      }
    }
    return active;
  }
}
