#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionType {
    Copy,
    Move,
    Rename,
    Delete,
    NewFolder,
}

#[derive(Debug, Clone)]
pub struct TransactionDetails {
    pub paths: Vec<String>,
    pub deleted_items: Option<Vec<HistoryDeletedItem>>,
    pub target_dir: Option<String>,
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub created_files: Option<Vec<String>>,
    pub created_dirs: Option<Vec<String>>,
    pub overwritten_files: Option<Vec<String>>,
    pub moved_files: Option<Vec<(String, String)>>,
    pub moved_file_identities: Option<Vec<(String, HistoryMoveIdentity)>>,
    pub trash_metadata_backups: Option<Vec<String>>,
    pub renamed_item_identity: Option<HistoryFileIdentity>,
    pub moved_directory_identities: Option<Vec<(String, HistoryDirectoryIdentity)>>,
    pub moved_dirs: Option<Vec<(String, String)>>,
    pub copied_files: Option<Vec<(String, String)>>,
    pub copied_file_hashes: Option<Vec<(String, String)>>,
    pub copied_file_identities: Option<Vec<(String, HistoryFileIdentity)>>,
    pub copied_directory_identities: Option<Vec<(String, HistoryDirectoryIdentity)>>,
    pub new_folder_identity: Option<HistoryFileIdentity>,
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub id: String,
    pub op_type: TransactionType,
    pub details: TransactionDetails,
    // Future: backup references for safe undo?
}

impl Transaction {
    pub fn new(op_type: TransactionType, details: TransactionDetails) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            op_type,
            details,
        }
    }

    /// Copies that overwrote existing files cannot be undone because the old
    /// contents are deliberately not kept as backups.
    pub fn undo_blocked_by_overwrite(&self) -> bool {
        matches!(&self.op_type, TransactionType::Copy)
            && self
                .details
                .overwritten_files
                .as_ref()
                .is_some_and(|files| !files.is_empty())
    }
}
/// Journal snapshot without a long-lived handle: no file locks or delayed deletion.
/// Identifiers may still be reused after deletion; creation time is an extra check,
/// not a cryptographic proof that metadata cannot be forged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryFileIdentity {
    identity: crate::platform::FileIdentity,
    created: std::time::SystemTime,
}

impl HistoryFileIdentity {
    pub(crate) fn from_file(file: &std::fs::File) -> std::io::Result<Self> {
        Ok(Self {
            identity: crate::platform::platform().file_identity(file)?,
            created: file.metadata()?.created()?,
        })
    }

    pub(crate) fn matches(&self, file: &std::fs::File) -> std::io::Result<bool> {
        Ok(*self == Self::from_file(file)?)
    }
}

/// Shared journal value so rollback can refresh an exclusively recreated directory,
/// including during unwinding. Clones never retain a filesystem handle.
#[derive(Debug, Clone)]
pub struct HistoryDirectoryIdentity {
    current: std::sync::Arc<std::sync::Mutex<Option<HistoryFileIdentity>>>,
}

/// A Move proof follows the logical file through cross-volume publication and
/// rollback. It is shared by history snapshots and never retains a file handle.
pub type HistoryMoveIdentity = HistoryDirectoryIdentity;

impl HistoryDirectoryIdentity {
    /// No object exists yet: only an exclusive journal creation may supply its proof.
    pub(crate) fn vacant() -> Self {
        Self {
            current: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }

    pub(crate) fn from_file(file: &std::fs::File) -> std::io::Result<Self> {
        Ok(Self {
            current: std::sync::Arc::new(std::sync::Mutex::new(Some(
                HistoryFileIdentity::from_file(file)?,
            ))),
        })
    }

    pub(crate) fn snapshot(&self) -> Option<HistoryFileIdentity> {
        *self.current.lock().unwrap()
    }

    pub(crate) fn refresh_from_file(&self, file: &std::fs::File) -> std::io::Result<()> {
        let captured = HistoryFileIdentity::from_file(file);
        // A failed capture invalidates the old proof; it must never be reused.
        *self.current.lock().unwrap() = captured.as_ref().ok().copied();
        captured.map(|_| ())
    }
}

/// A deletion records the object before the Shell call. Shared phase tracking
/// preserves completed effects when a batch fails and history remains reserved.
#[derive(Debug, Clone)]
pub struct HistoryDeletedItem {
    pub path: String,
    #[cfg(target_os = "windows")]
    pub(crate) identity: Option<HistoryFileIdentity>,
    #[cfg(target_os = "windows")]
    restored: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
#[cfg(target_os = "windows")]
impl HistoryDeletedItem {
    pub(crate) fn new(path: String, identity: Option<HistoryFileIdentity>) -> Self {
        Self {
            path,
            identity,
            restored: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
    pub(crate) fn is_restored(&self) -> bool {
        self.restored.load(std::sync::atomic::Ordering::Acquire)
    }
    pub(crate) fn set_restored(&self, value: bool) {
        self.restored
            .store(value, std::sync::atomic::Ordering::Release);
    }
}
