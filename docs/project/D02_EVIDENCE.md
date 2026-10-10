# D02 Evidence (execution log)

Ticket: `docs/issues/graph/tickets/D02.md` (Dependency graph engine: DAG, cycle detection, frontier, topological layers).
Base: `efc6ef3cd0ddc5fb0c05ccf054382e613c4645f8` (`main` equal to `origin/main`; D02's only blocker F02 is done; write surface `crates/vela-domain/src/graph/**` held by no other ticket; no resource keys; no unresolved decision).
Worker: branch `d02/dependency-graph-engine`.

## Delivered (`crates/vela-domain/src/graph/**`, pure, no new dependency, no `lib.rs` or manifest change)

| File | Content |
|---|---|
| `types.rs` | `EdgeKind` (`Explicit`, `InferredHazard`), `DependencyEdge`, `Cycle` (ordered tickets, edges, whole component), `CycleReport`, `GraphError`. |
| `engine.rs` | `DependencyGraph` over `petgraph::StableDiGraph`: add/remove tickets, typed edges (one per ordered pair, explicit wins), `find_cycles`, `validate`, `layers`, `frontier`. |
| `cycle.rs` | Tarjan SCCs, then the shortest cycle through each component's smallest id (deterministic). |
| `snapshot.rs` | `GraphSnapshot` (tickets, edges, layers; canonical) with `snapshot()` and re-validating `from_snapshot()`. |
| `mod.rs` | Module wiring, gated on `full` because `petgraph` is optional (the hook build has no graph code). |

## Acceptance criteria

1. Cycles reported with participating tickets; frontier never contains a ticket with an incomplete blocker: `cycle_detection_agrees_with_reachability_and_reports_real_cycles` (proptest vs. a brute-force reachability oracle; every reported cycle is a closed path over real edges), `layers_and_frontier_respect_every_edge_on_dags` (soundness and completeness of the frontier for arbitrary done sets), plus named cases (diamond, self-block, two cycles, component wider than the shortest cycle).
2. 500-node graphs within a test-recorded time: `five_hundred_node_graph_is_processed_within_the_recorded_time` builds 500 tickets / 981 edges, computes layers, 50 frontier waves, a snapshot and a reload; it prints the elapsed time (about 70 ms in a debug build here) and fails above 5 s.

## Decisions

Three interpretations in `D02_DECISION_REQUESTS.md` (DR-D02-1 to DR-D02-3), all non-blocking.

## Validation (this host: Linux; the Tauri crates need GTK and are excluded locally, CI covers them)

- `cargo fmt --all --check`: pass. `npm run check:structure`: pass.
- `cargo clippy --workspace --exclude vela-desktop --exclude fake-approval-window --all-targets --locked -- -D warnings`: pass.
- `cargo build` (same scope) and `cargo test` (same scope, and `-p vela-domain --features fault-injection`): pass, `vela-domain` 149 tests including 17 graph tests (3 proptest properties).
- `cargo check -p vela-domain --no-default-features --features hook-table`: pass (the graph module compiles to nothing).
- `npm run verify:ts` (after `npm ci --engine-strict=false`; this host has Node 22 where the repo requires Node 24): pass.
