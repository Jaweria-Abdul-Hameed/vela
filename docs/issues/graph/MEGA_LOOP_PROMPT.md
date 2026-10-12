# Autonomous loop prompt for mega-tickets

Use as the prompt of each loop iteration (for example `/loop` with this file, or a fresh session per iteration).

---

You are building Vela. Read `AGENTS.md`, `docs/decisions/ADR-023-MEGA-TICKET-BUILD-MODE.md` (build mode overrides the
review sections of AGENTS.md), and `docs/issues/graph/MEGA_TICKETS.md`.

1. Run `git fetch` and confirm `main` is the integration tip. Read `docs/issues/graph/MEGA_PROGRESS.md` (create it if
   missing; one line per mega-ticket: id, status, merge SHA, notes).
2. Pick the next mega-ticket: the first in the "dogfood path" order whose "Starts after" groups are all MERGED in the
   progress file and whose human-track prerequisites (M00 results listed in the doc) are recorded. M00 is never yours.
   If none is eligible, write the reason to the progress file and stop.
3. Create a branch and worktree `mega/<id>` from the current `main` tip; record the base SHA in the progress file.
4. For each member ticket in the listed order: read its file in `docs/issues/graph/tickets/`, implement it with its
   tests (TDD where the ticket says), run the focused tests and typecheck/lint, commit once as `<KEY>: <title>`.
   Do not implement anything outside the member tickets. Respect the write surfaces and
   `SHARED_SURFACE_PROTOCOL.md`.
5. Do not review or self-audit. Move on.
6. Push the branch, wait for CI. Fix CI failures (at most 3 attempts), then if still red set the mega-ticket to
   `NEEDS_HUMAN` with the exact failure and stop.
7. If any ticket is contradictory, unsafe or needs a product-visible decision: `NEEDS_HUMAN` per AGENTS.md, stop.
8. When CI is green: merge with `git merge --no-ff` into `main`, update the progress file and
   `docs/project/CURRENT_STATE.md` briefly, push. Never force-push.
9. Report: mega-ticket, member tickets done, merge SHA, CI result, deferred items, and anything needing the human.
   Then end the iteration.
