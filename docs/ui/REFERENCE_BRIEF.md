# Visual Reference Brief for Designers and UI Agents

Use the user-supplied Google Stitch screenshot as the strongest mood
reference, but do not clone branding, copy, or proprietary composition.

What to preserve conceptually: - vast black field, - tiny ordered dot
lattice, - cursor-responsive field, - luminous gradient landscape
entering from edges/bottom, - violet → electric blue → cyan spectrum, -
crisp soft-white type, - very little visible chrome, - central focus, -
generous negative space, - translucent dark input/control surfaces, -
premium restrained motion.

Translate those qualities into an engineering orchestration universe: -
issue nodes become stars/anchors, - dependencies become fine luminous
curves, - worker activity becomes localized field motion, - merge flow
becomes directional edge energy, - contextual details float in glass, -
the graph is draggable/pannable/zoomable, - camera focus replaces hard
page changes.

The visual test is: if all labels were removed, the application should
still feel calm, spatial, premium, dark, and alive---not like a gaming
overlay or analytics SaaS dashboard.

# Reference Asset Policy

The canonical visual reference is the actual user-supplied Google Stitch screenshot, versioned in this repository at:

**`docs/ui/reference/vela-stitch-reference.png`** (PNG, 2556 x 1483, supplied by the user and verified 2026-10-02; the file is never edited, cropped, recompressed, or regenerated).

Rules for UI implementation and UI fidelity-review agents:

-   **Inspect the image itself.** Open the PNG from the repository path; do not rely on this prose, on earlier conversation, or on memory of any real product, and do not redraw or approximate it.
-   The image is a **visual** reference only. It does not override the written requirements for functionality, architecture, orchestration, state models, accessibility, reduced motion, keyboard navigation, screen-reader support, performance, adaptive graphics quality, safety, approval handling, recovery, Stop All, or any other non-visual behavior. Where the image is silent, the written specifications govern. Where the image provides concrete visual evidence, inspect the image.
-   Do not clone Stitch branding, copy, layout, or proprietary assets; compare the page area only and ignore the browser frame.
-   Reference paths in documents are relative to the repository root, never absolute local paths.
-   Visual-fidelity acceptance (AT-008, `UI_ACCEPTANCE_CHECKLIST.md`, ticket U19) is judged against this file.

## Observed in the asset (recorded 2026-10-02 from the image itself)

The screenshot is the Stitch landing page in a browser window (browser tab strip, address bar, and scrollbar are visible and are not part of the reference).

-   **Canvas:** black dominates, most strongly in the upper center.
-   **Dot lattice:** a regular, sparse grid of tiny dim dots at uniform spacing, visible on the black areas and over the lit areas. A still image cannot show pointer response.
-   **Ambient light:** large luminous masses along the left, right, and bottom edges rising into wave or ridge shapes and leaving the center black. Hues run from deep violet and indigo through electric and periwinkle blue to sky-blue and cyan-leaning tones (strongest at the left), with magenta or pink-violet tints in the left and right masses. The masses are soft overall but carry fine banded or streaked texture and brighter ridge highlights.
-   **Typography:** oversized white sans-serif in a light-to-regular weight with tight leading, centered in two lines, followed by a smaller regular-weight subtitle; a small white wordmark with a pill badge at the top left.
-   **Glass:** one large panel with a generous corner radius, dark and translucent, tinted by the light behind it (a violet glow in its lower left), with a hairline border and a faint brighter rim at the upper left; its controls are fully rounded pills; no stacked glass.
-   **Chrome and composition:** almost none: wordmark top left, one opaque white pill action at the top right, a centered headline, one large input panel, and three small rounded suggestion chips below it. No sidebar, no card grid, no metric tiles, no neon borders, no heavy bloom, no 3D objects.
-   **Not shown:** any dependency graph, nodes, edges, constellation, pan or zoom, camera transitions, inspector, timeline, run controls, states, or accessibility behavior. The written specifications govern all of these.

## Differences between the asset and the written direction (recorded, not resolved here)

1.  The written spectrum names violet, electric indigo or blue, and cool cyan. The image also shows magenta or pink-violet tints, and its cyan is a sky-blue, cyan-leaning tone rather than saturated cyan.
2.  The written direction says "enormous blurred gradient masses". The image's masses are large and soft but have visible structure (ridges, streaks, fine texture).
3.  `DESIGN_SYSTEM.md` asks for display type that is "large, sparse, strong". The image's display type is large and sparse but light to regular in weight, not heavy.
4.  The written direction favors translucent dark controls. The image's single primary action is an opaque white pill.
5.  The pointer-reactive dot response, the local cyan or violet glow, and slow ambient motion are written requirements that the still image cannot confirm or contradict.

None of these is a contradiction that requires changing a frozen specification: `DESIGN_SYSTEM.md` states its color families are "to be tuned visually". Tickets U01-U04 record such differences during their reviews and U19 classifies them; any change to the written specification is a decision for the user.

## Canonical reference asset integrity

`docs/ui/reference/vela-stitch-reference.png` is the canonical baseline and must stay byte-identical.
SHA-256: `ddfc4309bd9839db194d85e349c134c4be3610d63fba5265788ba0f721027295`
