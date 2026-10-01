# Design System

## Color philosophy

Near-black dominates. Use tokens rather than hard-coded scattered
colors.

Suggested semantic families, to be tuned visually: - canvas: near-black
neutral, - surface: translucent black, - text-primary: soft white, -
text-secondary: cool gray, - violet-light, - blue-light, - cyan-light, -
warning-light, - danger-light, - success-light.

Do not make status colors fluorescent by default.

## Typography

Use a clean modern grotesk/sans family with excellent Windows rendering.
Typography should resemble the confidence and simplicity of modern
Google product marketing without copying proprietary assets.

Hierarchy: - display: large, sparse, strong, - title, - body, -
metadata, - mono only for SHAs/commands/paths/logs.

## Spacing

Prefer generous whitespace. Dense information belongs in expandable
detail surfaces, not the base canvas.

## Radius

Floating surfaces: generous rounded corners. Tiny pills: fully rounded.
Graph nodes: geometry may vary by semantic state but remains restrained.

## Borders

1px low-opacity boundaries only where necessary. Light/bloom should not
substitute for focus indication.

## Icons

Simple geometric/rounded icon set. Avoid mixed icon families.

## Depth

Depth comes from: 1. parallax, 2. blur, 3. scale, 4. occlusion, 5.
subtle lighting, not heavy drop shadows.

## Component inventory

-   ProjectDropZone
-   RecentProjectAnchor
-   SpatialCanvas
-   DotField
-   AmbientGradientField
-   DependencyNode
-   DependencyEdge
-   IntegrationNode
-   WorkerAura
-   FloatingRunBar
-   IssueInspector
-   Timeline
-   TestEvidencePanel
-   ReviewFindingsPanel
-   InterventionSheet
-   PreflightPanel
-   CommandPalette
-   SettingsSheet
-   Toast/Notification
-   StopAllControl
-   ReducedMotionFallback
