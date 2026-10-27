use crate::models::transaction::{Transaction, TransactionType};
use std::sync::Mutex;

fn cleanup_metadata_backups(transaction: &Transaction) {
    for backup in transaction
        .details
        .trash_metadata_backups
        .as_deref()
        .unwrap_or_default()
    {
        // No proof means preservation, including when a history entry expires.
        let identity = transaction
            .details
            .moved_file_identities
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find(|(path, _)| path == backup)
            .and_then(|(_, proof)| proof.snapshot());
        let hash = transaction
            .details
            .copied_file_hashes
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find(|(path, _)| path == backup)
            .map(|(_, hash)| hash.as_str());
        let result = match (identity, hash) {
            (Some(identity), Some(hash)) => {
                crate::systems::file_copy::remove_verified_output_identity(
                    std::path::Path::new(backup),
                    hash,
                    Some(identity),
                )
            }
            _ => Err(std::io::Error::other(
                "Trash metadata backup has no captured identity/hash; preserved",
            )),
        };
        if let Err(error) = result {
            if error.kind() != std::io::ErrorKind::NotFound {
                log::warn!("Cannot remove trash metadata backup {backup}: {error}");
            }
        }
    }
}

#[derive(Debug, Default)]
#[cfg_attr(test, derive(Clone))]
pub(crate) struct HistoryState {
    pub(crate) undo_stack: Vec<Transaction>,
    pub(crate) redo_stack: Vec<Transaction>,
}

/// Small snapshot for action buttons, without cloning file journals.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HistorySummary {
    pub undo: Option<TransactionType>,
    pub redo: Option<TransactionType>,
    pub undo_blocked_by_overwrite: bool,
}

#[derive(Default)]
struct HistoryData {
    history: HistoryState,
    active: bool,
    pending: Vec<Transaction>,
    clear_requested: bool,
}

#[derive(Default)]
pub struct HistoryManager {
    state: Mutex<HistoryData>,
}

#[derive(Clone, Copy)]
enum Direction {
    Undo,
    Redo,
}

/// Owns the history reservation, without holding its mutex during file I/O.
/// Dropping without completion leaves the selected transaction in its stack.
pub struct HistoryOperation<'a> {
    manager: &'a HistoryManager,
    transaction: Transaction,
    direction: Direction,
    completed: bool,
}

impl HistoryOperation<'_> {
    pub fn transaction(&self) -> &Transaction {
        &self.transaction
    }

    pub fn complete(mut self) {
        self.completed = true;
    }

    pub(crate) fn complete_with_details(mut self, details: crate::models::TransactionDetails) {
        self.transaction.details = details;
        self.completed = true;
    }
}

impl Drop for HistoryOperation<'_> {
    fn drop(&mut self) {
        let mut expired = Vec::new();
        {
            let mut data = self.manager.state.lock().unwrap_or_else(|e| e.into_inner());
            if data.clear_requested {
                expired.append(&mut data.history.undo_stack);
                expired.append(&mut data.history.redo_stack);
                expired.append(&mut data.pending);
                data.clear_requested = false;
            } else {
                if self.completed {
                    match self.direction {
                        Direction::Undo => {
                            data.history.undo_stack.pop().expect("reserved undo entry");
                            data.history.redo_stack.push(self.transaction.clone());
                        }
                        Direction::Redo => {
                            data.history.redo_stack.pop().expect("reserved redo entry");
                            data.history.undo_stack.push(self.transaction.clone());
                        }
                    }
                }
                // New actions invalidate redo only after the reserved action finishes.
                for transaction in std::mem::take(&mut data.pending) {
                    push_entry(&mut data.history, transaction, &mut expired);
                }
            }
            data.active = false;
        }
        for transaction in expired {
            cleanup_metadata_backups(&transaction);
        }
    }
}

fn push_entry(state: &mut HistoryState, transaction: Transaction, expired: &mut Vec<Transaction>) {
    state.undo_stack.push(transaction);
    expired.append(&mut state.redo_stack);
    if state.undo_stack.len() > 100 {
        expired.push(state.undo_stack.remove(0));
    }
}

