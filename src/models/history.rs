use crate::models::transaction::Transaction;
use std::sync::Mutex;

fn cleanup_replacement_backups(transaction: &Transaction) {
    for backup in transaction
        .details
        .trash_metadata_backups
        .as_deref()
        .unwrap_or_default()
    {
        if let Err(error) = std::fs::remove_file(backup) {
            if error.kind() != std::io::ErrorKind::NotFound {
                log::warn!("Cannot remove trash metadata backup {backup}: {error}");
            }
        }
    }
    if let Some(backups) = &transaction.details.replaced_files {
        for (_, backup) in backups {
            let started = std::time::Instant::now();
            log::info!("history backup removal started: {backup}");
            if let Err(error) = std::fs::remove_file(backup) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("Cannot remove replacement backup {backup}: {error}");
                }
            }
            log::info!(
                "history backup removal finished after {:?}: {backup}",
                started.elapsed()
            );
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct HistoryState {
    pub undo_stack: Vec<Transaction>,
    pub redo_stack: Vec<Transaction>,
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
                            let transaction =
                                data.history.undo_stack.pop().expect("reserved undo entry");
                            data.history.redo_stack.push(transaction);
                        }
                        Direction::Redo => {
                            let transaction =
                                data.history.redo_stack.pop().expect("reserved redo entry");
                            data.history.undo_stack.push(transaction);
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
            cleanup_replacement_backups(&transaction);
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
            cleanup_replacement_backups(transaction);
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
            cleanup_replacement_backups(&transaction);
        }
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

    pub fn get_state(&self) -> HistoryState {
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
            cleanup_replacement_backups(&transaction);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::transaction::{TransactionDetails, TransactionType};

    fn transaction() -> Transaction {
        Transaction::new(
            TransactionType::Copy,
            TransactionDetails {
                paths: Vec::new(),
                target_dir: None,
                old_path: None,
                new_path: None,
                created_files: None,
                created_dirs: None,
                replaced_files: None,
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
        let backup = temp.path().join("replacement.bak");
        std::fs::write(&backup, b"original content").unwrap();
        let history = HistoryManager::new();
        let mut tx = transaction();
        tx.details.replaced_files = Some(vec![(
            "destination".into(),
            backup.to_string_lossy().into_owned(),
        )]);
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
    fn replacement_backup_is_removed_when_history_is_dropped() {
        let temp = tempfile::tempdir().unwrap();
        let destination = temp.path().join("destination.txt");
        let backup = temp.path().join(".oxyde-replacement.bak");
        std::fs::write(&destination, b"new content").unwrap();
        std::fs::write(&backup, b"old content").unwrap();

        let history = HistoryManager::new();
        let mut tx = transaction();
        tx.details.replaced_files = Some(vec![(
            destination.to_string_lossy().into_owned(),
            backup.to_string_lossy().into_owned(),
        )]);
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
        history.push(tx.clone());
        history.begin_undo().unwrap().unwrap().complete();
        history.push(transaction());
        assert!(!backup.exists());
        std::fs::write(&backup, b"metadata fixture").unwrap();
        history.push(tx);
        assert!(history.clear());
        assert!(!backup.exists());
    }
}
