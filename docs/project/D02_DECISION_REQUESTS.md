# D02 Decision Requests

Interpretations D02 made where the frozen specification is silent, and the decisions the owners still have to take. DR-D02-5 is a
**blocking** request (for the graph-builder integration, not for the D02 code); the others are non-blocking.

## DR-D02-1: Do analyst (inferred-hazard) edges gate the frontier and take part in cycle detection?

- **Specification facts.** `IMPLEMENTATION_ARCHITECTURE.md` section 10 puts typed edges (explicit, inferred-hazard) in one `StableDiGraph`. `PARALLELIZATION.md` says analysis may only add hazards, never remove a hard blocker, and that when uncertain Vela serializes. FR-007 says inferred hazards never silently rewrite user-authored dependencies. No text says whether a hazard edge delays its target.
- **What D02 does.** The graph reuses F02's shared `DependencyEdge` and `EdgeOrigin` (`Authored` = explicit, `Analyst` = inferred hazard; no second edge type). Edges of both origins gate the frontier and the layers (the conservative, serializing reading).
- **Canonical build path (review B1).** `DependencyGraph::add_edges(&[DependencyEdge])` is the order-independent way to build: it validates every ticket first (atomic), inserts all authored edges, then inserts analyst edges sorted by (blocker, blocked), refusing any that would close a cycle, and returns the refused edges sorted. The result depends only on the *set* of edges. Authored edges always take precedence; of two conflicting analyst edges the smaller (blocker, blocked) pair wins. Anything that must be reproducible from a set of edges, in particular the stored snapshot, is built this way. The single-edge `add_edge` is unchanged and remains available, but it is documented as order-dependent (first analyst edge wins, and an authored edge added later can still close a cycle through an analyst edge); `drop_cyclic_analyst_edges()` removes those analyst edges and never an authored one.
- **What the graph does not remember.** A refused or dropped analyst edge leaves no trace in the graph or the snapshot. The caller must record each returned pair as "serialize, order unspecified" (owner: D03, DR-D02-6 below), otherwise the pair could run concurrently and weaken "when uncertain, serialize".
- **Alternative.** Gate only on authored edges and treat hazards as a separate pairwise serialization in `parallel`. Consequence: hazard pairs would be serialized there instead, and cycles could never involve analyst edges.
- **Decision needed from.** The D03 owner and S06: confirm the reading.

## DR-D02-2: Frontier input and double dispatch

- `frontier(done)` is the **dependency-ready set**: every ticket not in `done` whose blockers are all in `done`. It does not read run or worker state. **It therefore includes tickets that are already running** (and paused, failed or waiting on a human), because they are not done and their blockers are. Dispatching straight from it starts the same ticket twice.
- **What D02 provides.** `frontier_excluding(done, started)` returns the dispatchable set: the dependency-ready set minus every ticket that has a worker in any non-done state. A started ticket's dependents stay blocked until it is in `done`. The scheduler (S09) dispatches from `frontier_excluding`, then applies the parallel-safety (D03), integration-health and capacity filters.
- A `FAILED`, `NEEDS_HUMAN` or `CANCELLED` blocker never enters `done`, so its dependents never become ready.
- **Owner of the call site.** S09 must pass the started set and add a test that a running ticket is never re-dispatched. D02 cannot test S09.

## DR-D02-3: No ts-rs DTO exported; owner of the snapshot contract

- The ticket lists "ts-rs exported DTOs in packages/contracts" and the "graph snapshot shape (stored per run)" as shared contracts, but D02's only write surface is `crates/vela-domain/src/graph/**`. `GraphSnapshot` is serde-only: tickets, edges (F02's `DependencyEdge`, so the origin is `authored`/`analyst`) and layers, canonical and re-validated by `from_snapshot`.
- **Downstream owners.** S06 owns persisting the snapshot (reserved migration block 0060-0069, `graph_snapshots`, `dependency_edges`) and building it with `add_edges`. F08 owns `packages/contracts/src/**`; the ts-rs export of `GraphSnapshot` and `Cycle`/`CycleReport` (for the `GRAPH_INVALID` surface) is added there, or by S06 under the `graph` command group (`packages/contracts/src/commands/graph.ts`), as a documented contract change. The serde field names are the contract; do not rename them without that change.

## DR-D02-4: A self-block is not representable

- F02's `DependencyEdge` rejects a ticket blocking itself at construction and on deserialization, so the graph cannot hold a one-ticket cycle.
- **Downstream owners.** S06 (Markdown/local ingest) and S07 (GitHub ingest) must detect a self-block in the source metadata before building edges, flag it as malformed metadata (`COMPONENT_SPECIFICATIONS.md` C02: "malformed metadata (flagged, sequential)") and report it to the user; they must not drop it silently, and must not turn it into a panic or a generic parse error. Each needs an acceptance test for it. D02 does not edit those ticket files.

## DR-D02-5 (BLOCKING for graph-builder integration): origin semantics for deterministic hard constraints

- **Specification facts.** `PARALLELIZATION.md`: hard blockers (explicit edges, shared migration/schema objects, generated artifacts, contract changes, lockfiles, declared resource keys) "always apply"; the analyst "may only add hazards or evidence, never remove a deterministic hard blocker"; deterministic checks are "recomputed independently". F02's `EdgeOrigin` has only `Authored` ("stated explicitly by the ticket or tracker (a hard blocker)") and `Analyst` ("proposed by the dependency analyst and validated by Vela"). There is no origin for a hard constraint that Vela derives deterministically.
- **The gap.** If S06/D03 put a deterministic edge (for example a lockfile ordering) into the graph as `Analyst`, `add_edge` could refuse it and `drop_cyclic_analyst_edges` could remove it, which would silently drop a mandatory hard blocker.
- **What D02 enforces without touching F02.** Only `Analyst` edges can be refused or dropped; `Authored` edges never can (tests: authored edges survive both paths). So **until this is decided, every mandatory constraint must be mapped to `EdgeOrigin::Authored`**. That is fail-safe for correctness, but it mis-attributes a cycle among deterministic constraints as "authored-only" (a defect in the user's dependencies).
- **Why D02 stops here.** Adding a variant (or any new origin) changes the shared F02 contract (`crates/vela-domain/src/ids/values.rs` and the generated `packages/contracts/src/generated/EdgeOrigin.ts`), which is outside D02's write surface and needs authorization.
- **Alternatives.**
  - A. Add `EdgeOrigin::Deterministic` (F02 contract change plus regenerated TypeScript). D02 then protects it like `Authored`, and `Cycle::is_authored_only()` becomes "no analyst edge". Cost: a contract change and a snapshot-format change; best attribution.
  - B. Keep two origins and map deterministic constraints to `Authored` (current behaviour). Cost: none now; cycle attribution is coarser.
  - C. Keep deterministic constraints out of the graph and evaluate them in D03's pairwise/group verdicts; the graph holds only authored and analyst edges. Cost: the frontier does not see them, so D03/S09 must apply them as a separate filter.
- **Smallest decision required.** Pick A, B or C. Until then S06 and D03 must not feed a deterministic constraint to the graph as `Analyst`.
- **Decision needed from.** The human owner of the shared contracts (F02/D03/S06 owners).

## DR-D02-6: Refused and dropped analyst pairs must be recorded as serialized

- `add_edges` and `drop_cyclic_analyst_edges` return the analyst pairs they refused or removed. The graph keeps no record of them. D03 (parallel-safety) must record each as "Serialized by dependency: order unspecified" with its evidence, and S09 must honour it. Non-blocking for D02; a hard requirement for D03/S09.
