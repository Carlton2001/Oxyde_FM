//! A failed operation run again, once its archive is unlocked, for what the failure left
//! untouched: what was done is not done twice and the conflict decisions still apply.
use super::{FileOperation, FileOperationManager, ItemOutcome};
use std::path::Path;
use std::sync::atomic::Ordering;

impl FileOperationManager {
    /// Operation `id` again for the sources its failure left untouched, with its conflict
    /// decisions and settings. None when nothing is left, or when a source may hold part of
    /// the work: running it again could do that part twice.
    pub fn untouched_retry(&self, id: &str) -> Option<FileOperation> {
        let operations = self.operations.lock().unwrap();
        let operation = operations.get(id)?.lock().unwrap();
        operation.untouched_retry()
    }
}

impl FileOperation {
    pub(super) fn untouched_retry(&self) -> Option<FileOperation> {
        if !FileOperationManager::is_final(&self.status) {
            return None;
        }
        let outcomes = self.snapshot_item_outcomes();
        let mut remaining = Vec::new();
        for source in &self.sources {
            match outcomes.get(source)? {
                ItemOutcome::Completed | ItemOutcome::Skipped => {}
                ItemOutcome::NotProcessed | ItemOutcome::Pending => remaining.push(source.clone()),
                ItemOutcome::InProgress
                | ItemOutcome::Partial
                | ItemOutcome::Failed
                | ItemOutcome::Interrupted
                | ItemOutcome::Unknown => return None,
            }
        }
        if remaining.is_empty() {
            return None;
        }
        // Decisions are keyed by source path: those of the sources done would be dead weight.
        let resolutions = self.resolutions.as_ref().map(|resolutions| {
            resolutions
                .iter()
                .filter(|(path, _)| remaining.iter().any(|source| Path::new(path).starts_with(source)))
                .map(|(path, action)| (path.clone(), *action))
                .collect()
        });
        let mut retry = FileOperation::new(self.op_type.clone(), remaining, self.requested_destination.clone(), resolutions);
        retry.compression_quality = self.compression_quality.clone();
        retry.archive_qualities = self.archive_qualities.clone();
        retry.archive_password = self.archive_password.clone();
        retry.copy_name_suffix = self.copy_name_suffix.clone();
        retry.set_turbo(self.turbo_flag.load(Ordering::Relaxed));
        Some(retry)
    }
}

#[cfg(test)]
#[path = "tests/retry.rs"]
mod tests;
