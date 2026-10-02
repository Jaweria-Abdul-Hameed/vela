# ADR-019: UI Rendering Architecture (DOM, WebGL Canvas, and Graphics Modes)

## Status

Accepted (2026-10-02). Realizes `UI_UX_SPEC.md`, `MOTION_AND_3D.md`, `PERFORMANCE_BUDGET.md` and ADR-001's GPU-fallback
consequence; does not change them.

## Decision

1.  **Split.** The DOM owns all text, controls, the inspector, timeline, run bar, sheets, command palette, settings, and the
    accessible graph representation. A **single WebGL canvas layer** owns the dot lattice, the ambient gradient field,
    and the graph's nodes and edges (instanced geometry). Node labels are DOM elements positioned by projecting node
    coordinates, so text stays crisp (`MOTION_AND_3D.md`).
2.  **Library boundary.** `packages/ui/src/canvas` is the only module that imports React Three Fiber and Three.js. It
    exposes a small imperative interface (`setScene`, `pushEffect`, `setQuality`, `suspend`, `resume`, `focusNode`) and
    renders with `frameloop="demand"`. The rest of the UI never touches Three.js.
3.  **Event-driven animation.** The UI store turns journal events into effect descriptors (for example ripple at node X);
    effects are pushed into the canvas, which invalidates frames only while an effect, camera move, or pointer interaction
    is active. Idle means zero frame callbacks (budget in `PERFORMANCE_BUDGET.md`).
4.  **Suspension.** On window hidden, minimized, or blurred, the renderer switches to `never` or a reduced rate per the
    quality mode; orchestration is unaffected because it lives in the core.
5.  **Graph layout is computed in Rust** (`vela-domain::layout`: deterministic layered layout from the topological layers
    with barycenter ordering) at run state `ANALYZING` and stored in the graph snapshot, so layout is stable, reproducible,
    and testable. Nodes do not reflow on status changes. User-dragged offsets are UI preferences stored separately.
6.  **Accessibility.** A DOM tree/list view of the graph (ticket, state, blockers, dependents) is always mounted and is the
    keyboard path; the canvas is `aria-hidden`.
7.  **Graphics modes** (`Full`, `Balanced` default, `Efficiency`) set: device-pixel-ratio cap (Full 2, Balanced 1.5,
    Efficiency 1), post-processing (Full bloom-lite, Balanced none beyond the gradient field, Efficiency none), dot
    density, frame cap (Full 60, Balanced 60 when active, Efficiency 30 when active), and idle suspension aggressiveness.
    Automatic adaptation may suggest a mode and never overrides the user's setting. Reduced motion removes camera flight,
    parallax, ripples, and loops regardless of mode.
8.  **WebGL failure fallback.** If WebGL context creation fails, the context is lost and not restored, or software
    rendering is detected, the UI falls back to a static CSS gradient background plus the DOM graph list and a simple SVG
    graph without pointer-reactive dots; the notice is shown once and the mode is recorded.
9.  **Resource discipline.** The canvas module owns disposal of geometries, materials, and textures on unmount and on
    context loss; a test asserts no monotonic growth over repeated mount/unmount (budget table).

## Alternatives rejected

Canvas2D for the whole scene (cannot meet the dot-field and 500-node budget); WebGL text (poor accessibility and crispness);
computing layout in the browser (non-deterministic across runs, harder to test); a second renderer for the fallback beyond
CSS/SVG.

## Consequences

`packages/ui` is split into `design`, `components`, `canvas`, and `store` modules (`IMPLEMENTATION_ARCHITECTURE.md`).
