//! Oxyde's own file clipboard. GPUI reaches the Wayland and X11 clipboards only with text,
//! so files travel between Oxyde windows here, as in Zed. `wl-copy` and `wl-paste`, when
//! installed, exchange them with file managers in their format; otherwise the interface
//! writes their paths as text.
use super::FileClipboard;
use std::process::Child;
use std::sync::{Mutex, MutexGuard};

mod wayland;

struct Selection {
    paths: Vec<String>,
    cut: bool,
    /// The `wl-copy` process offering the selection; it exits once replaced.
    server: Option<Child>,
}

impl Selection {
    /// Whether the system clipboard still offers the selection, when known.
    fn served(&mut self) -> Option<bool> {
        self.server.as_mut().map(|server| matches!(server.try_wait(), Ok(None)))
    }

    fn release(mut self) {
        if let Some(server) = &mut self.server {
            let _ = server.kill();
            let _ = server.wait();
        }
    }
}

static SELECTION: Mutex<Option<Selection>> = Mutex::new(None);

fn selection() -> MutexGuard<'static, Option<Selection>> {
    SELECTION.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn hold(paths: Vec<String>, cut: bool, server: Option<Child>) {
    let new = (!paths.is_empty()).then_some(Selection { paths, cut, server });
    if let Some(old) = std::mem::replace(&mut *selection(), new) {
        old.release();
    }
}

/// Oxyde's selection while the system clipboard still offers it. A cut selection is
/// pasted once, as in file managers: its sources move away.
fn current(text: Option<&str>) -> Option<(Vec<String>, bool)> {
    let mut held = selection();
    let selection = held.as_mut()?;
    let current = selection
        .served()
        .unwrap_or_else(|| text == Some(selection.paths.join("\n").as_str()));
    if current && !selection.cut {
        return Some((selection.paths.clone(), false));
    }
    let selection = held.take()?;
    let files = current.then(|| (selection.paths.clone(), true));
    selection.release();
    files
}

pub struct LinuxFileClipboard;

impl FileClipboard for LinuxFileClipboard {
    fn set_files(&self, paths: Vec<String>, cut: bool) -> Result<bool, String> {
        hold(Vec::new(), false, None);
        // Another file manager would take a trashed item's content without its record.
        let trashed = paths.iter().any(|path| super::linux_trash::inside_trash(std::path::Path::new(path)));
        let server = if trashed { None } else { wayland::offer(&paths, cut) };
        let text = server.is_none();
        hold(paths, cut, server);
        Ok(text)
    }

    fn get_files(&self, text: Option<String>) -> Result<(Vec<String>, bool), String> {
        Ok(current(text.as_deref())
            .or_else(wayland::files)
            .unwrap_or_default())
    }

    fn has_file_format(&self) -> bool {
        selection().as_mut().is_some_and(|selection| selection.served().unwrap_or(true))
    }
}

#[cfg(test)]
#[path = "linux_file_clipboard/tests.rs"]
mod tests;
