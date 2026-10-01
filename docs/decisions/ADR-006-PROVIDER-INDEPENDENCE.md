# ADR-006: Provider-Independent Core

## Status

Accepted.

Vela's scheduler and domain model do not encode
Gemini/Claude/Codex-specific assumptions. Providers are adapters
exposing capabilities. This permits mixed tooling and prevents external
product changes from rewriting core orchestration.
