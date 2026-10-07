use crate::models::transaction::Transaction;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

fn cleanup_replacement_backups(transaction: &Transaction) {
    if let Some(backups) = &transaction.details.replaced_files {
        for (_, backup) in backups {
            let started = std::time::Instant::now();
            log::info!("history backup removal started: {backup}");
            if let Err(error) = std::fs::remove_file(backup) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("Cannot remove replacement backup {backup}: {error}");
                }
            }
            log::info!("history backup removal finished after {:?}: {backup}", started.elapsed());
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HistoryState {
    pub undo_stack: Vec<Transaction>,
    pub redo_stack: Vec<Transaction>,
}

pub struct HistoryManager(pub Mutex<HistoryState>);

impl Drop for HistoryManager {
    fn drop(&mut self) {
        let state = self
            .0
            .get_mut()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for transaction in state.undo_stack.iter().chain(state.redo_stack.iter()) {
            cleanup_replacement_backups(transaction);
        }
    }
}

impl Default for HistoryManager {
    fn default() -> Self {
        Self(Mutex::new(HistoryState::default()))
    }
}

impl HistoryManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, transaction: Transaction) {
        if let Ok(mut state) = self.0.lock() {
            state.undo_stack.push(transaction);
            for expired in state.redo_stack.drain(..) {
                cleanup_replacement_backups(&expired);
            }

            // Cap history to prevent memory leaks
            if state.undo_stack.len() > 100 {
                let expired = state.undo_stack.remove(0);
                cleanup_replacement_backups(&expired);
            }
        }
    }

    pub fn last_undo(&self) -> Option<Transaction> {
        self.0.lock().ok()?.undo_stack.last().cloned()
    }

    pub fn last_redo(&self) -> Option<Transaction> {
        self.0.lock().ok()?.redo_stack.last().cloned()
    }

    pub fn complete_undo(&self, transaction: &Transaction) -> bool {
        let Ok(mut state) = self.0.lock() else {
            return false;
        };
        if state.undo_stack.last().map(|top| top.id.as_str()) != Some(transaction.id.as_str()) {
            return false;
        }
        state.undo_stack.pop();
        state.redo_stack.push(transaction.clone());
        true
    }

    pub fn complete_redo(&self, transaction: &Transaction) -> bool {
        let Ok(mut state) = self.0.lock() else {
            return false;
        };
        if state.redo_stack.last().map(|top| top.id.as_str()) != Some(transaction.id.as_str()) {
            return false;
        }
        state.redo_stack.pop();
        state.undo_stack.push(transaction.clone());
        if state.undo_stack.len() > 100 {
            let expired = state.undo_stack.remove(0);
            cleanup_replacement_backups(&expired);
        }
        true
    }

    pub fn get_state(&self) -> HistoryState {
        self.0.lock().map(|s| (*s).clone()).unwrap_or_default()
    }

    pub fn clear(&self) {
        log::info!("history clear: waiting for state lock");
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        log::info!("history clear: lock acquired; undo={}; redo={}", state.undo_stack.len(), state.redo_stack.len());
        for transaction in state.undo_stack.iter().chain(state.redo_stack.iter()) {
            cleanup_replacement_backups(transaction);
        }
        state.undo_stack.clear();
        state.redo_stack.clear();
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

        assert!(!history.complete_undo(&transaction()));
        assert_eq!(history.get_state().undo_stack.len(), 1);
        assert!(history.get_state().redo_stack.is_empty());

        assert!(history.complete_undo(&tx));
        assert!(history.get_state().undo_stack.is_empty());
        assert_eq!(history.get_state().redo_stack[0].id, tx.id);
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
}
