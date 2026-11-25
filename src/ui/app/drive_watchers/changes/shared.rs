//! One inotify instance for every plain folder watch, as GIO keeps one per process, and
//! one polling thread per polled volume (NFS, FUSE) for its plain folders.
//!
//! A watcher per folder took one of the user's inotify instances (128 by default, shared
//! with every program of the session) and a thread each: « Expand all » on `/` used them
//! all up, and other programs could no longer watch anything while Oxyde held them.
//!
//! A share whose server stops answering holds its polling thread inside a scan, and
//! notify's lock with it: its folders are released on a thread of their own, so neither
//! the interface nor its closing waits for the server, and other volumes keep polling.

use super::{DirectoryChanges, Observed, WatcherChanges};
use notify::{Config, Event, EventKind, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher};
use oxyde_lib::utils::path_text::decode;
use std::collections::HashMap;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex, OnceLock, PoisonError};

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

/// How the shared instance learns of changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Inotify,
    /// Every interval, for volumes whose changes the kernel does not report.
    Poll { interval: std::time::Duration, volume: String },
}

struct Shared {
    watcher: Mutex<Option<Box<dyn Watcher + Send>>>,
    registry: Arc<Mutex<Registry>>,
    /// Folders to unwatch, for a thread that may wait on an unanswering server.
    releases: Mutex<Option<mpsc::Sender<PathBuf>>>,
}

impl Shared {
    fn new() -> Arc<Self> {
        Arc::new(Self { watcher: Mutex::new(None), registry: Default::default(), releases: Mutex::new(None) })
    }
}

fn shared(kind: &Kind) -> Arc<Shared> {
    static INOTIFY: OnceLock<Arc<Shared>> = OnceLock::new();
    static POLLED: OnceLock<Mutex<HashMap<String, Arc<Shared>>>> = OnceLock::new();
    match kind {
        Kind::Inotify => INOTIFY.get_or_init(Shared::new).clone(),
        Kind::Poll { volume, .. } => {
            let mut volumes = POLLED.get_or_init(Default::default).lock().unwrap_or_else(PoisonError::into_inner);
            volumes.entry(volume.clone()).or_insert_with(Shared::new).clone()
        }
    }
}

/// A folder watched through a shared instance, until released or dropped.
pub(super) struct Registration {
    polled: bool,
    shared: Arc<Shared>,
    path: PathBuf,
    generation: u64,
}

/// `None` when the same folder is already watched under another path by inotify: the
/// caller then keeps a watcher of its own. Polling compares each path's own listing.
pub(super) fn watch(path: &str, kind: Kind, changes: WatcherChanges) -> notify::Result<Option<Registration>> {
    let decoded = decode(path);
    let metadata = std::fs::metadata(&decoded).map_err(|error| notify::Error::io(error).add_path(decoded.clone()))?;
    let identity = (metadata.dev(), metadata.ino());
    let shared = shared(&kind);
    let mut watcher = shared.watcher.lock().map_err(|_| notify::Error::generic("watcher lock poisoned"))?;
    if watcher.is_none() {
        *watcher = Some(create(&kind, shared.registry.clone())?);
    }
    let generation = {
        let mut registry = shared.registry.lock().map_err(|_| notify::Error::generic("watcher lock poisoned"))?;
        if kind == Kind::Inotify
            && registry.roots.iter().any(|(other, root)| *other != decoded && root.identity == identity)
        {
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
        forget(&shared, &decoded, generation);
        return Err(error);
    }
    drop(watcher);
    Ok(Some(Registration { polled: kind != Kind::Inotify, shared, path: decoded, generation }))
}

/// Removes the registration's root, unless the same folder was watched again since.
fn forget(shared: &Shared, path: &PathBuf, generation: u64) -> bool {
    let Ok(mut registry) = shared.registry.lock() else {
        return false;
    };
    if registry.roots.get(path).is_some_and(|root| root.generation == generation) {
        registry.roots.remove(path);
        true
    } else {
        false
    }
}

/// The folder stops reporting at once; a polled one is unwatched on its volume's thread.
pub(super) fn release(registration: Registration) -> Result<(), String> {
    let Registration { polled, shared, path, generation } = registration;
    if !forget(&shared, &path, generation) {
        return Ok(());
    }
    if !polled {
        return unwatch(&shared, &path);
    }
    let mut releases = shared.releases.lock().map_err(|_| "watcher lock poisoned".to_string())?;
    if releases.is_none() {
        let (sender, receiver) = mpsc::channel::<PathBuf>();
        let target = shared.clone();
        std::thread::Builder::new()
            .name("oxyde-poll-release".into())
            .spawn(move || {
                for path in receiver {
                    if let Err(error) = unwatch(&target, &path) {
                        log::warn!("Cannot stop polling {}: {error}", path.display());
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        *releases = Some(sender);
    }
    releases.as_ref().map_or(Ok(()), |sender| sender.send(path).map_err(|error| error.to_string()))
}

/// Unless the folder was watched again meanwhile, which `watch` does under the same lock.
fn unwatch(shared: &Shared, path: &PathBuf) -> Result<(), String> {
    let Ok(mut watcher) = shared.watcher.lock() else {
        return Err("watcher lock poisoned".into());
    };
    if shared.registry.lock().map_err(|_| "watcher lock poisoned".to_string())?.roots.contains_key(path) {
        return Ok(());
    }
    let Some(watcher) = watcher.as_mut() else {
        return Ok(());
    };
    watcher.unwatch(path).map_err(|error| error.to_string())
}

fn create(kind: &Kind, registry: Arc<Mutex<Registry>>) -> notify::Result<Box<dyn Watcher + Send>> {
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
    let config = Config::default().with_follow_symlinks(false);
    Ok(match kind {
        Kind::Inotify => Box::new(RecommendedWatcher::new(callback, config)?),
        // Snapshots, not open handles: a polled volume can still be unmounted.
        Kind::Poll { interval, .. } => Box::new(PollWatcher::new(callback, config.with_poll_interval(*interval))?),
    })
}

#[cfg(test)]
#[path = "tests/shared.rs"]
mod tests;
