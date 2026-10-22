use crate::ui::components::sidebar::sidebar_paths_equal;
use notify::{Config, Event, EventKind, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Arc, Mutex},
};

pub type WatcherChanges = Arc<Mutex<HashMap<String, DirectoryChanges>>>;

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
}

impl DirectoryWatcher {
    pub(super) fn create(
        path: &str,
        polling: bool,
        recursive: bool,
        changes: WatcherChanges,
    ) -> notify::Result<Self> {
        let watched_path = path.to_owned();
        let callback = move |result: notify::Result<Event>| {
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
                config.with_poll_interval(std::time::Duration::from_secs(2)),
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
        })
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
