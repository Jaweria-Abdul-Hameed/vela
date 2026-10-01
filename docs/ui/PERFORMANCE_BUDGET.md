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

# Measurable Budgets (initial; calibrated by the performance pass)

The qualitative goals above are given measurable pass/fail criteria. Values are **initial budgets**;
the performance pass (Prompt 18) calibrates them against recorded **reference hardware**, and any
change is made in this document. Release items that depend on them cannot pass until the reference
hardware and measured results are recorded in the release validation report.

| Measure | Initial budget |
|---|---|
| Cold launch to interactive window | 3 s or less on reference hardware |
| Active canvas frame rate (100 nodes) | median 60 fps, 5th percentile at least 50 fps |
| Active canvas frame rate (500 nodes) | 5th percentile at least 30 fps |
| Idle (no animation in flight) | zero animation-frame callbacks; UI-process CPU at most 1% averaged over 60 s |
| Minimized or hidden | zero animation-frame callbacks and zero GPU frame submissions after 2 s; UI-process CPU at most 1% averaged over 60 s; orchestration events still journaled |
| Unfocused (visible) | frame rate reduced to at most 15 fps after 5 s |
| Event burst | 1,000 events/s batched with no main-thread task longer than 100 ms |
| Interaction latency | pointer response and focus transition within 100 ms |
| Memory | no monotonic growth of JS heap or GPU memory across 100 mount/unmount cycles of the canvas |

Reference hardware (CPU, GPU, RAM, display, Windows build) is defined and recorded before the
performance pass; Full, Balanced, and Efficiency modes are each measured.
