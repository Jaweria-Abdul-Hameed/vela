# Non-Goals

Vela is not: - a replacement code editor, - a general-purpose RPA
auto-clicker, - a mechanism for bypassing service quotas, - a
password/account automation tool, - a cloud build farm in v1, - a
generic project-management suite, - a replacement for Git/GitHub, - an
excuse to run every issue concurrently, - an opaque "AI swarm" whose
actions cannot be reconstructed.

# V1 Runtime Non-Goals

Unless explicitly added through an approved scope change, Vela v1 does **not** aim to:
- provide equal runtime support for every coding agent/provider;
- ship Claude and Codex runtime adapters merely because those tools can build Vela;
- delay Antigravity reliability in order to achieve provider parity;
- abstract away Antigravity-specific capabilities that are necessary for a high-quality Antigravity experience.

The core should remain extensible without prematurely implementing future providers.
