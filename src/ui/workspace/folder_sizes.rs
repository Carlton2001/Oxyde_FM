//! Per-location, bounded work queue. Changes during a scan supersede its result
//! and enqueue one follow-up at the tail, so busy folders cannot starve others.
use super::TabState;
use oxyde_lib::models::FileEntry;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(crate) const SIZE_FLASH_DURATION: Duration = Duration::from_millis(150);

pub(crate) fn measurable(file: &FileEntry) -> bool {
    file.is_dir && !file.is_symlink && !file.is_junction
}

fn path_key(path: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        path.replace('/', "\\")
            .trim_end_matches('\\')
            .to_lowercase()
    }
    #[cfg(not(target_os = "windows"))]
    {
        path.to_owned()
    }
}

#[derive(Clone, Default)]
pub(crate) struct FolderSizes {
    pub(crate) generation: u64,
    pub(crate) cancellation: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    revision: u64,
    // Render/layout snapshots clone TabState on the UI thread. Keep their
    // copies cheap, just like the file list, even with many queued folders.
    queue: Arc<VecDeque<String>>,
    pending: Arc<HashMap<String, u64>>,
    running: Arc<HashMap<String, Running>>,
    flashes: Arc<HashMap<String, Instant>>,
}

#[derive(Clone)]
struct Running {
    revision: u64,
    removed: bool,
}

impl FolderSizes {
    pub(crate) fn flash_updated_size(&mut self, path: String) {
        let now = Instant::now();
        let flashes = Arc::make_mut(&mut self.flashes);
        flashes.retain(|_, started| now.duration_since(*started) < SIZE_FLASH_DURATION);
        flashes.insert(path, now);
    }

    pub(crate) fn size_flashes(&self) -> Arc<HashMap<String, Instant>> {
        self.flashes.clone()
    }

    pub(crate) fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    fn enqueue(&mut self, path: String) {
        self.revision = self.revision.wrapping_add(1);
        if Arc::make_mut(&mut self.pending)
            .insert(path.clone(), self.revision)
            .is_none()
        {
            Arc::make_mut(&mut self.queue).push_back(path);
        }
    }

    #[cfg(test)]
    fn take_batch(&mut self) -> Vec<(String, u64)> {
        self.take_available(2)
    }

    pub(crate) fn take_available(&mut self, capacity: usize) -> Vec<(String, u64)> {
        let capacity = capacity.min(2usize.saturating_sub(self.running.len()));
        let mut batch = Vec::new();
        let mut remaining = self.queue.len();
        while batch.len() < capacity && remaining > 0 {
            remaining -= 1;
            let Some(path) = Arc::make_mut(&mut self.queue).pop_front() else {
                break;
            };
            if self.running.contains_key(&path) {
                Arc::make_mut(&mut self.queue).push_back(path);
                continue;
            }
            let Some(revision) = Arc::make_mut(&mut self.pending).remove(&path) else {
                continue;
            };
            Arc::make_mut(&mut self.running).insert(
                path.clone(),
                Running {
                    revision,
                    removed: false,
                },
            );
            batch.push((path, revision));
        }
        batch
    }

    pub(crate) fn current(&self, generation: u64, path: &str, revision: u64) -> bool {
        self.generation == generation
            && self
                .running
                .get(path)
                .is_some_and(|job| job.revision == revision && !job.removed)
            && !self.pending.contains_key(path)
    }

    pub(crate) fn finish(&mut self, path: &str, revision: u64) {
        if self
            .running
            .get(path)
            .is_some_and(|job| job.revision == revision)
        {
            Arc::make_mut(&mut self.running).remove(path);
        }
    }
}

impl TabState {
    pub(crate) fn reset_folder_sizes(&mut self) {
        if let Some(cancellation) = &self.folder_sizes.cancellation {
            cancellation.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.size_calculation = None;
        self.show_size_histogram = false;
        self.folder_sizes = FolderSizes {
            generation: self.folder_sizes.generation.wrapping_add(1),
            ..Default::default()
        };
    }

    /// None invalidates all directories; Some targets only immediate children.
    pub(crate) fn invalidate_folder_sizes(
        &mut self,
        paths: Option<&std::collections::HashSet<String>>,
    ) {
        if !self.show_size_histogram {
            return;
        }
        let paths = paths.map(|paths| {
            paths
                .iter()
                .map(|path| path_key(path))
                .collect::<std::collections::HashSet<_>>()
        });
        for file in self.files.iter().filter(|file| measurable(file)) {
            if paths
                .as_ref()
                .is_none_or(|paths| paths.contains(&path_key(&file.path)))
            {
                self.folder_sizes.enqueue(file.path.clone());
            }
        }
    }

    pub(crate) fn queue_missing_folder_sizes(&mut self) {
        if !self.show_size_histogram {
            return;
        }
        let eligible: std::collections::HashSet<_> = self
            .files
            .iter()
            .filter(|file| measurable(file))
            .map(|file| file.path.clone())
            .collect();
        Arc::make_mut(&mut self.folder_sizes.pending).retain(|path, _| eligible.contains(path));
        Arc::make_mut(&mut self.folder_sizes.queue).retain(|path| eligible.contains(path));
        // Keep a removed scan's slot occupied until its physical request ends.
        // A recreated path queues a new scan but cannot overlap the old one.
        for (path, job) in Arc::make_mut(&mut self.folder_sizes.running) {
            if !eligible.contains(path) {
                job.removed = true;
            }
        }
        for file in self
            .files
            .iter()
            .filter(|file| measurable(file) && !file.is_calculated)
        {
            if !self.folder_sizes.pending.contains_key(&file.path)
                && self
                    .folder_sizes
                    .running
                    .get(&file.path)
                    .is_none_or(|job| job.removed)
            {
                self.folder_sizes.enqueue(file.path.clone());
            }
        }
    }
}

#[cfg(test)]
#[path = "folder_sizes/tests/queue.rs"]
mod tests;
