//! No UI, WinRT, Shell or media imports in the LPAC loader's dependency graph.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
fn main() {
    #[cfg(target_os = "windows")]
    oxyde_lib::commands::archive_handle_worker::run_if_requested();
}
