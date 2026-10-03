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


# Reference Hardware Profile

**Profile ID: `REF-HW-1`** (recorded 2026-10-03; resolves human action H03). This section records where the budgets above are measured. It does not change, relax, or recalibrate any budget. The budgets stay canonical and change only in this document, through the performance pass, with a stated reason.

## Policy

-   The user's current development laptop is Vela's canonical reference machine for performance measurement.
-   `REF-HW-1` describes the machine **as configured on 2026-10-03**, with 16 GB of RAM installed as a single module. It is a record of that configuration, not a promise that the machine will stay that way.
-   Every benchmark report names the profile ID **and** includes the per-run environment capture listed below. The capture, not this section, is the historical record of what a result was measured on.
-   A change to the CPU, GPU, installed RAM (capacity, module count, or channel layout), the display panel, or a move to a different machine creates a **new profile** (`REF-HW-2`, and so on) in this document. `REF-HW-1` and every result recorded against it are never edited to match the new hardware. A RAM upgrade is therefore a new profile, not an edit.
-   Driver, Windows update, and WebView2 version changes are **not** new profiles; they are captured per run.
-   Only a **reference run** (conditions below) counts as evidence against the budgets or in the release validation report. Any other run is a **non-reference exploratory run**: it may use different conditions and may inform development, but its report must say plainly that it is non-reference, and it must never be used as release evidence for satisfying a budget.
-   Choosing this machine does not weaken any budget. Full, Balanced, and Efficiency are each measured. Balanced remains the default mode. Measurement continues to account for reduced motion, minimized and hidden behavior, unfocused behavior, the device-pixel-ratio cap, adaptive quality suggestions, and every other budget above. No benchmark numbers are recorded in this section.

## Verified directly on the machine (read-only queries, 2026-10-03)

| Item | Value |
|---|---|
| System | Acer Nitro ANV16S-71 (as reported by Windows); baseboard `RPL BRZ_RTH` |
| CPU | Intel Core 9 270H; 14 cores, 20 logical processors; reported clock 2700 MHz |
| RAM | 1 module of 16 GB, Kingston, DDR5, 5600 configured; slot `Controller0-ChannelA-DIMM0`; the OS sees 15.63 GB. One module means single-channel operation (derived from the module count). |
| Discrete GPU | NVIDIA GeForce RTX 5070 Laptop GPU; 8 GiB (`nvidia-smi` reports 8151 MiB); driver 616.92 (Windows driver 32.0.16.1692, dated 2026-09-04) |
| Integrated GPU | Intel Graphics; driver 32.0.101.7079 (dated 2025-10-28) |
| Display | panel manufacturer code CSW, product `164F` (EDID); 2560 x 1600 at 180 Hz current mode, reported on the Intel adapter; about 15.9 in diagonal from the EDID size (34 x 22 cm) |
| Display scaling | Windows scaling 150%; logical resolution about 1707 x 1067, so a device pixel ratio of 1.5 |
| Windows | Windows 11 Home 25H2, version 10.0.26200.9550 (build 26200, UBR 9550), 64-bit |
| Power plan | active power scheme named "Acer", GUID `a1f14ce5-b330-4a11-827f-ce34593eaa56`; on AC power with the battery at 79% at the time of inspection |
| WebView2 runtime | 154.0.4258.53 at inspection (it updates automatically; captured per run) |
| Storage (context only) | Micron 2500 NVMe SSD, about 954 GB, healthy; about 459 GB free on `C:` at inspection |

Hybrid graphics: the 2560 x 1600 at 180 Hz mode is reported on the Intel adapter, so the display may be driven through the integrated GPU while the RTX renders. Which GPU renders Vela's WebView2 content cannot be determined until the application exists and must be captured per run.

## Supplied by the user, not independently verified

-   The marketing name "Acer Nitro 5". Windows reports the model as `Nitro ANV16S-71`; the family naming is not verified.
-   The role of this laptop as the development machine.
-   The intent that 16 GB is the current reference configuration and that a RAM upgrade may happen later.
-   GPU power limit and any Acer or NitroSense performance mode. `nvidia-smi` reports the power limit as unavailable, and the vendor mode is not queryable without the vendor tool. These are conditions to capture, not profile facts. No Acer or Nitro performance mode is mandated: there is currently no reliable programmatic fact about which modes exist or which should be canonical, so choosing one is a future decision.

## Not relevant to the profile

Storage free space, battery charge, current CPU clock, and GPU performance state vary from moment to moment and are not profile values.

## Conditions for a reference run

A result counts as release evidence only if every condition below held for the whole run and is shown in the run's environment capture:

-   The hardware matches `REF-HW-1` as recorded: CPU, GPUs, RAM capacity and module count, and panel resolution and refresh rate.
-   AC power is connected.
-   A stable Windows power scheme is active, with the exact scheme (name and GUID) recorded and unchanged during the run.
-   No deliberate heavy foreground or background workload competes with Vela (builds, other benchmarks, other agents); the machine is exclusive for the run.
-   Every item of the per-run environment capture below is recorded, in particular the GPU that actually rendered Vela and its WebView2 content, the driver versions, the WebView2 runtime version, the graphics mode, the reduced-motion state, the window state where relevant, the effective device pixel ratio and its cap, and the display resolution and refresh rate.
-   Any Acer or Nitro performance mode that can be reliably determined is recorded. If it cannot be determined, the capture says "not determined". The profile does not require a particular mode.

The report states "reference run" or "non-reference exploratory run". A run that fails any condition is non-reference.

## Per-run environment capture (record with every benchmark result, reference or not)

-   Profile ID (`REF-HW-1`) and the date and time of the run.
-   Vela build identifier (commit SHA) and graphics mode: Full, Balanced, or Efficiency, and whether the mode was set manually or by an adaptive suggestion.
-   Reduced-motion setting, window state (focused, unfocused, minimized, hidden), window size, device pixel ratio, and the device-pixel-ratio cap in effect.
-   Windows version and build, installed RAM (capacity, module count, speed), CPU model, GPU models and driver versions, WebView2 runtime version.
-   Display mode (resolution, refresh rate, dynamic refresh rate setting) and Windows scaling.
-   Power source (AC or battery), battery charge, the exact active Windows power scheme (name and GUID) and Windows power mode, and any Acer or Nitro performance mode that can be reliably determined (otherwise "not determined").
-   Which GPU rendered the WebView2 content (the per-app Windows graphics preference for the application and its WebView2 host).
-   Machine condition: other applications and builds stopped (the machine is exclusive for the run), thermal state if observable, any deviation from the profile, and whether the run is a reference run or a non-reference exploratory run.