impl Drop for HistoryManager {
    fn drop(&mut self) {
        let data = self.state.get_mut().unwrap_or_else(|e| e.into_inner());
        for transaction in data
            .history
            .undo_stack
            .iter()
            .chain(data.history.redo_stack.iter())
            .chain(data.pending.iter())
        {
            cleanup_metadata_backups(transaction);
        }
    }
}

impl HistoryManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, transaction: Transaction) {
        let mut expired = Vec::new();
        {
            let mut data = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if data.active {
                data.pending.push(transaction);
            } else {
                push_entry(&mut data.history, transaction, &mut expired);
            }
        }
        for transaction in expired {
            cleanup_metadata_backups(&transaction);
        }
    }

    /// Update only an idle NewFolder entry whose recorded object was renamed.
    /// A reservation's source paths must never change while filesystem work runs.
    pub(crate) fn relocate_created_folder(
        &self,
        old: &str,
        new: &str,
        identity: crate::models::transaction::HistoryFileIdentity,
    ) -> bool {
        let mut data = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if data.active {
            return false;
        }
        if let Some(transaction) = data
            .history
            .undo_stack
            .iter_mut()
            .rev()
            .find(|transaction| {
                transaction.op_type == TransactionType::NewFolder
                    && transaction.details.paths.as_slice() == [old]
                    && transaction.details.new_folder_identity == Some(identity)
            })
        {
            transaction.details.paths[0] = new.to_owned();
            return true;
        }
        false
    }

    fn begin(&self, direction: Direction) -> Result<Option<HistoryOperation<'_>>, &'static str> {
        let mut data = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if data.active {
            return Err("An undo or redo operation is already running");
        }
        let transaction = match direction {
            Direction::Undo => data.history.undo_stack.last(),
            Direction::Redo => data.history.redo_stack.last(),
        }
        .cloned();
        Ok(transaction.map(|transaction| {
            data.active = true;
            HistoryOperation {
                manager: self,
                transaction,
                direction,
                completed: false,
            }
        }))
    }

    pub fn begin_undo(&self) -> Result<Option<HistoryOperation<'_>>, &'static str> {
        self.begin(Direction::Undo)
    }

    pub fn begin_redo(&self) -> Result<Option<HistoryOperation<'_>>, &'static str> {
        self.begin(Direction::Redo)
    }

    pub fn summary(&self) -> HistorySummary {
        let data = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let undo = data.history.undo_stack.last();
        HistorySummary {
            undo: undo.map(|transaction| transaction.op_type),
            redo: data
                .history
                .redo_stack
                .last()
                .map(|transaction| transaction.op_type),
            undo_blocked_by_overwrite: undo.is_some_and(Transaction::undo_blocked_by_overwrite),
        }
    }

    #[cfg(test)]
    pub(crate) fn get_state(&self) -> HistoryState {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .history
            .clone()
    }

    /// Returns false when cleanup is deferred until the reserved action finishes.
    pub fn clear(&self) -> bool {
        let expired = {
            let mut data = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if data.active {
                data.clear_requested = true;
                log::info!("history clear: deferred until the reserved action finishes");
                return false;
            }
            let mut expired = std::mem::take(&mut data.history.undo_stack);
            expired.append(&mut data.history.redo_stack);
            expired
        };
        for transaction in expired {
            cleanup_metadata_backups(&transaction);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::transaction::{TransactionDetails, TransactionType};

    fn bind_metadata_backup(tx: &mut Transaction, path: &std::path::Path) {
        let name = path.to_string_lossy().into_owned();
        let file = std::fs::File::open(path).unwrap();
        tx.details.moved_file_identities = Some(vec![(
            name.clone(),
            crate::models::transaction::HistoryMoveIdentity::from_file(&file).unwrap(),
        )]);
        tx.details.copied_file_hashes = Some(vec![(
            name,
            blake3::hash(&std::fs::read(path).unwrap())
                .to_hex()
                .to_string(),
        )]);
    }

    fn transaction() -> Transaction {
        Transaction::new(
            TransactionType::Copy,
            TransactionDetails {
                deleted_items: None,
                created_links: None,
                moved_links: None,
                directory_attributes: None,
                renamed_item_identity: None,
                moved_directory_identities: None,
                new_folder_identity: None,
                moved_file_identities: None,
                copied_directory_identities: None,
                copied_file_identities: None,
                paths: Vec::new(),
                target_dir: None,
                old_path: None,
                new_path: None,
                created_files: None,
                created_dirs: None,
                overwritten_files: None,
                moved_files: None,
                trash_metadata_backups: None,
                moved_dirs: None,
                copied_files: None,
                copied_file_hashes: None,
            },
        )
    }

    #[test]
    fn summary_preserves_operation_labels_and_copy_overwrite_blocking() {
        let history = HistoryManager::new();
        assert_eq!(history.summary(), HistorySummary::default());
        for kind in [
            TransactionType::Copy,
            TransactionType::Move,
            TransactionType::Rename,
            TransactionType::Delete,
            TransactionType::NewFolder,
        ] {
            let mut entry = transaction();
            entry.op_type = kind;
            entry.details.overwritten_files = Some(vec!["existing file".into()]);
            history.push(entry);
            assert_eq!(history.summary().undo, Some(kind));
            assert_eq!(
                history.summary().undo_blocked_by_overwrite,
                kind == TransactionType::Copy
            );
            history.begin_undo().unwrap().unwrap().complete();
            assert_eq!(
                history.summary(),
                HistorySummary {
                    undo: None,
                    redo: Some(kind),
                    undo_blocked_by_overwrite: false,
                }
            );
            history.begin_redo().unwrap().unwrap().complete();
            assert_eq!(history.summary().undo, Some(kind));
            assert_eq!(history.summary().redo, None);
            assert!(history.clear());
            assert_eq!(history.summary(), HistorySummary::default());
        }
    }

    #[test]
    fn summary_uses_committed_stacks_during_reservation_and_deferred_clear() {
        let history = HistoryManager::new();
        history.push(transaction());
        let selected = history.begin_undo().unwrap().unwrap();
        let before = history.summary();
        let mut pending = transaction();
        pending.op_type = TransactionType::Rename;
        history.push(pending);
        assert_eq!(history.summary(), before);
        selected.complete();
        assert_eq!(history.summary().undo, Some(TransactionType::Rename));
        assert_eq!(history.summary().redo, None);
        let selected = history.begin_undo().unwrap().unwrap();
        assert!(!history.clear());
        assert_eq!(history.summary().undo, Some(TransactionType::Rename));
        drop(selected);
        assert_eq!(history.summary(), HistorySummary::default());
    }

    #[test]
    #[ignore = "manual in-memory comparison of history preparation for four panes"]
    fn profile_history_summary_for_multiple_panes() {
        let history = HistoryManager::new();
        for entry in 0..100 {
            let mut tx = transaction();
            let paths: Vec<_> = (0..128)
                .map(|file| {
                    format!(
                        "C:\\fixture\\entry-{entry}\\{}\\file-{file}.txt",
                        "long directory name ".repeat(5)
                    )
                })
                .collect();
            tx.details.paths = paths.clone();
            tx.details.created_files = Some(paths.clone());
            tx.details.copied_files = Some(
                paths
                    .iter()
                    .map(|path| (path.clone(), path.clone()))
                    .collect(),
            );
            tx.details.copied_file_hashes = Some(
                paths
                    .iter()
                    .map(|path| (path.clone(), "a".repeat(64)))
                    .collect(),
            );
            history.push(tx);
        }
        const FRAMES: usize = 50;
        const PANES: usize = 4;
        let start = std::time::Instant::now();
        for _ in 0..FRAMES * PANES {
            std::hint::black_box(history.get_state());
        }
        let full = start.elapsed();
        let start = std::time::Instant::now();
        for _ in 0..FRAMES * PANES {
            std::hint::black_box(history.summary());
        }
        let summary = start.elapsed();
        eprintln!(
            "100 transactions x 128 files; {FRAMES} frames x {PANES} panes: full clone {full:?}, summary {summary:?}; summary size {} bytes",
            std::mem::size_of::<HistorySummary>()
        );
    }

    #[test]
    fn undo_history_moves_only_after_success_is_reported() {
        let history = HistoryManager::new();
        let tx = transaction();
        history.push(tx.clone());

        drop(history.begin_undo().unwrap().unwrap());
        assert_eq!(history.get_state().undo_stack.len(), 1);
        assert!(history.get_state().redo_stack.is_empty());

        history.begin_undo().unwrap().unwrap().complete();
        assert!(history.get_state().undo_stack.is_empty());
        assert_eq!(history.get_state().redo_stack[0].id, tx.id);
        history.begin_redo().unwrap().unwrap().complete();
        assert_eq!(history.get_state().undo_stack[0].id, tx.id);
        assert!(history.get_state().redo_stack.is_empty());
    }

    #[test]
    fn new_entry_during_undo_preserves_completion() {
        let history = HistoryManager::new();
        let first = transaction();
        history.push(first.clone());
        let selected = history.begin_undo().unwrap().unwrap();
        assert_eq!(selected.transaction().id, first.id);
        let second = transaction();
        history.push(second.clone());
        assert_eq!(history.get_state().undo_stack[0].id, first.id);
        selected.complete();
        assert_eq!(history.get_state().undo_stack.len(), 1);
        assert_eq!(history.get_state().undo_stack[0].id, second.id);
        assert!(history.get_state().redo_stack.is_empty());
    }

    #[test]
    fn new_entry_during_redo_follows_the_reapplied_transaction() {
        let history = HistoryManager::new();
        let first = transaction();
        history.push(first.clone());
        history.begin_undo().unwrap().unwrap().complete();
        let selected = history.begin_redo().unwrap().unwrap();
        let second = transaction();
        history.push(second.clone());
        selected.complete();
        let state = history.get_state();
        assert_eq!(state.undo_stack.len(), 2);
        assert_eq!(state.undo_stack[0].id, first.id);
        assert_eq!(state.undo_stack[1].id, second.id);
        assert!(state.redo_stack.is_empty());
    }

    #[test]
    fn concurrent_requests_cannot_reserve_the_same_or_opposite_action() {
        let history = HistoryManager::new();
        history.push(transaction());
        let selected = history.begin_undo().unwrap().unwrap();
        std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    assert!(history.begin_undo().is_err());
                    assert!(history.begin_redo().is_err());
                    history.push(transaction());
                })
                .join()
                .unwrap();
        });
        drop(selected);
        assert_eq!(history.get_state().undo_stack.len(), 2);
        assert!(history.begin_undo().unwrap().is_some());
    }

    #[test]
    fn failed_redo_keeps_its_entry_and_releases_reservation() {
        let history = HistoryManager::new();
        let tx = transaction();
        history.push(tx.clone());
        history.begin_undo().unwrap().unwrap().complete();
        drop(history.begin_redo().unwrap().unwrap());
        assert_eq!(history.get_state().redo_stack[0].id, tx.id);
        history.begin_redo().unwrap().unwrap().complete();
        assert_eq!(history.get_state().undo_stack[0].id, tx.id);
    }

    #[test]
    fn pending_entries_keep_their_order_and_history_limit_after_failure() {
        let history = HistoryManager::new();
        history.push(transaction());
        let selected = history.begin_undo().unwrap().unwrap();
        let pending: Vec<_> = (0..102).map(|_| transaction()).collect();
        for tx in &pending {
            history.push(tx.clone());
        }
        drop(selected);
        let state = history.get_state();
        assert_eq!(state.undo_stack.len(), 100);
        assert_eq!(
            state.undo_stack.iter().map(|tx| &tx.id).collect::<Vec<_>>(),
            pending[2..].iter().map(|tx| &tx.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn panic_releases_reservation_and_applies_pending_entries() {
        let history = HistoryManager::new();
        history.push(transaction());
        let next = transaction();
        let result = std::panic::catch_unwind(|| {
            let _selected = history.begin_undo().unwrap().unwrap();
            history.push(next.clone());
            panic!("simulated worker panic");
        });
        assert!(result.is_err());
        assert_eq!(history.get_state().undo_stack.last().unwrap().id, next.id);
        assert!(history.begin_undo().unwrap().is_some());
    }

    #[test]
    fn clear_during_action_preserves_backups_until_completion() {
        let temp = tempfile::tempdir().unwrap();
        let backup = temp.path().join("metadata.bak");
        std::fs::write(&backup, b"original content").unwrap();
        let history = HistoryManager::new();
        let mut tx = transaction();
        tx.details.trash_metadata_backups = Some(vec![backup.to_string_lossy().into_owned()]);
        bind_metadata_backup(&mut tx, &backup);
        history.push(tx);
        let selected = history.begin_undo().unwrap().unwrap();
        history.push(transaction());
        assert!(!history.clear());
        assert!(backup.exists());
        selected.complete();
        assert!(!backup.exists());
        assert!(history.get_state().undo_stack.is_empty());
        assert!(history.get_state().redo_stack.is_empty());
        assert!(history.clear());
    }

    #[test]
    fn metadata_backup_is_removed_when_history_is_dropped() {
        let temp = tempfile::tempdir().unwrap();
        let destination = temp.path().join("destination.txt");
        let backup = temp.path().join(".oxyde-metadata.bak");
        std::fs::write(&destination, b"new content").unwrap();
        std::fs::write(&backup, b"old content").unwrap();

        let history = HistoryManager::new();
        let mut tx = transaction();
        tx.details.trash_metadata_backups = Some(vec![backup.to_string_lossy().into_owned()]);
        bind_metadata_backup(&mut tx, &backup);
        history.push(tx);
        assert!(backup.exists());

        drop(history);
        assert!(!backup.exists());
        assert_eq!(std::fs::read(destination).unwrap(), b"new content");
    }
    #[test]
    fn detached_trash_metadata_expires_on_clear_and_on_redo_invalidation() {
        let root = tempfile::tempdir().unwrap();
        let history = HistoryManager::new();
        let backup = root.path().join("metadata.tmp");
        std::fs::write(&backup, b"metadata fixture").unwrap();
        let mut tx = transaction();
        tx.details.trash_metadata_backups = Some(vec![backup.to_string_lossy().into_owned()]);
        bind_metadata_backup(&mut tx, &backup);
        history.push(tx.clone());
        history.begin_undo().unwrap().unwrap().complete();
        history.push(transaction());
        assert!(!backup.exists());
        std::fs::write(&backup, b"metadata fixture").unwrap();
        // A new detach creates a new journal proof, rather than reusing an expired one.
        bind_metadata_backup(&mut tx, &backup);
        history.push(tx);
        assert!(history.clear());
        assert!(!backup.exists());
    }
    #[test]
    fn metadata_backup_cleanup_preserves_identical_replacements_in_all_expiry_paths() {
        for expiry in 0..4 {
            let root = tempfile::tempdir().unwrap();
            let backup = root.path().join("metadata.tmp");
            let saved = root.path().join("original-metadata.tmp");
            std::fs::write(&backup, b"selected metadata").unwrap();
            let mut tx = transaction();
            tx.details.trash_metadata_backups = Some(vec![backup.to_string_lossy().into_owned()]);
            bind_metadata_backup(&mut tx, &backup);
            let history = HistoryManager::new();
            history.push(tx);
            std::fs::rename(&backup, &saved).unwrap();
            std::fs::write(&backup, b"selected metadata").unwrap();
            match expiry {
                0 => drop(history),
                1 => {
                    assert!(history.clear());
                }
                2 => {
                    history.begin_undo().unwrap().unwrap().complete();
                    history.push(transaction());
                }
                _ => {
                    for _ in 0..100 {
                        history.push(transaction());
                    }
                }
            }
            assert_eq!(std::fs::read(&backup).unwrap(), b"selected metadata");
            assert_eq!(std::fs::read(&saved).unwrap(), b"selected metadata");
        }
    }

    #[test]
    fn metadata_backup_cleanup_requires_identity_and_original_contents() {
        for issue in 0..3 {
            let root = tempfile::tempdir().unwrap();
            let backup = root.path().join("metadata.tmp");
            std::fs::write(&backup, b"selected metadata").unwrap();
            let mut tx = transaction();
            tx.details.trash_metadata_backups = Some(vec![backup.to_string_lossy().into_owned()]);
            bind_metadata_backup(&mut tx, &backup);
            match issue {
                0 => tx.details.moved_file_identities = None,
                1 => tx.details.copied_file_hashes = None,
                _ => std::fs::write(&backup, b"independent edits").unwrap(),
            }
            let expected = std::fs::read(&backup).unwrap();
            let history = HistoryManager::new();
            history.push(tx);
            assert!(history.clear());
            assert_eq!(std::fs::read(&backup).unwrap(), expected);
        }
    }
}
