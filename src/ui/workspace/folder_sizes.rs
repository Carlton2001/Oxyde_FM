//! Per-location work queue. Keep completed measurements of busy folders visible
//! while coalescing their refreshes behind folders without a first measurement.
use super::TabState;
use oxyde_lib::models::{FileEntry, FolderSizeResult};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(crate) const SIZE_FLASH_DURATION: Duration = Duration::from_millis(300);
const RESCAN_INTERVAL: Duration = Duration::from_secs(5);

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
    pub(crate) publication: Option<Arc<gpui::Task<()>>>,
    revision: u64,
    // Render/layout snapshots clone TabState on the UI thread. Keep their
    // copies cheap, just like the file list, even with many queued folders.
    queue: Arc<VecDeque<String>>,
    pending: Arc<HashMap<String, Pending>>,
    running: Arc<HashMap<String, Running>>,
    ready: Arc<HashMap<String, (u64, FolderSizeResult)>>,
    last_completed: Arc<HashMap<String, Instant>>,
    provisional: Arc<HashSet<String>>,
    flashes: Arc<HashMap<String, Instant>>,
}

#[derive(Clone)]
struct Running {
    revision: u64,
    removed: bool,
}

#[derive(Clone)]
struct Pending {
    revision: u64,
    not_before: Instant,
    forced: bool,
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
        self.enqueue_at(path, Instant::now());
    }

    fn enqueue_at(&mut self, path: String, now: Instant) {
        self.revision = self.revision.wrapping_add(1);
        // Watcher activity dirties the measurement, not its publication identity.
        // In particular, a first completed traversal must always be publishable.
        let measured = self.running.contains_key(&path)
            || self.ready.contains_key(&path)
            || self.last_completed.contains_key(&path);
        if measured {
            Arc::make_mut(&mut self.provisional).insert(path.clone());
        }
        let minimum = self
            .last_completed
            .get(&path)
            .map_or(now, |completed| *completed + RESCAN_INTERVAL);
        // Repeated events defer automatic refreshes until the folder is quiet.
        // A manual request remains eligible despite subsequent watcher events.
        let forced = self.pending.get(&path).filter(|job| job.forced);
        let not_before = forced.map_or_else(
            || minimum.max(if measured { now + RESCAN_INTERVAL } else { now }),
            |job| job.not_before,
        );
        let forced = forced.is_some();
        if Arc::make_mut(&mut self.pending)
            .insert(
                path.clone(),
                Pending {
                    revision: self.revision,
                    not_before,
                    forced,
                },
            )
            .is_none()
        {
            Arc::make_mut(&mut self.queue).push_back(path);
        }
    }

    fn force_at(&mut self, path: String, now: Instant) {
        self.enqueue_at(path.clone(), now);
        let job = Arc::make_mut(&mut self.pending).get_mut(&path).unwrap();
        job.not_before = now;
        job.forced = true;
    }

    #[cfg(test)]
    fn take_batch(&mut self) -> Vec<(String, u64)> {
        self.take_available(2)
    }

    pub(crate) fn take_available(&mut self, capacity: usize) -> Vec<(String, u64)> {
        self.take_available_at(capacity, Instant::now())
    }

    fn take_available_at(&mut self, capacity: usize, now: Instant) -> Vec<(String, u64)> {
        let capacity = capacity.min(2usize.saturating_sub(self.running.len()));
        let mut batch = Vec::new();
        // Never spend an available slot on a repeat scan while a first scan is
        // eligible. FIFO order is preserved within each class.
        for first_only in [true, false] {
            let mut remaining = self.queue.len();
            while batch.len() < capacity && remaining > 0 {
                remaining -= 1;
                let Some(path) = Arc::make_mut(&mut self.queue).pop_front() else {
                    break;
                };
                if self.running.contains_key(&path)
                    || self.ready.contains_key(&path)
                    || (first_only && self.last_completed.contains_key(&path))
                    || self
                        .pending
                        .get(&path)
                        .is_some_and(|job| job.not_before > now)
                {
                    Arc::make_mut(&mut self.queue).push_back(path);
                    continue;
                }
                let Some(job) = Arc::make_mut(&mut self.pending).remove(&path) else {
                    continue;
                };
                Arc::make_mut(&mut self.running).insert(
                    path.clone(),
                    Running {
                        revision: job.revision,
                        removed: false,
                    },
                );
                batch.push((path, job.revision));
            }
        }
        batch
    }

    pub(crate) fn current(&self, generation: u64, path: &str, revision: u64) -> bool {
        self.generation == generation
            && self
                .running
                .get(path)
                .is_some_and(|job| job.revision == revision && !job.removed)
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

    /// Release the physical slot before any list preparation or publication.
    pub(crate) fn complete(&mut self, path: String, revision: u64, size: FolderSizeResult) {
        self.complete_at(path, revision, size, Instant::now());
    }

    fn complete_at(&mut self, path: String, revision: u64, size: FolderSizeResult, now: Instant) {
        if self.current(self.generation, &path, revision) {
            Arc::make_mut(&mut self.ready).insert(path.clone(), (revision, size));
            Arc::make_mut(&mut self.last_completed).insert(path.clone(), now);
            if let Some(job) = Arc::make_mut(&mut self.pending).get_mut(&path) {
                if !job.forced {
                    job.not_before = job.not_before.max(now + RESCAN_INTERVAL);
                }
            }
        }
        self.finish(&path, revision);
    }

    pub(crate) fn has_ready(&self) -> bool {
        !self.ready.is_empty()
    }

    pub(crate) fn provisional_paths(&self) -> Arc<HashSet<String>> {
        self.provisional.clone()
    }

    pub(crate) fn ready_batch(&self) -> Vec<(String, u64, FolderSizeResult)> {
        self.ready
            .iter()
            .take(128)
            .map(|(path, (revision, size))| (path.clone(), *revision, size.clone()))
            .collect()
    }

    pub(crate) fn publication_current(&self, generation: u64, path: &str, revision: u64) -> bool {
        self.generation == generation
            && self
                .ready
                .get(path)
                .is_some_and(|(current, _)| *current == revision)
    }

    pub(crate) fn published(&mut self, paths: &[(String, u64)]) {
        let ready = Arc::make_mut(&mut self.ready);
        for (path, revision) in paths {
            if ready
                .get(path)
                .is_some_and(|(current, _)| current == revision)
            {
                ready.remove(path);
                if !self.pending.contains_key(path) && !self.running.contains_key(path) {
                    Arc::make_mut(&mut self.provisional).remove(path);
                }
            }
        }
    }
}

