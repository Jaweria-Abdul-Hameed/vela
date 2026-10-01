# CLAUDE.md

Claude must treat `AGENTS.md` as canonical. This file only supplies
Claude-specific routing.

1.  Read `AGENTS.md` first.
2.  Respect ticket scope and the recorded integration fixed point.
3.  Use repository skills when installed rather than reproducing their
    behavior informally.
4.  For implementation, follow the repository's `/implement`/TDD/review
    protocol when available.
5.  For review, compare against the explicitly recorded fixed point.
    Never guess the base.
6.  Do not treat a long conversation as durable project memory; write
    durable decisions to Markdown/ADR/issues.
7.  Do not change product/UI architecture because an alternative seems
    preferable. Raise a decision request.
8.  Return the structured completion report required by `AGENTS.md`.

# Runtime Scope Reminder

Vela v1 is Antigravity-first and provider-extensible. Claude may be used to build/review Vela, but a Claude runtime adapter is not a v1 requirement unless explicitly introduced by an approved issue. Preserve the provider-neutral seam while prioritizing complete Antigravity behavior.
