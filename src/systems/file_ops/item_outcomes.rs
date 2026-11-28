use super::{FileOpType, FileOperation, FileOperationManager, OpStatus};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ItemOutcome {
    Pending,
    InProgress,
    Completed,
    NotProcessed,
    Skipped,
    Partial,
    Failed,
    Interrupted,
    Unknown,
}

#[derive(Debug, Clone, Default)]
pub(super) struct ItemProgress {
    expected: usize,
    done: usize,
    planned: bool,
    started: bool,
    retained: bool,
    skipped: bool,
    failed: bool,
    destination: Option<PathBuf>,
    requires_removal: bool,
    removed: bool,
}

impl FileOperation {
    fn progress_for(&mut self, path: &Path) -> Option<&mut ItemProgress> {
        let root = path
            .ancestors()
            .find(|path| self.item_progress.contains_key(*path))?
            .to_path_buf();
        self.item_progress.get_mut(&root)
    }

    pub(super) fn item_plan(
        &mut self,
        path: &Path,
        destination: PathBuf,
        expected: usize,
        skipped: bool,
        requires_removal: bool,
    ) {
        if let Some(progress) = self.item_progress.get_mut(path) {
            progress.expected = expected;
            progress.planned = true;
            progress.skipped |= skipped;
            if let Some(previous) = progress.destination.replace(destination.clone()) {
                if self.item_destinations.get(&previous).is_some_and(|source| source == path) {
                    self.item_destinations.remove(&previous);
                }
            }
            self.item_destinations.insert(destination, path.to_path_buf());
            progress.requires_removal = requires_removal;
        }
    }

    pub(super) fn item_started(&mut self, path: &Path) {
        if let Some(progress) = self.progress_for(path) {
            progress.started = true;
        }
    }

    pub(super) fn item_done(&mut self, path: &Path) {
        if let Some(progress) = self.progress_for(path) {
            progress.started = true;
            progress.done += 1;
            progress.retained = true;
        }
    }

    pub(super) fn item_effect(&mut self, path: &Path) {
        if let Some(progress) = self.progress_for(path) {
            progress.retained = true;
        }
    }

    pub(super) fn item_complete(&mut self, path: &Path) {
        if let Some(progress) = self.item_progress.get_mut(path) {
            progress.expected = 1;
            progress.done = 1;
            progress.planned = true;
            progress.started = true;
            progress.removed = true;
            progress.retained = true;
        }
    }

    /// A selected item inside a selected folder goes with it: done when the folder is.
    #[cfg(any(target_os = "windows", test))]
    pub(super) fn item_covered(&mut self, path: &Path) {
        let covered = path.ancestors().skip(1).filter_map(|folder| self.item_progress.get(folder))
            .any(|folder| folder.removed && folder.done == folder.expected && !folder.failed);
        if covered {
            self.item_complete(path);
        }
    }

    pub(super) fn item_failed(&mut self, path: &Path) {
        if let Some(progress) = self.item_progress.get_mut(path) {
            progress.started = true;
            progress.failed = true;
        }
    }

    pub(super) fn item_skipped(&mut self, path: &Path) {
        if let Some(progress) = self.progress_for(path) {
            progress.skipped = true;
        }
    }

    pub(super) fn item_directory(&mut self, destination: &Path, done: bool) {
        // The deepest root holding the destination is its first ancestor that is one.
        let source = destination
            .ancestors()
            .find_map(|ancestor| self.item_destinations.get(ancestor));
        if let Some(progress) = source.and_then(|source| self.item_progress.get_mut(source)) {
            progress.started = true;
            if done {
                progress.done += 1;
            }
        }
    }

    pub(super) fn item_directory_removed(&mut self, path: &Path) {
        if let Some(progress) = self.item_progress.get_mut(path) {
            progress.removed = true;
        }
    }

    pub(super) fn item_directory_created(&mut self, destination: &Path) {
        for ancestor in destination.ancestors() {
            if let Some(progress) = self
                .item_destinations
                .get(ancestor)
                .and_then(|source| self.item_progress.get_mut(source))
            {
                progress.retained = true;
            }
        }
    }

    /// Where each source was planned to go, under the name a conflict gave it
    /// (« keep both » renames it); sources not planned yet are absent.
    pub fn item_destinations(&self) -> HashMap<PathBuf, PathBuf> {
        self.item_progress
            .iter()
            .filter_map(|(source, progress)| Some((source.clone(), progress.destination.clone()?)))
            .collect()
    }

