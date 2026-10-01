# Configuration

Settings layers: 1. application defaults, 2. user global settings, 3.
project settings, 4. run snapshot.

A running build uses a snapshot so changing settings mid-run does not
silently rewrite policy unless the setting is explicitly live.

Categories: - appearance, - motion/graphics, - orchestration
concurrency, - review policy, - test commands, - Git behavior, -
tracker, - agent adapter, - autonomy/security, - notifications, -
diagnostics.

Recommended defaults: - graphics: Balanced, - review: Engineering, -
parallelism: conservative automatic, - destructive operations: Ask, -
Stop All confirmation: minimal but clear, - background execution:
enabled only after explicit onboarding choice.

# Default Runtime Configuration

The default production runtime/provider for v1 is Antigravity. Configuration schemas may be extensible, but documentation and defaults must not present unsupported providers as selectable working alternatives.

Future provider values should remain hidden/unsupported until their adapter passes the required acceptance contract.
