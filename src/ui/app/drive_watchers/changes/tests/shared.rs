use super::super::DirectoryWatcher;
use super::WatcherChanges;
use std::path::Path;
use std::time::{Duration, Instant};

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn reported(changes: &WatcherChanges, root: &str) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if changes.lock().unwrap().contains_key(root) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn more_folders_than_the_inotify_instance_limit_are_watched_and_routed() {
    let root = tempfile::tempdir().unwrap();
    let changes: WatcherChanges = Default::default();
    // The default limit is 128 instances per user, for every program of the session.
    let watchers: Vec<_> = (0..300)
        .map(|index| {
            let folder = root.path().join(index.to_string());
            std::fs::create_dir(&folder).unwrap();
            let path = text(&folder);
            let watcher = DirectoryWatcher::create(&path, false, false, changes.clone()).unwrap();
            (path, watcher)
        })
        .collect();
    std::fs::write(root.path().join("250").join("new"), b"x").unwrap();
    assert!(reported(&changes, &watchers[250].0));
    assert_eq!(changes.lock().unwrap().len(), 1);
    for (path, watcher) in watchers {
        watcher.release(&path).unwrap();
    }
}

#[test]
fn a_folder_watched_through_a_link_keeps_its_watch_when_the_other_path_is_released() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    std::fs::create_dir(&folder).unwrap();
    let link = root.path().join("link");
    std::os::unix::fs::symlink(&folder, &link).unwrap();
    let changes: WatcherChanges = Default::default();
    let direct = DirectoryWatcher::create(&text(&folder), false, false, changes.clone()).unwrap();
    let linked = DirectoryWatcher::create(&text(&link), false, false, changes.clone()).unwrap();
    direct.release(&text(&folder)).unwrap();
    std::fs::write(folder.join("new"), b"x").unwrap();
    assert!(reported(&changes, &text(&link)));
    linked.release(&text(&link)).unwrap();
}

#[test]
fn releasing_an_older_watch_of_a_folder_watched_again_keeps_the_newer_one() {
    let root = tempfile::tempdir().unwrap();
    let path = text(root.path());
    let changes: WatcherChanges = Default::default();
    let older = DirectoryWatcher::create(&path, false, false, changes.clone()).unwrap();
    let newer = DirectoryWatcher::create(&path, false, false, changes.clone()).unwrap();
    older.release(&path).unwrap();
    std::fs::write(root.path().join("new"), b"x").unwrap();
    assert!(reported(&changes, &path));
    drop(newer);
    changes.lock().unwrap().clear();
    std::fs::write(root.path().join("later"), b"x").unwrap();
    std::thread::sleep(Duration::from_millis(300));
    assert!(!changes.lock().unwrap().contains_key(&path));
}

#[test]
fn polled_folders_share_one_instance_and_each_change_reaches_its_folder() {
    let root = tempfile::tempdir().unwrap();
    let changes: WatcherChanges = Default::default();
    let watchers: Vec<_> = (0..40)
        .map(|index| {
            let folder = root.path().join(index.to_string());
            std::fs::create_dir(&folder).unwrap();
            let path = text(&folder);
            let watcher = DirectoryWatcher::create(&path, true, false, changes.clone()).unwrap();
            (path, watcher)
        })
        .collect();
    std::fs::write(root.path().join("30").join("new"), b"x").unwrap();
    assert!(reported(&changes, &watchers[30].0));
    assert_eq!(changes.lock().unwrap().len(), 1);
    // Released, a folder no longer reports.
    let (path, watcher) = watchers.into_iter().nth(30).unwrap();
    watcher.release(&path).unwrap();
    changes.lock().unwrap().clear();
    std::fs::write(root.path().join("30").join("other"), b"x").unwrap();
    assert!(!reported(&changes, &path));
}
