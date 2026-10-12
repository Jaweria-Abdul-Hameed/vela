//! The Vela desktop executable: all composition lives in the library (`vela_desktop::run`).
// Release builds are a GUI application (no console window).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    vela_desktop::run();
}
