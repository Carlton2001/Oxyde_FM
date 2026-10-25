use crate::ui::components::sidebar::sidebar_paths_equal;
use notify::{Config, Event, EventKind, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

pub type WatcherChanges = Arc<Mutex<HashMap<String, DirectoryChanges>>>;

pub(in crate::ui::app) enum SizeChanges {
    None,
    All,
    Children(HashSet<String>),
}

/// Consume only size invalidations; listing work remains queued for every tab.
pub(in crate::ui::app) fn take_size_changes(changes: &WatcherChanges, root: &str) -> SizeChanges {
    let Ok(mut queue) = changes.lock() else {
        return SizeChanges::All;
    };
    let Some(change) = queue.get_mut(root) else {
        return SizeChanges::None;
    };
    let all = std::mem::take(&mut change.all_sizes);
    let children = std::mem::take(&mut change.children);
    if !change.listing {
        queue.remove(root);
    }
    if all {
        SizeChanges::All
    } else if children.is_empty() {
        SizeChanges::None
    } else {
        SizeChanges::Children(children)
    }
}

/// Bounded per-root accumulation, even if millions of descendants change.
#[derive(Default)]
pub struct DirectoryChanges {
    pub(super) listing: bool,
    pub(super) all_sizes: bool,
    pub(super) children: HashSet<String>,
}

impl DirectoryChanges {
    fn rescan(&mut self) {
        self.listing = true;
        self.all_sizes = true;
        self.children.clear();
    }

    fn record(&mut self, root: &str, event: &Event) {
        if event.need_rescan() || event.paths.is_empty() {
            self.rescan();
            return;
        }
        for path in &event.paths {
            if sidebar_paths_equal(&path.to_string_lossy(), root) {
                self.rescan();
                continue;
            }
            // Compare whole ancestors, including Windows case/separator aliases.
            // This requires no metadata and also handles deleted/renamed paths.
            let child = path.ancestors().find(|ancestor| {
                ancestor
                    .parent()
                    .is_some_and(|parent| sidebar_paths_equal(&parent.to_string_lossy(), root))
            });
            let Some(child) = child else {
                self.rescan();
                continue;
            };
            if child == path {
                self.listing = true;
            }
            if !self.all_sizes {
                self.children.insert(child.to_string_lossy().into_owned());
                if self.children.len() > 256 {
                    self.all_sizes = true;
                    self.children.clear();
                }
            }
        }
    }
}

pub struct DirectoryWatcher {
    pub(super) polling: bool,
    pub(super) recursive: bool,
    watcher: Box<dyn Watcher + Send>,
    failed: Arc<AtomicBool>,
}

impl DirectoryWatcher {
    pub(super) fn create(
        path: &str,
        polling: bool,
        recursive: bool,
        changes: WatcherChanges,
    ) -> notify::Result<Self> {
        // Recursive polling walks the whole subtree: do not repeat that every
        // two seconds on removable or unidentified storage.
        let interval = std::time::Duration::from_secs(if recursive { 30 } else { 2 });
        Self::with_poll_interval(path, polling, recursive, changes, interval)
    }

    fn with_poll_interval(
        path: &str,
        polling: bool,
        recursive: bool,
        changes: WatcherChanges,
        interval: std::time::Duration,
    ) -> notify::Result<Self> {
        let watched_path = path.to_owned();
        let failed = Arc::new(AtomicBool::new(false));
        let callback_failed = failed.clone();
        let callback = move |result: notify::Result<Event>| {
            if result.is_err() {
                callback_failed.store(true, Ordering::Relaxed);
            }
            // Reading directories for a size scan must not trigger another scan.
            if matches!(&result, Ok(event) if matches!(event.kind, EventKind::Access(_))) {
                return;
            }
            if let Ok(mut queue) = changes.lock() {
                let change = queue.entry(watched_path.clone()).or_default();
                match result {
                    Ok(event) => change.record(&watched_path, &event),
                    Err(error) => {
                        log::warn!("Directory watcher for {watched_path}: {error}");
                        change.rescan();
                    }
                }
            }
        };
        // USB polling retains snapshots, not open directory handles. Recursive
        // snapshots are enabled only for locations with live folder sizes.
        let config = Config::default().with_follow_symlinks(false);
        let mut watcher: Box<dyn Watcher + Send> = if polling {
            Box::new(PollWatcher::new(
                callback,
                config.with_poll_interval(interval),
            )?)
        } else {
            Box::new(RecommendedWatcher::new(callback, config)?)
        };
        watcher.watch(
            Path::new(path),
            if recursive {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            },
        )?;
        Ok(Self {
            polling,
            recursive,
            watcher,
            failed,
        })
    }

    pub(in crate::ui::app) fn reliable_size_events(&self) -> bool {
        self.recursive && !self.polling && !self.failed.load(Ordering::Relaxed)
    }

    pub(in crate::ui::app) fn release(mut self, path: &str) -> Result<(), String> {
        self.watcher
            .unwatch(Path::new(path))
            .map_err(|error| error.to_string())?;
        // The configure reply is a FIFO barrier for Windows' queued unwatch.
        self.watcher
            .configure(Config::default())
            .map_err(|error| error.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/changes.rs"]
mod tests;
