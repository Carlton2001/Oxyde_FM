//! One inotify instance for every plain folder watch, as GIO keeps one per process.
//!
//! A watcher per folder took one of the user's inotify instances (128 by default, shared
//! with every program of the session) and a thread each: « Expand all » on `/` used them
//! all up, and other programs could no longer watch anything while Oxyde held them.

use super::{DirectoryChanges, Observed, WatcherChanges};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use oxyde_lib::utils::path_text::decode;
use std::collections::HashMap;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

struct Root {
    encoded: String,
    changes: WatcherChanges,
    /// Device and inode: inotify gives one descriptor per inode, so two paths of the same
    /// folder (a link, a bind mount) would share, and lose, a single watch.
    identity: (u64, u64),
    generation: u64,
}

#[derive(Default)]
struct Registry {
    roots: HashMap<PathBuf, Root>,
    next: u64,
}

struct Shared {
    watcher: Mutex<Option<RecommendedWatcher>>,
    registry: Arc<Mutex<Registry>>,
}

fn shared() -> &'static Shared {
    static SHARED: OnceLock<Shared> = OnceLock::new();
    SHARED.get_or_init(|| Shared { watcher: Mutex::new(None), registry: Default::default() })
}

/// A folder watched through the shared instance, until released or dropped.
pub(super) struct Registration {
    path: PathBuf,
    generation: u64,
}

/// `None` when the same folder is already watched under another path: the caller then
/// keeps a watcher of its own.
pub(super) fn watch(path: &str, changes: WatcherChanges) -> notify::Result<Option<Registration>> {
    let decoded = decode(path);
    let metadata = std::fs::metadata(&decoded).map_err(|error| notify::Error::io(error).add_path(decoded.clone()))?;
    let identity = (metadata.dev(), metadata.ino());
    let shared = shared();
    let mut watcher = shared.watcher.lock().map_err(|_| notify::Error::generic("watcher lock poisoned"))?;
    if watcher.is_none() {
        *watcher = Some(create(shared.registry.clone())?);
    }
    let generation = {
        let mut registry = shared.registry.lock().map_err(|_| notify::Error::generic("watcher lock poisoned"))?;
        if registry.roots.iter().any(|(other, root)| *other != decoded && root.identity == identity) {
            return Ok(None);
        }
        registry.next += 1;
        let generation = registry.next;
        let encoded = path.to_owned();
        registry.roots.insert(decoded.clone(), Root { encoded, changes, identity, generation });
        generation
    };
    // The registry is released first: the watcher's thread routes events through it.
    if let Some(Err(error)) = watcher.as_mut().map(|watcher| watcher.watch(&decoded, RecursiveMode::NonRecursive)) {
        forget(&decoded, generation);
        return Err(error);
    }
    Ok(Some(Registration { path: decoded, generation }))
}

/// Removes the registration's root, unless the same folder was watched again since.
fn forget(path: &PathBuf, generation: u64) -> bool {
    let Ok(mut registry) = shared().registry.lock() else {
        return false;
    };
    if registry.roots.get(path).is_some_and(|root| root.generation == generation) {
        registry.roots.remove(path);
        true
    } else {
        false
    }
}

pub(super) fn release(registration: Registration) -> Result<(), String> {
    let shared = shared();
    let Ok(mut watcher) = shared.watcher.lock() else {
        return Err("watcher lock poisoned".into());
    };
    if !forget(&registration.path, registration.generation) {
        return Ok(());
    }
    let Some(watcher) = watcher.as_mut() else {
        return Ok(());
    };
    watcher.unwatch(&registration.path).map_err(|error| error.to_string())
}

fn create(registry: Arc<Mutex<Registry>>) -> notify::Result<RecommendedWatcher> {
    let callback = move |result: notify::Result<Event>| {
        if matches!(&result, Ok(event) if matches!(event.kind, EventKind::Access(_))) {
            return;
        }
        // Routed under the lock, worked out after it: watching another folder waits on it.
        let targets: Vec<(String, WatcherChanges)> = {
            let Ok(registry) = registry.lock() else {
                return;
            };
            let paths: &[PathBuf] = match &result {
                Ok(event) if !event.need_rescan() => &event.paths,
                Err(error) => &error.paths,
                Ok(_) => &[],
            };
            let mut roots: Vec<&Root> = paths
                .iter()
                .flat_map(|path| [Some(path.as_path()), path.parent()])
                .flatten()
                .filter_map(|candidate| registry.roots.get(candidate))
                .collect();
            // An overflow, or an error naming no watched folder, may hide any change.
            if roots.is_empty() && !matches!(&result, Ok(event) if !event.need_rescan()) {
                roots = registry.roots.values().collect();
            }
            roots.sort_by(|a, b| a.encoded.cmp(&b.encoded));
            roots.dedup_by(|a, b| a.encoded == b.encoded);
            roots.into_iter().map(|root| (root.encoded.clone(), root.changes.clone())).collect()
        };
        if let Err(error) = &result {
            log::warn!("Directory watcher: {error}");
        }
        for (root, changes) in targets {
            let observed = match &result {
                Ok(event) => Some(Observed::of(&root, event)),
                Err(_) => None,
            };
            if let Ok(mut queue) = changes.lock() {
                let change: &mut DirectoryChanges = queue.entry(root).or_default();
                match observed {
                    Some(observed) => change.apply(observed),
                    None => change.rescan(),
                }
            }
        }
    };
    RecommendedWatcher::new(callback, Config::default().with_follow_symlinks(false))
}

#[cfg(test)]
#[path = "tests/shared.rs"]
mod tests;
