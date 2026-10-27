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

#[test]
fn manual_size_flush_keeps_listing_work_and_consumes_each_size_change_once() {
    let root = root();
    let path = root.to_string_lossy().into_owned();
    let mut change = DirectoryChanges::default();
    change.record(
        &path,
        &Event::new(EventKind::Create(CreateKind::Folder)).add_path(root.join("direct")),
    );
    change.record(
        &path,
        &Event::new(EventKind::Modify(ModifyKind::Any)).add_path(root.join("deep").join("file")),
    );
    let changes = Arc::new(Mutex::new(HashMap::from([(path.clone(), change)])));
    let SizeChanges::Children(children) = take_size_changes(&changes, &path) else {
        panic!("missing child changes")
    };
    assert_eq!(
        children,
        HashSet::from([
            root.join("direct").to_string_lossy().into_owned(),
            root.join("deep").to_string_lossy().into_owned()
        ])
    );
    assert!(changes.lock().unwrap()[&path].listing);
    assert!(matches!(
        take_size_changes(&changes, &path),
        SizeChanges::None
    ));
    changes.lock().unwrap().get_mut(&path).unwrap().record(
        &path,
        &Event::new(EventKind::Modify(ModifyKind::Any)).add_path(root.join("later").join("file")),
    );
    let SizeChanges::Children(later) = take_size_changes(&changes, &path) else {
        panic!("lost later change")
    };
    assert_eq!(
        later,
        HashSet::from([root.join("later").to_string_lossy().into_owned()])
    );
}

#[test]
fn manual_size_flush_preserves_full_rescan_without_repeating_it() {
    let path = root().to_string_lossy().into_owned();
    let mut change = DirectoryChanges::default();
    change.rescan();
    let changes = Arc::new(Mutex::new(HashMap::from([(path.clone(), change)])));
    assert!(matches!(
        take_size_changes(&changes, &path),
        SizeChanges::All
    ));
    assert!(changes.lock().unwrap()[&path].listing);
    assert!(matches!(
        take_size_changes(&changes, &path),
        SizeChanges::None
    ));
}

#[test]
fn manual_size_flush_clears_only_the_target_root_when_no_listing_is_pending() {
    let root = root();
    let path = root.to_string_lossy().into_owned();
    let mut change = DirectoryChanges::default();
    change.record(
        &path,
        &Event::new(EventKind::Modify(ModifyKind::Any)).add_path(root.join("deep").join("file")),
    );
    let other = root.with_file_name("other").to_string_lossy().into_owned();
    let changes = Arc::new(Mutex::new(HashMap::from([
        (path.clone(), change),
        (other.clone(), DirectoryChanges::default()),
    ])));
    assert!(matches!(
        take_size_changes(&changes, &path),
        SizeChanges::Children(_)
    ));
    let queue = changes.lock().unwrap();
    assert!(!queue.contains_key(&path));
    assert!(queue.contains_key(&other));
}

fn recursive_changes(polling: bool) {
    let directory = tempfile::tempdir().unwrap();
    let child = directory.path().join("child");
    std::fs::create_dir(&child).unwrap();
    let path = directory.path().to_string_lossy().into_owned();
    let changes = Arc::new(Mutex::new(HashMap::new()));
    let watcher = DirectoryWatcher::with_poll_interval(
        &path,
        polling,
        true,
        changes.clone(),
        std::time::Duration::from_millis(100),
    )
    .unwrap();
    assert_eq!(watcher.reliable_size_events(), !polling);
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

#[cfg(target_os = "linux")]
#[test]
fn installing_a_recursive_watch_on_a_large_tree_does_not_invalidate_every_size() {
    let root = tempfile::tempdir().unwrap();
    // Each folder read during installation queues an event: more than the
    // default inotify queue of 16 384.
    for index in 0..20_000 {
        std::fs::create_dir_all(root.path().join(format!("{}", index % 100)).join(format!("{index}"))).unwrap();
    }
    let changes: WatcherChanges = Default::default();
    let path = root.path().to_string_lossy().into_owned();
    let _watcher = DirectoryWatcher::create(&path, false, true, changes.clone()).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1500));
    assert!(changes.lock().unwrap().get(&path).is_none_or(|change| !change.all_sizes));
    std::fs::write(root.path().join("7").join("7").join("new"), "x").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    let queue = changes.lock().unwrap();
    let change = queue.get(&path).unwrap();
    assert!(!change.all_sizes);
    assert_eq!(change.children, HashSet::from([root.path().join("7").to_string_lossy().into_owned()]));
}
