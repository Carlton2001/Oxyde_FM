//! Bounded flat-file batches; directories keep their descendant-aware receipts.
use super::*;
use crate::platform::ShellDeleteControl;
use crate::systems::file_ops::FILE_OPS_LOG_TARGET;
use crate::systems::history_delete::PreparedDelete;

fn parse_batch_size(value: Option<&std::ffi::OsStr>) -> Option<usize> {
    match value {
        None => Some(256),
        Some(value) if value == "64" => Some(64),
        Some(value) if value == "256" => Some(256),
        _ => None,
    }
}

fn diagnostic_batch_size() -> usize {
    let value = std::env::var_os("OXYDE_TRASH_BATCH_SIZE");
    parse_batch_size(value.as_deref()).unwrap_or_else(|| {
        log::warn!(target: FILE_OPS_LOG_TARGET,
            "Invalid OXYDE_TRASH_BATCH_SIZE={value:?}; expected 64 or 256; using 256");
        256
    })
}

/// Journals every receipt and returns each refused root with its cause.
fn record_batch_receipts(
    selected: &[PreparedDelete],
    outcomes: Vec<crate::platform::ShellDeleteOutcome>,
    completed: &mut dyn FnMut(crate::models::transaction::HistoryDeletedItem),
) -> Vec<(std::path::PathBuf, String)> {
    let mut failures = Vec::new();
    let mut outcomes = outcomes.into_iter();
    for item in selected {
        let failure = match outcomes.next() {
            Some(outcome) => item.record_outcome(outcome, completed).err().map(|error| error.to_string()),
            None => Some("Missing batch deletion receipts".into()),
        };
        if let Some(failure) = failure {
            failures.push((item.path().to_path_buf(), failure));
        }
    }
    failures
}

impl FileOperationManager {
    pub(super) fn perform_selected_trash(
        op: &Arc<Mutex<FileOperation>>,
        selected: Vec<PreparedDelete>,
        control: ShellDeleteControl,
    ) -> Result<(), String> {
        let started = std::time::Instant::now();
        let batch_size = diagnostic_batch_size();
        log::info!(target: "oxyde_lib::delete_timing", "trash_batch_plan op={} selected={} batch_size={} turbo={}",
            op.lock().unwrap().id, selected.len(), batch_size, control.turbo.load(Ordering::Relaxed));
        let mut index = 0;
        let mut batches = 0;
        let mut failures = Vec::new();
        while index < selected.len() {
            if control.cancel.load(Ordering::Relaxed) {
                break;
            }
            while control.pause.load(Ordering::Relaxed) {
                if control.cancel.load(Ordering::Relaxed) {
                    return Ok(());
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            follow_turbo_priority(control.turbo.load(Ordering::Relaxed), ThreadPriority::Normal);
            let mut files = Vec::new();
            for item in selected[index..].iter().take(batch_size) {
                let Some(file) = item.batch_file() else {
                    break;
                };
                files.push(file);
            }
            let end = index + files.len().max(1);
            let roots: std::collections::HashSet<_> = selected[index..end]
                .iter()
                .map(|item| item.path().to_path_buf())
                .collect();
            {
                let mut locked = op.lock().unwrap();
                locked.current_file = Some(selected[index].path().to_string_lossy().into_owned());
                for path in &roots {
                    locked.item_started(path);
                }
            }
            let mut completed = |deleted: crate::models::transaction::HistoryDeletedItem| {
                let mut locked = op.lock().unwrap();
                let path = std::path::Path::new(&deleted.path);
                if roots.contains(path) {
                    locked.processed_files += 1;
                    locked.item_complete(path);
                }
                locked.deleted_items.push(deleted);
            };
            let batch_failures = if files.is_empty() {
                match selected[index].recycle_controlled(control.clone(), &mut completed) {
                    Ok(_) => Vec::new(),
                    Err(failure) => vec![(selected[index].path().to_path_buf(), failure.to_string())],
                }
            } else {
                let outcomes = platform().shell_delete_files_tracked(&files, control.clone());
                record_batch_receipts(&selected[index..end], outcomes, &mut completed)
            };
            batches += 1;
            index = end;
            // Every returned receipt is journaled before honoring cancellation.
            if control.cancel.load(Ordering::Relaxed) {
                break;
            }
            // A refused item must not prevent the rest of the selection from being deleted.
            let mut locked = op.lock().unwrap();
            for (path, failure) in batch_failures {
                log::warn!(target: FILE_OPS_LOG_TARGET, "trash_item_failed path={path:?} error={failure}");
                locked.item_failed(&path);
                locked.processed_files += 1;
                failures.push(failure);
            }
        }
        log::info!(target: FILE_OPS_LOG_TARGET,
            "trash_batches op={} selected={} batches={} batch_size={} cancelled={} wall_s={:.6}",
            op.lock().unwrap().id, selected.len(), batches, batch_size,
            control.cancel.load(Ordering::Relaxed), started.elapsed().as_secs_f64());
        if control.cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        crate::models::error::DeleteBatchFailure::summarize(selected.len(), failures)
    }
}

#[cfg(test)]
#[path = "trash_batch/tests/receipts.rs"]
mod tests;
