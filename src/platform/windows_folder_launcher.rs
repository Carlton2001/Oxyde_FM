//! What the Shell runs to open a folder while Oxyde is the default for folders: the
//! small `oxyde-folder-launcher` executable, not Oxyde itself. Explorer starts it for
//! every folder opened, including those opened inside its own windows, so it has to
//! start at once and show nothing: no console, no interface, no runtime. It leaves the
//! folder to the Explorer tab it came from, hands it to the running Oxyde, or starts
//! Oxyde on it; then it stays a while for the program that opened the folder to show a
//! file in it (`windows_shell_window`), and hands that file to Oxyde to select.
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// The launcher's file name, next to the main executable.
pub(super) const EXECUTABLE: &str = "oxyde-folder-launcher.exe";
/// The main executable, as the installer and the portable archive name it, then as
/// Cargo builds it.
const PROGRAMS: [&str; 2] = ["Oxyde_FM.exe", "oxyde.exe"];

fn program() -> Option<PathBuf> {
    let launcher = std::env::current_exe().ok()?;
    PROGRAMS.iter().map(|name| launcher.with_file_name(name)).find(|program| program.is_file())
}

/// How long an Oxyde this launcher started may take to listen for the items to select.
const START: Duration = Duration::from_secs(20);

fn open(locations: &[String]) {
    if super::windows_instance::hand_over(locations) {
        return;
    }
    // No Oxyde runs: this one becomes the session's and opens the folders itself.
    if let Some(program) = program() {
        let _ = std::process::Command::new(program).args(locations).spawn();
    }
}

/// Hands the items to select to the Oxyde the folder went to, which may still be
/// starting.
fn select(items: &[String]) {
    let asked = Instant::now();
    while !super::windows_instance::hand_over_selection(items) && asked.elapsed() < START {
        std::thread::sleep(Duration::from_millis(200));
    }
}

pub fn run() {
    let current = std::env::current_dir().unwrap_or_default();
    let locations = super::windows_startup::locations(std::env::args_os().skip(1), &current);
    let [folder] = locations.as_slice() else { return open(&locations) };
    if super::windows_explorer_browse::browse_where_opened(folder) {
        return;
    }
    let items = super::windows_shell_window::selection(folder, || open(&locations));
    if !items.is_empty() {
        select(&items);
    }
}
