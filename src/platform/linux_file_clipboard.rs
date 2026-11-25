//! Oxyde's own file clipboard. Files travel between Oxyde windows here, as in Zed, and the
//! interface writes their paths to the system clipboard. In a Wayland session, GPUI offers
//! them there as files too, in the formats GNOME, KDE and GTK 4 file managers read; under
//! X11 other applications receive the paths as text. Files other file managers offer are
//! read through `wl-paste`, when installed.
use super::FileClipboard;
use std::sync::{Mutex, MutexGuard};

mod wayland;

struct Selection {
    paths: Vec<String>,
    cut: bool,
}

static SELECTION: Mutex<Option<Selection>> = Mutex::new(None);

fn selection() -> MutexGuard<'static, Option<Selection>> {
    SELECTION.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn hold(paths: Vec<String>, cut: bool) {
    *selection() = (!paths.is_empty()).then_some(Selection { paths, cut });
}

/// Oxyde's selection while the system clipboard still holds its paths. A cut selection
/// is pasted once, as in file managers: its sources move away.
fn current(text: Option<&str>) -> Option<(Vec<String>, bool)> {
    let mut held = selection();
    let selection = held.as_ref()?;
    let current = text == Some(selection.paths.join("\n").as_str());
    if current && !selection.cut {
        return Some((selection.paths.clone(), false));
    }
    let selection = held.take()?;
    current.then_some((selection.paths, true))
}

/// Another file manager would take a trashed item's content without its record.
fn trashed(paths: &[String]) -> bool {
    paths.iter().any(|path| super::linux_trash::inside_trash(std::path::Path::new(path)))
}

pub struct LinuxFileClipboard;

impl FileClipboard for LinuxFileClipboard {
    fn set_files(&self, paths: Vec<String>, cut: bool) -> Result<bool, String> {
        hold(paths, cut);
        Ok(true)
    }

    fn interface_offers_files(&self, paths: &[String]) -> bool {
        wayland::session() && !trashed(paths)
    }

    fn get_files(&self, text: Option<String>) -> Result<(Vec<String>, bool), String> {
        Ok(current(text.as_deref())
            .or_else(wayland::files)
            .unwrap_or_default())
    }

    fn has_file_format(&self) -> bool {
        selection().is_some()
    }

    fn external_files_type(&self) -> Option<&'static str> {
        wayland::session().then_some(wayland::FORMAT)
    }
}

#[cfg(test)]
#[path = "linux_file_clipboard/tests.rs"]
mod tests;
