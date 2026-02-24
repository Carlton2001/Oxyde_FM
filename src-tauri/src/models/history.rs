use std::sync::Mutex;
use serde::{Deserialize, Serialize};
use crate::models::transaction::Transaction;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HistoryState {
    pub undo_stack: Vec<Transaction>,
    pub redo_stack: Vec<Transaction>,
}

pub struct HistoryManager(pub Mutex<HistoryState>);

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
            state.redo_stack.clear(); // New action clears redo history
            
            // Cap history to prevent memory leaks
            if state.undo_stack.len() > 100 {
                state.undo_stack.remove(0);
            }
        }
    }

    pub fn pop_undo(&self) -> Option<Transaction> {
        if let Ok(mut state) = self.0.lock() {
            state.undo_stack.pop()
        } else {
            None
        }
    }

    pub fn pop_redo(&self) -> Option<Transaction> {
        if let Ok(mut state) = self.0.lock() {
            state.redo_stack.pop()
        } else {
            None
        }
    }

    pub fn push_redo(&self, transaction: Transaction) {
        if let Ok(mut state) = self.0.lock() {
            state.redo_stack.push(transaction);
        }
    }
    
    // For manual push back to undo if redo succeeds
    pub fn push_undo_raw(&self, transaction: Transaction) {
        if let Ok(mut state) = self.0.lock() {
            state.undo_stack.push(transaction);
        }
    }

    pub fn get_state(&self) -> HistoryState {
        self.0.lock().map(|s| (*s).clone()).unwrap_or_default()
    }
    
    pub fn clear(&self) {
        if let Ok(mut state) = self.0.lock() {
            state.undo_stack.clear();
            state.redo_stack.clear();
        }
    }
}
