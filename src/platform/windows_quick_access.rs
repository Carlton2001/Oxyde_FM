//! Windows quick access: the Shell's Home/Quick access folder and its pin verbs.
use super::QuickAccess;
use crate::models::{CommandError, QuickAccessItem};

pub struct WindowsQuickAccess;

impl QuickAccess for WindowsQuickAccess {
    fn items(&self) -> Vec<QuickAccessItem> {
        super::windows_shell::get_quick_access_items().unwrap_or_else(|error| {
            log::warn!("Cannot read Quick access: {error}");
            Vec::new()
        })
    }

    fn pin(&self, path: &str) -> Result<(), CommandError> {
        super::windows_shell::execute_shell_verb_by_canonical_name(
            path,
            &["pintohome", "pintofavorites"],
        )
    }

    fn unpin(&self, path: &str) -> Result<(), CommandError> {
        super::windows_shell::execute_shell_verb_by_canonical_name(
            path,
            &["unpinfromhome", "unpinfromquickaccess"],
        )
    }

    fn watch(&self, revision: std::sync::Arc<crate::utils::change_signal::ChangeSignal>)
        -> Result<Option<Box<dyn Send>>, String> {
        let root = std::env::var_os("APPDATA").ok_or("APPDATA is unavailable")?;
        let recent = std::path::PathBuf::from(root).join("Microsoft/Windows/Recent");
        crate::systems::quick_access_watcher::watch(&recent, revision).map(|watcher| Some(Box::new(watcher) as Box<dyn Send>))
    }
}
