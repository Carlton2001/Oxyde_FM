//! What the Shell runs to open a folder while Oxyde is the default for folders: the
//! small `oxyde-folder-launcher` executable, not Oxyde itself. Explorer starts it for
//! every folder opened, including those opened inside its own windows, so it has to
//! start at once and show nothing: no console, no interface, no runtime. It leaves the
//! folder to the Explorer tab it came from, hands it to the running Oxyde, or starts
//! Oxyde on it.
use std::path::PathBuf;

/// The launcher's file name, next to the main executable.
pub(super) const EXECUTABLE: &str = "oxyde-folder-launcher.exe";
/// The main executable, as the installer and the portable archive name it, then as
/// Cargo builds it.
const PROGRAMS: [&str; 2] = ["Oxyde_FM.exe", "oxyde.exe"];

fn program() -> Option<PathBuf> {
    let launcher = std::env::current_exe().ok()?;
    PROGRAMS.iter().map(|name| launcher.with_file_name(name)).find(|program| program.is_file())
}

pub fn run() {
    let current = std::env::current_dir().unwrap_or_default();
    let locations = super::windows_startup::locations(std::env::args_os().skip(1), &current);
    if let [folder] = locations.as_slice() {
        if super::windows_explorer_browse::browse_where_opened(folder) {
            return;
        }
    }
    if super::windows_instance::hand_over(&locations) {
        return;
    }
    // No Oxyde runs: this one becomes the session's and opens the folders itself.
    if let Some(program) = program() {
        let _ = std::process::Command::new(program).args(&locations).spawn();
    }
}
