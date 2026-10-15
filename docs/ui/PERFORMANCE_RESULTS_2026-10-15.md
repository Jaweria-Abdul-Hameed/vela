# Performance results 2026-10-15 (NON-REFERENCE exploratory run)

**This is a non-reference exploratory run.** It is not release evidence and satisfies no budget: the machine was not exclusive (builds and other work were running), rendering used software GL in headless Chromium rather than the GPU and WebView2, and the reference-hardware conditions in `PERFORMANCE_BUDGET.md` were not all verified. Budgets are unchanged.

## Environment capture

- **profile**: REF-HW-1 (claimed; hardware match not verified by this script)
- **when**: 2026-10-15T03:53:50.843Z
- **commit**: b20e710a6a7259d9a9434437042787cdaf1cbd65
- **node**: v24.19.0
- **os**: win32 10.0.26300
- **cpu**: Intel(R) Core(TM) 9 270H (20 logical)
- **ramGiB**: 15.63
- **powerScheme**: Power Scheme GUID: a1f14ce5-b330-4a11-827f-ce34593eaa56  (Acer)
- **battery**: status=2 charge=79%
- **gpus**: NVIDIA GeForce RTX 5070 Laptop GPU driver 32.0.16.1742; Intel(R) Graphics driver 32.0.101.7079
- **display**: x@Hz; 2560x1600@180Hz
- **webview2**: 155.0.4283.45
- **acPower**: AC (or no battery)
- **acerMode**: not determined
- **exclusive**: NOT guaranteed: this run is not isolated from other work on the machine

## Graph code (Node, pure functions, per frame)

| Nodes | Zoom | median ms | p95 ms | max ms |
|---|---|---|---|---|
| 100 | 0.25 | 0.015 | 0.034 | 0.322 |
| 100 | 0.5 | 0.013 | 0.033 | 0.281 |
| 100 | 1 | 0.007 | 0.020 | 0.166 |
| 500 | 0.25 | 0.047 | 0.117 | 0.352 |
| 500 | 0.5 | 0.035 | 0.089 | 0.259 |
| 500 | 1 | 0.014 | 0.033 | 0.185 |

Event burst: 1020 events folded over 60 frames into 60 flushes; longest frame 0.080 ms (budget: no task over 100 ms).

## Renderer (headless Chromium, software GL, DPR 1.5)

| Mode | Reduced motion | Cold launch ms | rAF/s idle (budget 0) | Long tasks | Longest task ms | JS heap MiB |
|---|---|---|---|---|---|---|
| efficiency | false | 276 | 0.0 | 0 | 0 | 28.0 |
| efficiency | true | 241 | 0.0 | 0 | 0 | 28.0 |
| balanced | false | 230 | 0.0 | 0 | 0 | 29.8 |
| balanced | true | 270 | 0.0 | 0 | 0 | 28.0 |
| full | false | 238 | 0.0 | 0 | 0 | 28.0 |
| full | true | 233 | 0.0 | 0 | 0 | 28.0 |

## Not measured by this harness

- Frame rate of the 100- and 500-node WebGL graph on the real GPU through WebView2 (needs the built desktop app and a reference run).
- Minimized and hidden behaviour, unfocused throttling, GPU frame submissions, UI-process CPU over 60 s, memory across 100 canvas mount/unmount cycles on the GPU.
- AT-009 (orchestration events still journaled while minimized): the journal is written by the core, independent of any window; confirming it with the real minimized window needs the desktop app.

