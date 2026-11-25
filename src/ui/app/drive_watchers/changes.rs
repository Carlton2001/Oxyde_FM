use crate::ui::paths::paths_match;
use oxyde_lib::utils::path_text::{decode, encode};
use notify::{Config, Event, EventKind, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

/// Overflow reported this soon after installation stems from the installation itself.
const INSTALLATION_SETTLE: std::time::Duration = std::time::Duration::from_secs(1);

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

    #[cfg(test)]
    fn record(&mut self, root: &str, event: &Event) {
        self.apply(Observed::of(root, event));
    }

    fn apply(&mut self, observed: Observed) {
        if observed.rescan {
            self.rescan();
            return;
        }
        self.listing |= observed.listing;
        for child in observed.children {
            if self.all_sizes {
                break;
            }
            self.children.insert(child);
            if self.children.len() > 256 {
                self.all_sizes = true;
                self.children.clear();
            }
        }
    }
}

/// What one event changes under its root, worked out before the shared queue
/// is locked: other watchers' callbacks do not wait on these comparisons.
struct Observed {
    rescan: bool,
    listing: bool,
    children: Vec<String>,
}

impl Observed {
    fn of(root: &str, event: &Event) -> Self {
        let mut observed = Self { rescan: false, listing: false, children: Vec::new() };
        if event.need_rescan() || event.paths.is_empty() {
            observed.rescan = true;
            return observed;
        }
        for path in &event.paths {
            if paths_match(&encode(path), root) {
                observed.rescan = true;
                return observed;
            }
            // Compare whole ancestors, including Windows case/separator aliases.
            // This requires no metadata and also handles deleted/renamed paths.
            let child = path.ancestors().find(|ancestor| {
                ancestor
                    .parent()
                    .is_some_and(|parent| paths_match(&encode(parent), root))
            });
            let Some(child) = child else {
                observed.rescan = true;
                return observed;
            };
            if child == path {
                observed.listing = true;
            }
            observed.children.push(encode(child));
        }
        observed
    }
}

#[cfg(target_os = "linux")]
mod shared;

enum Backend {
    Own(Box<dyn Watcher + Send>),
    /// Emptied once released.
    #[cfg(target_os = "linux")]
    Shared(Option<shared::Registration>),
}

pub struct DirectoryWatcher {
    pub(super) polling: bool,
    pub(super) recursive: bool,
    backend: Backend,
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
        #[cfg(target_os = "linux")]
        if !recursive {
            let kind = if polling { shared::Kind::Poll(interval) } else { shared::Kind::Inotify };
            if let Some(registration) = shared::watch(path, kind, changes.clone())? {
                return Ok(Self {
                    polling,
                    recursive,
                    backend: Backend::Shared(Some(registration)),
                    failed: Arc::new(AtomicBool::new(false)),
                });
            }
        }
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
        let installed = Arc::new(std::sync::OnceLock::new());
        let callback_installed = installed.clone();
        let callback = move |result: notify::Result<Event>| {
            if result.is_err() {
                callback_failed.store(true, Ordering::Relaxed);
            }
            // Reading directories for a size scan must not trigger another scan.
            if matches!(&result, Ok(event) if matches!(event.kind, EventKind::Access(_))) {
                return;
            }
            // Installing a recursive inotify watch reads every subfolder; on a large
            // tree those reads overflow the kernel queue before the installation
            // returns. The installation's own gap check already covers that time.
            if matches!(&result, Ok(event) if event.need_rescan())
                && callback_installed.get().is_none_or(|installed: &std::time::Instant| installed.elapsed() < INSTALLATION_SETTLE)
            {
                return;
            }
            let observed = match result {
                Ok(event) => Some(Observed::of(&watched_path, &event)),
                Err(error) => {
                    log::warn!("Directory watcher for {watched_path}: {error}");
                    None
                }
            };
            if let Ok(mut queue) = changes.lock() {
                let change = queue.entry(watched_path.clone()).or_default();
                match observed {
                    Some(observed) => change.apply(observed),
                    None => change.rescan(),
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
            &decode(path),
            if recursive {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            },
        )?;
        let _ = installed.set(std::time::Instant::now());
        Ok(Self {
            polling,
            recursive,
            backend: Backend::Own(watcher),
            failed,
        })
    }

    pub(in crate::ui::app) fn reliable_size_events(&self) -> bool {
        self.recursive && !self.polling && !self.failed.load(Ordering::Relaxed)
    }

    pub(in crate::ui::app) fn release(mut self, path: &str) -> Result<(), String> {
        let watcher = match &mut self.backend {
            Backend::Own(watcher) => watcher,
            #[cfg(target_os = "linux")]
            Backend::Shared(registration) => return registration.take().map_or(Ok(()), shared::release),
        };
        // A failed unwatch must not skip the barrier below: the watcher is dropped anyway.
        let unwatched = watcher
            .unwatch(&decode(path))
            .map_err(|error| error.to_string());
        // The configure reply is a FIFO barrier for Windows' queued unwatch.
        let configured = watcher
            .configure(Config::default())
            .map(|_| ())
            .map_err(|error| error.to_string());
        unwatched.and(configured)
    }
}

#[cfg(target_os = "linux")]
impl Drop for DirectoryWatcher {
    fn drop(&mut self) {
        // An own watcher stops with its instance; a shared watch has to be removed.
        if let Backend::Shared(registration) = &mut self.backend {
            if let Some(registration) = registration.take() {
                let _ = shared::release(registration);
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/changes.rs"]
mod tests;
