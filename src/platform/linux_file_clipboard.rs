//! Oxyde's own file clipboard. GPUI reaches the Wayland and X11 clipboards only with text,
//! so files travel between Oxyde windows here, as in Zed; the interface also writes their
//! paths as text for other applications.
use super::FileClipboard;
use std::sync::{Mutex, MutexGuard};

static FILES: Mutex<Option<(Vec<String>, bool)>> = Mutex::new(None);

fn files() -> MutexGuard<'static, Option<(Vec<String>, bool)>> {
    FILES.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub struct LinuxFileClipboard;

impl FileClipboard for LinuxFileClipboard {
    fn set_files(&self, paths: Vec<String>, cut: bool) -> Result<(), String> {
        *files() = (!paths.is_empty()).then_some((paths, cut));
        Ok(())
    }

    fn get_files(&self) -> Result<(Vec<String>, bool), String> {
        let mut files = files();
        // A cut selection is pasted once, as in file managers: its sources move away.
        Ok(match files.as_ref() {
            Some((_, true)) => files.take(),
            other => other.cloned(),
        }
        .unwrap_or_default())
    }

    fn has_file_format(&self) -> bool {
        files().is_some()
    }

    fn discard(&self) {
        *files() = None;
    }
}

#[cfg(test)]
#[path = "linux_file_clipboard/tests.rs"]
mod tests;