impl TabState {
    pub(crate) fn refresh_folder_sizes(&mut self) {
        if !self.show_size_histogram {
            return;
        }
        let now = Instant::now();
        for file in self.files.iter().filter(|file| measurable(file)) {
            let active = self
                .folder_sizes
                .running
                .get(&file.path)
                .is_some_and(|job| !job.removed)
                || self.folder_sizes.ready.contains_key(&file.path);
            if self.folder_sizes.pending.contains_key(&file.path)
                || (!active
                    && (!file.is_calculated || self.folder_sizes.provisional.contains(&file.path)))
            {
                self.folder_sizes.force_at(file.path.clone(), now);
            }
        }
    }

    /// A replacement watcher may leave a gap for measurements already started.
    /// Untouched initial requests will be read after installation and need no
    /// synthetic invalidation or second pass.
    pub(crate) fn reconcile_folder_size_watcher(&mut self) {
        if !self.show_size_histogram {
            return;
        }
        for file in self.files.iter().filter(|file| measurable(file)) {
            if file.is_calculated
                || self.folder_sizes.running.contains_key(&file.path)
                || self.folder_sizes.ready.contains_key(&file.path)
                || self.folder_sizes.last_completed.contains_key(&file.path)
            {
                self.folder_sizes.enqueue(file.path.clone());
            }
        }
    }

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
        Arc::make_mut(&mut self.folder_sizes.ready).retain(|path, _| eligible.contains(path));
        Arc::make_mut(&mut self.folder_sizes.last_completed)
            .retain(|path, _| eligible.contains(path));
        Arc::make_mut(&mut self.folder_sizes.provisional).retain(|path| eligible.contains(path));
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
                && !self.folder_sizes.ready.contains_key(&file.path)
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

#[cfg(test)]
#[path = "folder_sizes/tests/live.rs"]
mod live_tests;
