//! Cheap display queries: never materialize per-file history for badges or polling.
use super::{FileOpType, FileOperationManager, OpStatus};

pub struct OperationProgress {
    pub id: String,
    pub op_type: FileOpType,
    pub status: OpStatus,
    pub total_bytes: u64,
    pub processed_bytes: u64,
    pub total_files: usize,
    pub processed_files: usize,
}

impl FileOperationManager {
    pub fn operation_counts(&self) -> (usize, usize) {
        let operations = self.operations.lock().unwrap();
        let active = operations
            .values()
            .filter(|operation| !Self::is_final(&operation.lock().unwrap().status))
            .count();
        (operations.len(), active)
    }

    pub fn list_active_progress(&self) -> Vec<OperationProgress> {
        let operations = self.operations.lock().unwrap();
        let mut progress: Vec<_> = operations
            .values()
            .filter_map(|operation| {
                let op = operation.lock().unwrap();
                if Self::is_final(&op.status) {
                    return None;
                }
                Some((
                    op.sequence,
                    OperationProgress {
                        id: op.id.clone(),
                        op_type: op.op_type.clone(),
                        status: op.status.clone(),
                        total_bytes: op.total_bytes,
                        processed_bytes: op.processed_bytes,
                        total_files: op.total_files,
                        processed_files: op.processed_files,
                    },
                ))
            })
            .collect();
        progress.sort_by_key(|(sequence, _)| *sequence);
        progress.into_iter().map(|(_, summary)| summary).collect()
    }

    pub fn operation_state(&self, id: &str) -> Option<(FileOpType, OpStatus)> {
        self.operations.lock().unwrap().get(id).map(|operation| {
            let op = operation.lock().unwrap();
            (op.op_type.clone(), op.status.clone())
        })
    }
}

#[cfg(test)]
#[path = "tests/summaries.rs"]
mod tests;
