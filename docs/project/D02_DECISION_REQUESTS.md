# D02 Decision Requests

Interpretations D02 made where the frozen specification is silent. None blocks D02; each is non-blocking and recorded for the owners.

## DR-D02-1: Do analyst (inferred-hazard) edges gate the frontier and take part in cycle detection?

- **Specification facts.** `IMPLEMENTATION_ARCHITECTURE.md` section 10 puts typed edges (explicit, inferred-hazard) in one `StableDiGraph`. `PARALLELIZATION.md` says analysis may only add hazards, never remove a hard blocker, and that when uncertain Vela serializes. FR-007 says inferred hazards never silently rewrite user-authored dependencies. No text says whether a hazard edge delays its target.
- **What D02 did.** The graph reuses F02's shared `DependencyEdge` and `EdgeOrigin` (`Authored` = explicit, `Analyst` = inferred hazard; no second edge type). Edges of both origins gate the frontier and the layers (the conservative, serializing reading). An analyst edge that would close a cycle is refused at insertion (`GraphError::HazardClosesCycle`), so analysis cannot invalidate a valid graph (FR-007). An authored edge added later can still close a cycle through an analyst edge; `drop_cyclic_analyst_edges()` removes those analyst edges (never an authored one) and returns them, so the builder treats each pair as sequential-but-unordered. After it, every remaining cycle is authored-only (`Cycle::is_authored_only()`): a defect in the user's dependencies (`GRAPH_INVALID`).
- **Alternative.** Gate only on authored edges and treat hazards as a separate pairwise serialization in `parallel`. Consequence: hazard pairs would be serialized there instead, and cycles could never involve analyst edges.
- **Decision needed from.** The `parallel` owner and S06/S07: confirm the reading, and call `drop_cyclic_analyst_edges()` before treating a graph as invalid.

## DR-D02-2: Frontier input is a plain set of done tickets

- The frontier takes the set of tickets that are `DONE` and returns every other ticket whose blockers are all in it. It does not read run or worker state, so a `FAILED`/`NEEDS_HUMAN`/`CANCELLED` blocker simply never enters the set and its dependents stay off the frontier. Integration health and the parallel-safety verdict are applied by the scheduler (S09) and `parallel`, not here. Consequence: the frontier is "dependency-ready", and S09 must intersect it with the safe group.

## DR-D02-3: No ts-rs DTO exported

- Section 9 of the ticket lists "ts-rs exported DTOs in packages/contracts" and the "graph snapshot shape (stored per run)" as shared contracts, but the only write surface is `crates/vela-domain/src/graph/**`. `GraphSnapshot` is therefore serde-only (canonical, re-validated by `from_snapshot`). Exporting it to `packages/contracts` belongs to the ticket that adds the `graph` command group or the snapshot IPC contract (F08/S06), as a documented contract change.

## DR-D02-4: A self-block is not representable

- F02's `DependencyEdge` rejects a ticket blocking itself at construction and on deserialization, so the graph cannot hold a one-ticket cycle. The tracker/Markdown ingest (S06/S07) must report a self-block as malformed metadata before it reaches the graph; D02 does not model it.
