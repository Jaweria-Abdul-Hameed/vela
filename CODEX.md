# CODEX.md

Codex must treat `AGENTS.md` as the source of truth.

-   Work only in the assigned worktree and ticket.
-   Inspect relevant specs and ADRs before editing.
-   Prefer small, reviewable changes.
-   Run the required focused and repository gates.
-   Preserve the fixed-point commit for review.
-   A ticket gets one comprehensive independent `/code-review`, then
    independent verification of the blocking fixes (ADR-021). Do not
    review or verify your own implementation session's work, and do not
    self-authorize a further review or verification round.
-   Do not infer completion from compilation alone.
-   Do not perform destructive Git operations, force pushes, credential
    changes, or unrelated refactors.
-   Emit the structured completion report defined in `AGENTS.md`.

# Runtime Scope Reminder

Vela v1 is Antigravity-first and provider-extensible. Codex may be used to build/review Vela, but a Codex runtime adapter is not a v1 requirement unless explicitly introduced by an approved issue. Preserve the provider-neutral seam while prioritizing complete Antigravity behavior.