    pub(super) fn snapshot_item_outcomes(&self) -> HashMap<PathBuf, ItemOutcome> {
        self.sources.iter().cloned().zip(self.item_states()).collect()
    }

    /// The outcome of each source, in the order of `sources`, without cloning paths.
    pub(super) fn item_states(&self) -> Vec<ItemOutcome> {
        let terminal = FileOperationManager::is_final(&self.status);
        let archive = self.item_tracking_atomic;
        self.sources
            .iter()
            .map(|path| {
                if let Some(saved) = self.item_outcomes.get(path) {
                    *saved
                } else if let Some(progress) = self.item_progress.get(path) {
                    if archive {
                        if matches!(self.op_type, FileOpType::Compress | FileOpType::Extract)
                            && (self.archive_publication_finished
                                || self.status == OpStatus::Completed)
                        {
                            ItemOutcome::Completed
                        } else if terminal && !self.execution_started {
                            ItemOutcome::NotProcessed
                        } else if terminal && !progress.started {
                            // Its work stayed in private folders, which went with the failure.
                            ItemOutcome::NotProcessed
                        } else if terminal {
                            ItemOutcome::Unknown
                        } else if self.execution_started {
                            ItemOutcome::InProgress
                        } else {
                            ItemOutcome::Pending
                        }
                    } else if progress.planned
                        && progress.expected > 0
                        && progress.done == progress.expected
                        && !progress.skipped
                        && (!progress.requires_removal || progress.removed)
                    {
                        ItemOutcome::Completed
                    } else if terminal {
                        if progress.retained {
                            ItemOutcome::Partial
                        } else if progress.failed {
                            ItemOutcome::Failed
                        } else if progress.started {
                            ItemOutcome::Interrupted
                        } else if progress.skipped {
                            ItemOutcome::Skipped
                        } else {
                            ItemOutcome::NotProcessed
                        }
                    } else if progress.started {
                        ItemOutcome::InProgress
                    } else if progress.skipped {
                        ItemOutcome::Skipped
                    } else {
                        ItemOutcome::Pending
                    }
                } else {
                    ItemOutcome::Unknown
                }
            })
            .collect()
    }
}

/// The outcome of each item of an operation, as the operations window lists them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemStates {
    /// One per source, in the order of the operation's sources.
    pub outcomes: Vec<ItemOutcome>,
    /// The operation had ended when they were read: they no longer change.
    pub settled: bool,
}

/// What an operation works on, read once by the operations window.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OperationItems {
    pub sources: Vec<PathBuf>,
    /// Where each source was planned to go, as `FileOperation::item_destinations`.
    pub destinations: HashMap<PathBuf, PathBuf>,
}

impl FileOperationManager {
    pub fn operation_item_outcomes(&self, id: &str) -> Option<HashMap<PathBuf, ItemOutcome>> {
        let operations = self.operations.lock().unwrap();
        operations
            .get(id)
            .map(|operation| operation.lock().unwrap().snapshot_item_outcomes())
    }

    /// The operation is taken out of the list first: other operations stay
    /// available while its items are walked.
    fn operation_handle(&self, id: &str) -> Option<std::sync::Arc<std::sync::Mutex<FileOperation>>> {
        self.operations.lock().unwrap().get(id).cloned()
    }

    /// The outcome of each source by position, without cloning their paths.
    pub fn operation_item_states(&self, id: &str) -> Option<ItemStates> {
        let operation = self.operation_handle(id)?;
        let operation = operation.lock().unwrap();
        Some(ItemStates {
            outcomes: operation.item_states(),
            settled: FileOperationManager::is_final(&operation.status),
        })
    }

    pub fn operation_items(&self, id: &str) -> Option<OperationItems> {
        let operation = self.operation_handle(id)?;
        let operation = operation.lock().unwrap();
        Some(OperationItems {
            sources: operation.sources.clone(),
            destinations: operation.item_destinations(),
        })
    }
}

#[cfg(test)]
#[path = "tests/item_outcomes.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/item_cancellation.rs"]
mod cancellation_tests;

#[cfg(test)]
pub(super) fn copy_progress_test_hook(id: &str, path: &Path, bytes: u64) {
    cancellation_tests::pause_copy(id, path, bytes);
}
