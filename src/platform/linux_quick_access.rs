//! Linux quick access: the GTK bookmarks Nautilus shows as its favorites, in
//! `$XDG_CONFIG_HOME/gtk-3.0/bookmarks`.
use super::QuickAccess;
use crate::models::{CommandError, QuickAccessItem};
use std::path::{Path, PathBuf};

mod bookmarks;

fn file() -> Option<PathBuf> {
    dirs::config_dir().map(|config| config.join("gtk-3.0").join("bookmarks"))
}

fn unavailable() -> CommandError {
    CommandError::SystemError("The user configuration folder is unknown".into())
}

pub struct LinuxQuickAccess;

impl QuickAccess for LinuxQuickAccess {
    fn items(&self) -> Vec<QuickAccessItem> {
        file().map(|file| bookmarks::items(&file)).unwrap_or_default()
    }

    fn pin(&self, path: &str) -> Result<(), CommandError> {
        bookmarks::pin(&file().ok_or_else(unavailable)?, path).map_err(CommandError::from)
    }

    fn unpin(&self, path: &str) -> Result<(), CommandError> {
        bookmarks::unpin(&file().ok_or_else(unavailable)?, path).map_err(CommandError::from)
    }

    /// GTK replaces the file on each change: watch its folder, when it exists.
    fn watch(&self, revision: std::sync::Arc<std::sync::atomic::AtomicU64>)
        -> Result<Option<Box<dyn Send>>, String> {
        let Some(folder) = file().as_deref().and_then(Path::parent).filter(|folder| folder.is_dir()).map(Path::to_path_buf) else {
            return Ok(None);
        };
        crate::systems::quick_access_watcher::watch(&folder, revision).map(|watcher| Some(Box::new(watcher) as Box<dyn Send>))
    }
}

#[cfg(test)]
#[path = "linux_quick_access/tests.rs"]
mod tests;
