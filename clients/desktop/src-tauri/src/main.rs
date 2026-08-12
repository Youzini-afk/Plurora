// Plurora Desktop entry point.
// The library owns a managed, loopback-only Host sidecar and navigates the
// shared Web shell to it after readiness, preserving one origin for every
// platform protocol and Work surface.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    plurora_desktop_lib::run();
}
