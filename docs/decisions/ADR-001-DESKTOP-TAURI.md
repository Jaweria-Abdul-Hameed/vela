# ADR-001: Tauri Desktop Shell

## Status

Proposed / preferred.

## Decision

Use Tauri with React/TypeScript for the Windows-first desktop
application.

## Rationale

The product needs a polished web-capable rendering surface plus native
filesystem/process integration while remaining lighter than a
conventional Electron distribution. Tauri also allows a Rust core to own
orchestration independently of React.

## Consequences

-   Rust expertise required for core/native boundary.
-   WebView behavior must be tested on supported Windows versions.
-   GPU/WebGL compatibility requires fallback modes.
-   UI must not assume browser-only APIs without Tauri consideration.
