# Interaction Specification

## Pointer

-   hover node: reveal minimal metadata and strengthen connected edges;
-   click: focus node;
-   drag empty canvas: pan;
-   wheel/trackpad: zoom;
-   drag project folder on home: import;
-   drag dependency handle (advanced mode): propose edge, never silently
    mutate without confirmation.

## Keyboard

-   `/` or platform-appropriate shortcut: command palette;
-   Escape: back out of focus/modal;
-   arrows/tab: navigate accessible node representation;
-   Enter: focus/activate;
-   shortcut for Stop All must avoid accidental activation and be
    configurable.

## Context menus

Use sparingly for advanced actions: open worktree, copy branch/SHA,
inspect logs, pause/cancel worker.

## Progressive disclosure

Raw terminal output is never the default visual. User drills: node →
inspector → evidence → raw log.

## Feedback

Every click that initiates a stateful operation immediately shows
accepted/pending state. Never leave the user wondering whether an action
registered.
