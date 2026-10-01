# Accessibility

Vela's visual ambition does not override usability.

Requirements: - full keyboard access to project selection, graph nodes,
inspectors, run controls, dialogs; - visible focus indicator independent
of glow; - semantic DOM equivalents for graph information; - list/tree
fallback view of dependency graph; - reduced-motion support; -
sufficient text contrast; - status never encoded only by hue; - scalable
text without clipping; - screen-reader labels for controls and ticket
state; - Stop All reachable without precise pointer interaction; - no
rapid flashing; - animations do not trap focus; - shortcuts discoverable
and remappable where feasible.

The graph's accessible representation should expose ticket, state,
blockers, and children/dependents in a navigable hierarchy/list.
