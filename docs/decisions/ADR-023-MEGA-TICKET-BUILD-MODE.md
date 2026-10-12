# ADR-023: Mega-Ticket Build Mode (Build Process Only; No Per-Ticket Independent Review)

## Status

ACCEPTED (2026-10-12, by the user's instruction: no review time at all). If accepted it supersedes ADR-021 and ADR-022 **for the process
that builds Vela only** (AGENTS.md "Implementation loop", CLAUDE.md item 5). It does not change what Vela, the
product, does at runtime: FR-013, FR-014, AT-004, tickets D06, S11, S13, S14 and A10 still implement the product's
review loop as specified.

## Context

Vela is needed for the user's own projects as soon as possible. With 130 tickets left and independent review plus
verification per ticket, each ticket takes hours and the project would take far too long. The 130 tickets are regrouped
into 32 mega-tickets plus a human track (`docs/issues/graph/MEGA_TICKETS.md`).

## Decision

1. **Unit of work:** one mega-ticket per session. Its member tickets remain the specification and the internal
   checklist; the ticket files and their acceptance criteria are unchanged.
2. **No review of any kind**: no independent `/code-review`, no verification round, no self-audit pass.
3. **Tests that belong to a member ticket's acceptance criteria are still written** (TDD where the ticket calls for
   it). Only the standalone test/verification tickets (X04, R03, Z03, U18, A15, PF1, Z04, Z07) are deferred, to M28
   to M31.
4. **Gates:** a mega-ticket is complete only when its required CI gates are green on its branch. Local Rust gates are
   not available on this host (Smart App Control), so CI is the check.
5. **Checkpoints:** one commit per member ticket, so a failure loses at most one member.
6. **Unchanged:** security gates, the fixed-point and base-SHA recording, git isolation (own branch and worktree per
   concurrent mega-ticket, no force-push), `NEEDS_HUMAN` on contradictions, the shared-surface protocol, and the
   rule against changing product or UI architecture without a decision request.
7. **Human review remains available:** the user may run `/code-review` on any mega-ticket's branch at will; it is not
   required and a finding does not block the merge unless the user says so.

## Consequences

- Much faster; defects that review would have caught surface later through use. Mitigation: the deferred hardening
  groups M28 to M31 and the user's own dogfooding.
- AGENTS.md and CLAUDE.md were updated to match.

## Alternatives considered

- Keep one review per ticket: rejected, too slow.
- Review once per mega-ticket instead: reasonable middle ground (one `/code-review` per mega-ticket, about 20 to 30
  minutes each); not chosen by default but allowed under decision 7.
