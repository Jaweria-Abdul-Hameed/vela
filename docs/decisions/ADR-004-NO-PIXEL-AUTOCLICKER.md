# ADR-004: Prefer Supported Automation Over Pixel Clicking

## Status

Accepted.

## Decision

Vela will not use coordinate-based clicking as its primary
approval/execution mechanism.

## Rationale

UI automation is fragile under layout, DPI, focus, modal, and version
changes. Antigravity exposes native command-execution settings and CLI
permission mechanisms. Vela should configure/detect supported autonomy
and use explicit adapters.

## Exception

Future UI automation may be considered for a capability with no
supported interface only if it is isolated, observable, opt-in, and
fails closed.
