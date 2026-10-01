# UI / UX Specification --- The Visual Source of Truth

## 1. Creative direction

Vela must **not** look like a conventional developer dashboard wearing
a dark theme. The visual language is a spatial, near-black, luminous
environment inspired by the supplied Google Stitch reference: black
negative space, a precise field of tiny dots, large diffused
violet/electric-blue/cyan light forms, extremely restrained translucent
surfaces, oversized clean typography, and fluid cursor-responsive
motion.

The canvas is the hero. Glass is secondary.

Avoid: - permanent left admin sidebar as the dominant composition, -
grids of generic metric cards, - thick neon borders, - excessive
bloom, - gamer/RGB aesthetic, - cyberpunk cliché, - glass on every
surface, - dense telemetry on first view, - arbitrary 3D objects, -
animations unrelated to state.

## 2. Visual hierarchy

Order of visual importance: 1. spatial project/issue constellation, 2.
current execution focus, 3. ambient environment, 4. contextual floating
controls, 5. detailed logs/telemetry on demand.

The default screen should be calm even while significant work happens.

## 3. Home

Near-black full-window canvas. Top-level branding is small and quiet.
Center composition uses large typography and one primary project action.

Suggested hierarchy:

``` text
                         VELA

                What are we building?

          [ Drop a project folder here ]

        Recent Project     Recent Project
```

No conventional dashboard chrome. Recent projects appear as restrained
floating pills/cards or constellation anchors.

## 4. Project universe

Selecting a project transitions---not hard-cuts---into a spatial
dependency canvas.

Each issue is a node. Edges are dependency relationships. The graph can
pan, zoom, and be dragged. The camera should preserve orientation and
avoid motion sickness.

Node states are communicated through shape, motion, label, and
light---not color alone.

Examples: - pending: quiet/dim, - ready: slightly clearer edge, -
running: slow breathing aura, - testing: subtle orbital/tick motion, -
reviewing: slow outer-ring motion, - fixing: nested pulse, - merging:
edge flow toward integration, - done: settled, lower-energy luminous
state, - blocked: visibly constrained edge/lock glyph, - failed: warm
restrained accent, never full-screen alarm red, - needs human: focused
attention treatment.

## 5. Reactive dot field

The background contains a regular/sparse micro-dot lattice.

Cursor influence: - dots within a radius respond smoothly, - small
positional displacement, - slight scale/brightness increase, - subtle
local cyan/violet light, - spring return after pointer passes.

The effect must feel like a physical field, not a particle explosion.

The dot grid also reacts to system events: - worker launch: faint
propagating ripple from node, - tests pass: subtle local wave, -
dependency unlock: connecting edge illuminates then settles, - merge:
directional energy moves toward integration node, - completion:
low-energy settling wave.

All effects have reduced-motion alternatives.

## 6. Ambient gradients

Behind the dots are enormous blurred gradient masses. Primary
spectrum: - deep violet, - electric indigo/blue, - cool cyan.

Gradients should occupy huge regions and bleed beyond viewport
boundaries. They move extremely slowly and may respond subtly to
camera/cursor/run state.

Black remains dominant. Color is light, not paint.

## 7. Glass

Glass surfaces appear only for information that floats above the spatial
canvas: - issue inspector, - command palette, - intervention dialog, -
settings popover, - run controls, - notifications.

Characteristics: - translucent near-black, - backdrop blur, - very
subtle border, - large radius, - low-contrast shadow, - environmental
color bleeding through.

Never stack multiple glass layers unnecessarily.

## 8. Issue focus transition

Click node: 1. camera eases toward node, 2. neighboring graph recedes,
3. nonessential labels fade, 4. selected node grows, 5. inspector
materializes spatially near it, 6. detailed timeline becomes available.

Back: camera pulls out and restores constellation context.

This should feel continuous, not page navigation.

## 9. Issue inspector

Shows, progressively: - issue ID/title/status, - current stage, -
agent/profile, - branch/worktree, - elapsed time, - dependencies, -
changed files count, - tests summary, - review findings summary, -
latest activity, - actions: open session/logs/pause/cancel where
permitted.

Avoid dumping raw logs by default.

## 10. Build-ready view

Before start, graph is visible with a compact summary: - issue count, -
wave/frontier summary, - parallelizable vs serialized reasoning, - max
configured concurrency, - preflight status, - primary **Build** action.

User can inspect why any pair was serialized.

## 11. Global run controls

A minimal floating control capsule can show: - running worker count, -
run status, - autonomy mode, - pause, - Stop All.

Stop All remains reachable during any active run.

## 12. Timeline

Every ticket has an event timeline. Human-readable entries derive from
real event journal records. Timeline can expand to commands/test/review
evidence.

## 13. Conflict forecasting UI

When user attempts manual parallel grouping or when analysis detects
conflict: - show affected tickets, - concrete shared surfaces, -
dependency/contract reason, - recommended serialization. Never show fake
precision such as "61% safe" unless the metric is rigorously defined;
prefer evidence labels.

## 14. Drag and drop

Useful drag interactions: - add project folder, - pan spatial canvas, -
optionally propose dependency edge by dragging node connector, -
manually group candidate tickets for analysis.

Manual grouping does not bypass safety. Vela validates before
scheduling.

## 15. Completion

Completion should feel calm and earned: - graph settles, - completed
nodes remain visible, - ambient field becomes quieter, - concise result
appears, - user can inspect execution history, PR/branch, test evidence.

No confetti.

## 16. Window behavior

Desktop-native: - custom title bar only if it remains accessible and
draggable, - correct resize/minimize/maximize, - state persists, -
optional system tray/background behavior, - deep links later.

## 17. Responsiveness

Primary target is laptop/desktop. Support narrow windows gracefully, but
do not compromise the spatial desktop concept to imitate mobile.
