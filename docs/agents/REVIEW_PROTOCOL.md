# Review Protocol

## Objective

Determine independently: 1. Does the change satisfy the originating
spec/ticket? 2. Does it satisfy repository engineering standards? 3. Are
there correctness/security/integration risks that required gates reveal?

## Fixed point

Never guess. Each worker stores the base integration SHA. Review uses an
explicit resolvable fixed point and non-empty diff.

## Finding schema

Each finding: - ID, - axis/category, - severity, - evidence, - affected
location, - requirement violated, - actionable fix, - blocking
boolean, - disposition.

## Default Engineering policy

Blocks: - security vulnerabilities introduced by diff, - incorrect
behavior, - missing acceptance criteria, - broken
tests/build/typecheck, - architecture boundary violation with real
impact, - data loss/race/error-path bugs, - medium-or-higher
spec/correctness findings.

Normally nonblocking: - subjective naming, - optional abstraction, -
speculative future-proofing, - style already accepted by
formatter/linter, - micro-optimizations without measured need.

## Loop

`review → classify → fix blocking/actionable → focused tests → checkpoint → review again`.

Do not blindly fix contradictory review suggestions. If repeated reviews
oscillate, mark `REVIEW_STALLED`.

## Exit

`PASS` only when blocking findings are zero and required validation
gates pass.
