# D02 Evidence (execution log)

Ticket: `docs/issues/graph/tickets/D02.md` (Dependency graph engine: DAG, cycle detection, frontier, topological layers).
Base: `efc6ef3cd0ddc5fb0c05ccf054382e613c4645f8` (`main` equal to `origin/main`; D02's only blocker F02 is done; write surface `crates/vela-domain/src/graph/**` held by no other ticket; no resource keys; no unresolved decision).
Worker: branch `d02/dependency-graph-engine`.

## Delivered (`crates/vela-domain/src/graph/**`, pure, no new dependency, no `lib.rs` or manifest change)

| File | Content |
|---|---|
| `types.rs` | `Cycle` (ordered tickets, edges, whole component), `CycleReport`, `GraphError`. Edges are F02's shared `DependencyEdge`/`EdgeOrigin` (`Authored` = explicit, `Analyst` = inferred hazard). |
| `engine.rs` | `DependencyGraph` over `petgraph::StableDiGraph`: add/remove tickets, typed edges (one per ordered pair, authored wins; an analyst edge that would close a cycle is refused), `drop_cyclic_analyst_edges`, `find_cycles`, `validate`, `layers`, `frontier`. |
| `cycle.rs` | Tarjan SCCs, then the shortest cycle through each component's smallest id (deterministic). |
| `snapshot.rs` | `GraphSnapshot` (tickets, edges, layers; canonical) with `snapshot()` and re-validating `from_snapshot()`. |
| `mod.rs` | Module wiring, gated on `full` because `petgraph` is optional (the hook build has no graph code). |

## Acceptance criteria

1. Cycles reported with participating tickets; frontier never contains a ticket with an incomplete blocker: `cycle_detection_agrees_with_reachability_and_reports_real_cycles` (proptest vs. a brute-force reachability oracle; every reported cycle is a closed path over real edges), `layers_and_frontier_respect_every_edge_on_dags` (soundness and completeness of the frontier for arbitrary done sets), plus named cases (diamond, two cycles, component wider than the shortest cycle, refused and dropped analyst edges).
2. 500-node graphs within a test-recorded time: `five_hundred_node_graph_is_processed_within_the_recorded_time` builds 500 tickets / 980 edges, computes layers, 50 frontier waves, a snapshot and a reload; it prints the elapsed time (tens of milliseconds in a debug build here; the exact time is printed by the test) and fails above 5 s.

## Decisions

Four interpretations in `D02_DECISION_REQUESTS.md` (DR-D02-1 to DR-D02-4), all non-blocking.

## Validation (this host: Linux; the Tauri crates need GTK and are excluded locally, CI covers them)

- `cargo fmt --all --check`: pass. `npm run check:structure`: pass.
- `cargo clippy --workspace --exclude vela-desktop --exclude fake-approval-window --all-targets --locked -- -D warnings`: pass.
- `cargo build` (same scope) and `cargo test` (same scope, and `-p vela-domain --features fault-injection`): pass, `vela-domain` the `vela-domain` unit tests (see the CI run for the count), including the graph tests (4 proptest properties).
- `cargo check -p vela-domain --no-default-features --features hook-table`: pass (the graph module compiles to nothing).
- `npm run verify:ts` (after `npm ci --engine-strict=false`; this host has Node 22 where the repo requires Node 24): pass.

## Review (advisory `/code-review` run inside the implementing session; the authoritative fresh-session review is owned by Vela, `REVIEW_PROTOCOL.md`)

- Iteration 1 (head `c674999`, fixed point `efc6ef3`): 9 findings, no severities supplied. Fixed: a hazard that closes a cycle or targets itself could invalidate a run (now refused at insertion, DR-D02-1); linear membership scan in cycle search (now a set); the cycle property test now checks one cycle per cyclic component and that `component` is exactly that component. Dispositioned non-blocking: `frontier` fails for the whole graph when any cycle exists (a cycle is `GRAPH_INVALID` for the run, `COMPONENT_SPECIFICATIONS.md` C02); `frontier` re-validates on each call (O(V+E), measured 70 ms for 50 waves of 500 tickets, no need shown); one reported cycle per component (documented, the `component` field names every participant); opaque `SnapshotMismatch` (safe, rejects); the 5 s bound is generous and records the time.
- Iteration 2 (head `6f3586e`): 8 findings. Fixed: the second `DependencyEdge` type (the graph now reuses F02's shared `DependencyEdge`/`EdgeOrigin`, so the stored snapshot and cycle edges use the contract type and self-edges are unrepresentable, DR-D02-4); hazard protection depended on insertion order (new `drop_cyclic_analyst_edges`, DR-D02-1); `from_snapshot` now reloads edges unchecked so a cyclic snapshot is always `Cycle`. Dispositioned non-blocking: `frontier` whole-graph failure and per-call validation (as in iteration 1); per-insertion DFS allocation (no measured need); one cycle per component (now documented on `is_authored_only`, with `drop_cyclic_analyst_edges` as the remedy); wall-clock bound; an empty `CycleReport` from `layers` is unreachable (Kahn leftovers imply a cycle that Tarjan reports).
- Iteration 3 (head `fe7730d`, the last permitted): 9 findings, no severities supplied. Fixed: `drop_cyclic_analyst_edges` could leave an analyst edge on a second cycle of the same component (now one deterministic pass that removes every analyst edge whose blocked ticket still reaches its blocker; new tests, including order independence); the evidence listed three decision requests instead of four. Dispositioned non-blocking (no code change): `frontier` fails for the whole graph on any cycle and re-validates per call (a cycle is `GRAPH_INVALID` for the run by specification; O(V+E)); which of two conflicting analyst edges is refused at insertion depends on insertion order (the authored side never loses, and `drop_cyclic_analyst_edges` is order-independent for edges added unchecked or later); one cycle reported per component (the `component` field and the drop pass cover it); wall-clock bound; per-pass cost of the drop; a style nit on `Entry`. Closest to the line, for the owners to overrule: the whole-graph `frontier` failure (S09 should treat `GRAPH_INVALID` as run-level, as `ORCHESTRATION_ENGINE.md` section 1.1 does) and the insertion-order dependence of the refusal.
