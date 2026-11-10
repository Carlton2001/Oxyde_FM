//! Opens folders for the Shell while Oxyde is their default: see
//! `platform/windows_folder_launcher.rs`. Never a console program: Explorer starts it
//! at each folder opened.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
fn main() {
    #[cfg(target_os = "windows")]
    oxyde_lib::platform::run_folder_launcher();
}
