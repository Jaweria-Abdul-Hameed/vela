# UI Performance Budget

Targets are product goals and must be measured on representative Windows
hardware.

-   Cold launch should feel fast; avoid blocking startup on repository
    analysis.
-   UI becomes interactive before expensive project indexing finishes.
-   60fps target for active canvas on capable hardware; degrade
    gracefully.
-   No continuous high GPU usage when nothing is moving.
-   Unfocused/minimized canvas should throttle aggressively or suspend.
-   Large logs are virtualized.
-   500-node graph remains navigable.
-   Event bursts are batched.
-   shader/texture memory is bounded and disposed.
-   expensive blur/bloom passes are minimal.

Provide three graphics modes: 1. **Full** --- all supported ambient
effects. 2. **Balanced** --- default; reduced postprocessing. 3.
**Efficiency** --- minimal canvas effects and aggressive idle
suspension.

Automatic adaptation may suggest a mode but must not hide the user
setting.
