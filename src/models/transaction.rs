use std::{collections::HashSet, ffi::OsString, path::PathBuf};

#[derive(Debug, Clone)]
pub enum TransactionType {
    Copy,
    Move,
    Rename,
    Delete,
    NewFolder,
    Restore,
}

#[derive(Debug, Clone)]
pub struct TransactionDetails {
    pub paths: Vec<String>,
    pub target_dir: Option<String>,
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub created_files: Option<Vec<String>>,
    pub created_dirs: Option<Vec<String>>,
    pub replaced_files: Option<Vec<(String, String)>>,
    pub overwritten_files: Option<Vec<String>>,
    pub moved_files: Option<Vec<(String, String)>>,
    pub moved_dirs: Option<Vec<(String, String)>>,
    pub copied_files: Option<Vec<(String, String)>>,
    pub copied_file_hashes: Option<Vec<(String, String)>>,
}

#[derive(Debug, Clone)]
pub(crate) struct RestoreEntry {
    pub destination: PathBuf,
    pub state: RestoreState,
}

#[derive(Debug, Clone)]
pub(crate) enum RestoreState {
    Present,
    Recycling { previous_ids: HashSet<OsString> },
    Recycled { id: OsString },
    Restoring { id: OsString },
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub id: String,
    pub op_type: TransactionType,
    pub details: TransactionDetails,
    pub(crate) restore_journal: Option<Vec<RestoreEntry>>,
}

impl Transaction {
    pub fn new(op_type: TransactionType, details: TransactionDetails) -> Self {
        let restore_journal = if matches!(op_type, TransactionType::Restore) {
            details
                .created_files
                .as_ref()
                .filter(|paths| !paths.is_empty())
                .map(|paths| {
                    paths
                        .iter()
                        .map(|path| RestoreEntry {
                            destination: PathBuf::from(path),
                            state: RestoreState::Present,
                        })
                        .collect()
                })
        } else {
            None
        };
        Self {
            restore_journal,
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
