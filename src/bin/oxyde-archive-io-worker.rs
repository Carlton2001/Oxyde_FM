//! Headless archive codec executable. Keep UI and media DLL imports out of its
//! bootstrap: they are not required by the zero-capability archive sandbox.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "windows")]
    oxyde_lib::commands::archive_io_worker::run_if_requested();
}
