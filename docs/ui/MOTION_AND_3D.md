# Motion, 3D, and Reactive Environment

## Philosophy

Motion communicates topology, causality, focus, and state. If an
animation does not improve comprehension or atmosphere without harming
performance, remove it.

## Rendering split

Prefer DOM/CSS for text-heavy interactive surfaces. Use WebGL/Canvas for
the spatial graph/background only where it produces meaningful
quality/performance. Avoid rendering crisp body text inside WebGL unless
necessary.

## Cursor-reactive dot field

Each dot has: - rest position, - current position, - velocity/spring
state, - base opacity/size, - influence weight.

Pointer computes local influence radius. Response is bounded and smooth.
Avoid expensive all-pairs work; use shader math, spatial partitioning,
or GPU-friendly field evaluation.

Behavior: - pointer motion creates gentle displacement, - nearest dots
brighten/scale slightly, - response falls off smoothly, - dots spring
back, - no chaotic random motion.

## Ambient field

Use large blurred gradient primitives/shader field. Motion period should
be slow enough not to distract. Avoid large CPU-bound blur animations.

## Spatial graph

Graph layout must be stable. Nodes should not constantly reflow when
statuses change. Dependency topology changes animate deliberately.

Interactions: - wheel/pinch zoom, - pointer drag pan, - node drag only
if semantics allow, - click focus, - keyboard focus alternative, - reset
view, - fit graph.

## Camera transitions

Use critically damped/spring-like easing with bounded duration. Respect
reduced motion by crossfading/instant reframing.

## Event animation mapping

`WorkerStarted` → local ripple\
`TestGateFinished(pass)` → brief cool wave\
`ReviewStarted` → restrained vela/ring\
`ReviewFindingRaised` → attached marker\
`ReviewFindingResolved` → marker collapses\
`MergeStarted` → directional edge flow\
`MergeCompleted` → integration pulse\
`TicketReady` → node gently emerges\
`HumanActionRequired` → persistent but non-flashing attention state

## Performance

-   requestAnimationFrame only while visible/active,
-   cap device pixel ratio for heavy canvas,
-   adaptive quality,
-   suspend expensive effects when minimized,
-   lower refresh when unfocused,
-   no unnecessary React rerenders per frame,
-   dispose Three.js resources,
-   profile GPU memory,
-   use instancing for many dots/nodes where appropriate.

## Reduced motion

When enabled: - no camera flight, - no parallax, - no propagating
ripples, - no breathing/pulsing loops, - status changes use short
fades/static indicators, - all information remains equally
understandable.
