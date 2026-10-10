# D02 Decision Requests

Interpretations D02 made where the frozen specification is silent. None blocks D02; each is non-blocking and recorded for the owners.

## DR-D02-1: Do inferred-hazard edges gate the frontier and take part in cycle detection?

- **Specification facts.** `IMPLEMENTATION_ARCHITECTURE.md` section 10 puts "typed `DependencyEdge`s (explicit, inferred-hazard)" in one `StableDiGraph`. `PARALLELIZATION.md` says analysis may only add hazards, never remove a hard blocker, and that when uncertain Vela serializes. FR-007 says inferred hazards never silently rewrite user-authored dependencies. No text says whether a hazard edge delays its target.
- **What D02 did.** Edges of both kinds gate the frontier and the layers (the conservative, serializing reading). Cycles are detected over all edges, but each reported `Cycle` carries its edges, and `Cycle::is_explicit_only()` lets the caller tell a defect in the user's dependencies from a clash caused by a hazard. Adding a pair twice keeps the explicit kind, so a hazard never weakens a user edge.
- **Alternative.** Gate only on explicit edges and treat hazards as a separate pairwise serialization (the `parallel` module, a later ticket). Consequence: hazard pairs would need to be serialized there instead, and cycles could never involve hazards.
- **Decision needed from.** The owner of `parallel` (and S06/S07): confirm the reading, or have the graph builder drop a hazard edge whose `is_explicit_only()` is false before treating the graph as invalid.

## DR-D02-2: Frontier input is a plain set of done tickets

- The frontier takes the set of tickets that are `DONE` and returns every other ticket whose blockers are all in it. It does not read run or worker state, so a `FAILED`/`NEEDS_HUMAN`/`CANCELLED` blocker simply never enters the set and its dependents stay off the frontier. Integration health and the parallel-safety verdict are applied by the scheduler (S09) and `parallel`, not here. Consequence: the frontier is "dependency-ready", and S09 must intersect it with the safe group.

## DR-D02-3: No ts-rs DTO exported

- Section 9 of the ticket lists "ts-rs exported DTOs in packages/contracts" and the "graph snapshot shape (stored per run)" as shared contracts, but the only write surface is `crates/vela-domain/src/graph/**`. `GraphSnapshot` is therefore serde-only (canonical, re-validated by `from_snapshot`). Exporting it to `packages/contracts` belongs to the ticket that adds the `graph` command group or the snapshot IPC contract (F08/S06), as a documented contract change.
