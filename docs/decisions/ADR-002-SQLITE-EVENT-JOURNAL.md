# ADR-002: SQLite + Durable Event Journal

## Status

Proposed / preferred.

## Decision

Persist materialized orchestration state and an append-oriented event
journal in SQLite.

## Why

Autonomous work must survive crashes and be explainable after the fact.
Pure in-memory state is unacceptable; raw log files alone are
insufficient for reconciliation.

## Consequences

Every state transition and external operation needs transaction
boundaries and migration coverage.
