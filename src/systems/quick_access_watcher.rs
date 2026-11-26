use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use crate::utils::change_signal::ChangeSignal;
use std::sync::Arc;

/// Watching the parent survives creation/replacement of AutomaticDestinations files.
pub fn watch(path: &Path, revision: Arc<ChangeSignal>) -> Result<RecommendedWatcher, String> {
    let mut watcher = RecommendedWatcher::new(
        move |result: Result<Event, notify::Error>| match result {
            Ok(event) if !matches!(event.kind, EventKind::Access(_)) => {
                revision.notify();
            }
            Err(error) => {
                log::warn!("Quick access watcher: {error}");
                revision.notify();
            }
            _ => {}
        },
        Config::default(),
    )
    .map_err(|error| error.to_string())?;
    watcher
        .watch(path, RecursiveMode::Recursive)
        .map_err(|error| error.to_string())?;
    Ok(watcher)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn observes_external_changes_and_stops_when_dropped() {
        let root = tempfile::tempdir().unwrap();
        let children = root.path().join("AutomaticDestinations");
        let revision = Arc::new(ChangeSignal::new());
        let watcher = watch(root.path(), revision.clone()).unwrap();
        std::fs::create_dir(&children).unwrap();
        std::fs::write(children.join("fixture"), b"pinned").unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while revision.value() == 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(revision.value() > 0);
        drop(watcher);
        // Windows queues the shutdown request. Wait for the callback to release
        // its revision before checking that new changes are no longer observed.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while Arc::strong_count(&revision) > 1 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(
            Arc::strong_count(&revision),
            1,
            "watcher callback was not released"
        );
        let before = revision.value();
        std::fs::write(children.join("fixture"), b"unpinned").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(revision.value(), before);
    }
}
