//! vela-desktop: module skeleton (F01). Modules are empty and implemented by the tickets that own them;
//! see docs/issues/graph/SHARED_SURFACE_PROTOCOL.md section 1. Never edit this declaration list in a ticket.

pub mod autostart;
pub mod commands;
pub mod events;
pub mod lifecycle;
pub mod notify;
pub mod plugins;
pub mod tray;
pub mod update;
pub mod window;
pub mod wiring;

#[cfg(test)]
mod tests {
    #[test]
    fn bootstrap_placeholder() {}
}
