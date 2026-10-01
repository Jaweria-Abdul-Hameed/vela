# Issue Authoring Standard

## Ticket philosophy

Tickets are tracer-bullet vertical slices sized for one fresh
implementation context whenever possible.

Bad horizontal decomposition: - create all database models, - create all
APIs, - create all UI.

Better: - project import from folder through persisted recent-project
entry, - one complete worker lifecycle through implementation/review
evidence, - one complete graph node state from scheduler event to UI.

## Template

``` markdown
# <Outcome-oriented title>

## Objective
What capability exists after this ticket?

## Requirements
- FR-...
- ...

## User-visible behavior
...

## Technical scope
...

## Dependencies / blocked by
- #...

## Parallelization notes
Expected write surfaces and known contracts.

## Acceptance criteria
- [ ] observable criterion
- [ ] observable criterion

## Test/evidence requirements
- unit:
- integration:
- UI:
- manual:

## Relevant docs
- `...`

## Non-goals
...
```

## Dependency rules

Declare real blockers, not arbitrary ordering. If ticket B merely
touches a different module and does not require A's output, do not block
it.

## Size

If a ticket requires multiple unrelated mental models, split it. If
splitting would produce unusable horizontal layers, retain the vertical
slice and reduce scope another way.
