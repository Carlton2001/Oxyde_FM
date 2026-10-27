//! Quick access domain: folders the user pins beside the drives, shared with the
//! system file manager.
use crate::models::{CommandError, QuickAccessItem};

/// Calls may read or write system files: run them on a background worker.
pub trait QuickAccess: Send + Sync {
    /// Pinned folders, in the system file manager's order.
    fn items(&self) -> Vec<QuickAccessItem>;
    fn pin(&self, path: &str) -> Result<(), CommandError>;
    fn unpin(&self, path: &str) -> Result<(), CommandError>;
    /// The returned guard owns the watcher; callbacks only increment `revision`.
    fn watch(&self, revision: std::sync::Arc<std::sync::atomic::AtomicU64>)
        -> Result<Option<Box<dyn Send>>, String>;
}
