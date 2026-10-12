# ADR-024: Product Review Stage Off by Default (Worker Loop Skips Review)

## Status

ACCEPTED (2026-10-12, by the user's instruction: review loops take too long on Antigravity). Overrides, for the Vela
product at runtime, ADR-015 (review ownership), ADR-021, ADR-022, FR-013, FR-014, AT-004 and the review sections of
`ORCHESTRATION_ENGINE.md` section 4 and `REVIEW_PROTOCOL.md`. It does not change checkpointing, base-SHA recording,
focused validation, integration validation, the merge lane, security gates, or the policy engine.

## Decision

1. Add a run setting `review_mode` with values `off` and `single`. **Default `off`.**
2. With `off`, a worker goes implement, focused validation, checkpoint commit, then to the merge lane. There is no
   reviewer session, no findings, no fix round, no verification, no `REVIEW_STALLED`, and the final-review step before
   push is skipped. Integration validation (tests/build after merge) is unchanged and remains the quality gate.
3. `single` is the ADR-022 behavior (one independent review, targeted verification, human escalation). It stays
   available but is implemented last and is not on the dogfood path.
4. Ticket handling under this ADR:
   - D06 (review domain): implement only the types needed to carry `review_mode`; finding identity and exit policy are
     implemented only if cheap, otherwise deferred.
   - S11: implement checkpoint and the `off` path only; the review branch is a stub that returns NEEDS_HUMAN
     "review_mode=single not implemented".
   - S13, S14, A10, A11: skip every review-related step and acceptance criterion; keep the rest (conflict escalation,
     finalization, push, promotion; dependency analyst is unchanged).
   - AT-004 and any acceptance criterion about review findings, review iterations or verification are deferred and are
     not release blockers for the dogfood cut.
5. Other tickets that mention review only in passing follow the same rule: if a criterion needs a review result, skip
   it and record it as deferred in the progress file.

## Consequences

- The worker loop is much shorter and uses far less Antigravity quota per ticket.
- Defects are caught only by focused validation, integration validation, and the user. Raising `review_mode` to
  `single` later is possible without redesign because the seam is kept.
