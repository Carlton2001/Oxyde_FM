use super::*;
use notify::event::{CreateKind, ModifyKind, RenameMode};

fn root() -> std::path::PathBuf {
    std::env::temp_dir().join("oxyde-watcher-events")
}

#[test]
fn nested_writes_coalesce_by_child_without_reloading_root_listing() {
    let root = root();
    let mut changes = DirectoryChanges::default();
    for index in 0..10_000 {
        changes.record(
            &root.to_string_lossy(),
            &Event::new(EventKind::Modify(ModifyKind::Any))
                .add_path(root.join("child").join(format!("file-{index}"))),
        );
    }
    assert!(!changes.listing);
    assert!(!changes.all_sizes);
    assert_eq!(
        changes.children,
        HashSet::from([root.join("child").to_string_lossy().into_owned()])
    );
}

#[test]
fn rename_tracks_both_parents_and_direct_changes_reload_listing() {
    let root = root();
    let mut changes = DirectoryChanges::default();
    changes.record(
        &root.to_string_lossy(),
        &Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
            .add_path(root.join("a").join("old"))
            .add_path(root.join("b").join("new")),
    );
    assert_eq!(changes.children.len(), 2);
    assert!(!changes.listing);
    changes.record(
        &root.to_string_lossy(),
        &Event::new(EventKind::Create(CreateKind::Folder)).add_path(root.join("new")),
    );
    assert!(changes.listing);
    assert!(changes
        .children
        .contains(&root.join("new").to_string_lossy().into_owned()));
}

#[test]
fn overflow_unknown_paths_and_lost_events_request_a_full_rescan() {
    let root = root();
    let mut changes = DirectoryChanges::default();
    for index in 0..1000 {
        changes.record(
            &root.to_string_lossy(),
            &Event::new(EventKind::Any).add_path(root.join(index.to_string()).join("file")),
        );
    }
    assert!(changes.all_sizes);
    assert!(changes.children.is_empty());
    assert!(!changes.listing);
    for event in [
        Event::new(EventKind::Any),
        Event::new(EventKind::Any)
            .set_flag(notify::event::Flag::Rescan)
            .add_path(root.join("a").join("file")),
        Event::new(EventKind::Any).add_path(root.with_file_name("other").join("file")),
        Event::new(EventKind::Any).add_path(root.clone()),
    ] {
        let mut changes = DirectoryChanges::default();
        changes.record(&root.to_string_lossy(), &event);
        assert!(changes.all_sizes && changes.listing);
    }
}

#[test]
fn recursive_polling_detects_nested_writes_and_releases_its_snapshot() {
    recursive_changes(true);
}

#[test]
fn recursive_native_watcher_detects_nested_writes_and_releases_handles() {
    recursive_changes(false);
}

fn recursive_changes(polling: bool) {
    let directory = tempfile::tempdir().unwrap();
    let child = directory.path().join("child");
    std::fs::create_dir(&child).unwrap();
    let path = directory.path().to_string_lossy().into_owned();
    let changes = Arc::new(Mutex::new(HashMap::new()));
    let watcher = DirectoryWatcher::create(&path, polling, true, changes.clone()).unwrap();
    // PollWatcher takes its initial snapshot synchronously during watch().
    changes.lock().unwrap().clear();
    std::fs::write(child.join("new-file"), b"new bytes").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    loop {
        if changes.lock().unwrap().get(&path).is_some_and(|change| {
            change
                .children
                .contains(&child.to_string_lossy().into_owned())
        }) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "nested change was not reported"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    watcher.release(&path).unwrap();
    directory.close().unwrap();
}
