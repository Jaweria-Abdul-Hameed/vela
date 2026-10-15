# UI fidelity review against the Stitch reference (U19)

**Reference:** `docs/ui/reference/vela-stitch-reference.png` (inspected directly as an image, not from prose). **Compared:** the page area only (below the browser chrome); the Stitch name, the "Beta" badge, the hero copy and the prompt-box layout are ignored, because Vela must not clone them.
**Subject:** the running renderer in headless Chromium with software GL, 1280 x 800, scenes from `apps/desktop/src/visual/fixtures.ts` (the same images are the visual baselines in `apps/desktop/e2e/visual.e2e.ts-snapshots`).
**Reviewer:** the implementing agent. **This review is not independent**, which U19 asks for; it is recorded as a self-review done because a person was not available during the build (ADR-023: no review loops while building). An independent pass by a person remains open and is the correct place to catch what a self-review misses.

## Regions compared and findings

| # | Region (reference) | Reference shows | Vela before | Class | Result |
|---|---|---|---|---|---|
| 1 | Page background, whole area | near-black canvas that stays dominant | near-black | none | matches |
| 2 | Lower-left and lower-right edges | large soft violet, blue and cyan masses rising from the edges, volumetric | **no light at all**: the shell painted an opaque background over the canvases, and the field was far too dim | BLOCKING FIDELITY | fixed: the shell is transparent over the canvases; three masses (violet left, blue right, cyan low centre) at an intensity that leaves the centre black. Compared again: comparable character and placement |
| 3 | Whole area | an ordered, subtle dot lattice | **not visible** (hidden by the same opaque shell) | BLOCKING FIDELITY | fixed by the same change; the lattice now shows faintly, ordered, not noisy |
| 4 | Controls (the only buttons in the reference are pills and a glass field) | quiet pill buttons, soft glass input | the browser's default grey rectangles | IMPORTANT | fixed: pill buttons with a hairline border, glass-toned fields, visible pressed state |
| 5 | Content column | a centred composition with generous space | content flush to the left edge across the full width | IMPORTANT | fixed: a centred column (max 72 rem) |
| 6 | Typography weight and hierarchy | light display type, strong hierarchy | headings are bold 1.5 rem system type | POLISH | left: Vela is a tool, not a landing page; hierarchy is correct, display type is not needed on working screens |
| 7 | Glass panel (prompt box) | one contextual glass surface | glass is used only on the inspector, trust card and intervention sheet | none | matches the rule "glass is contextual, not everywhere" |
| 8 | Scroll bar and browser chrome | present | not applicable | SUBJECTIVE | ignored |

## Where the image is silent
Graph, node states, interaction, camera, accessibility, reduced motion, performance, approval, recovery and Stop All are governed by the written specifications (`UI_UX_SPEC.md`, `MOTION_AND_3D.md`, `ACCESSIBILITY.md`, `PERFORMANCE_BUDGET.md`); the image does not override them. Those were checked by their own tests (U05 to U17), not by this comparison.

## While doing the review (found by the new visual suite)
- Clicking a ticket label in the graph did nothing in a real browser: the container captured the pointer on press, so the label's click never fired. Unit tests missed it because they dispatch a synthetic click. Fixed (a press on a ticket button no longer starts a pan), and the worker-detail scene now covers it with a real click.

## Status
Blocking findings 2 and 3: fixed and compared again against the reference. Important findings 4 and 5: fixed. Not re-reviewed by anyone else.
