use super::*;

#[test]
fn bounded_listing_stops_at_its_limit_and_reports_the_end() {
    let root = tempfile::tempdir().unwrap();
    for index in 0..5 {
        std::fs::write(root.path().join(format!("file{index}")), b"x").unwrap();
    }
    let path = root.path().to_string_lossy().into_owned();
    let (entries, complete) = list_local_dir_bounded(&path, true, true, 3).unwrap();
    assert_eq!((entries.len(), complete), (3, false));
    let (entries, complete) = list_local_dir_bounded(&path, true, true, 5).unwrap();
    assert_eq!((entries.len(), complete), (5, true));
}

#[test]
fn bounded_listing_skips_hidden_entries_unless_shown() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join(".hidden"), b"x").unwrap();
    std::fs::write(root.path().join("shown"), b"x").unwrap();
    let path = root.path().to_string_lossy().into_owned();
    let names = |show_hidden| {
        let (entries, _) = list_local_dir_bounded(&path, show_hidden, true, 10).unwrap();
        let mut names: Vec<_> = entries.into_iter().map(|entry| entry.name).collect();
        names.sort();
        names
    };
    #[cfg(unix)]
    assert_eq!(names(false), ["shown"]);
    assert_eq!(names(true), [".hidden", "shown"]);
}

#[test]
fn contents_tell_empty_files_only_and_nested_folders_apart() {
    let root = tempfile::tempdir().unwrap();
    let empty = root.path().join("empty");
    std::fs::create_dir(&empty).unwrap();
    let files_only = root.path().join("files");
    std::fs::create_dir(&files_only).unwrap();
    std::fs::write(files_only.join("a"), b"x").unwrap();
    let nested = root.path().join("nested");
    std::fs::create_dir_all(nested.join("child")).unwrap();
    let text = |path: &std::path::Path| path.to_string_lossy().into_owned();
    assert_eq!(visible_contents(&text(&empty), true, true), (false, false));
    assert_eq!(visible_contents(&text(&files_only), true, true), (true, false));
    assert_eq!(visible_contents(&text(&nested), false, false), (true, true));
    assert_eq!(visible_contents(&text(&root.path().join("missing")), true, true), (false, false));
    #[cfg(unix)]
    {
        let hidden = root.path().join("hidden");
        std::fs::create_dir_all(hidden.join(".git")).unwrap();
        assert_eq!(visible_contents(&text(&hidden), false, false), (false, false));
        assert_eq!(visible_contents(&text(&hidden), true, false), (true, true));
    }
}

#[test]
fn walking_counts_the_whole_tree_and_stops_at_its_limit() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("a/b")).unwrap();
    std::fs::write(root.path().join("a/b/one"), b"123").unwrap();
    std::fs::write(root.path().join("two"), b"45").unwrap();
    let path = root.path().to_string_lossy().into_owned();
    let running = std::sync::atomic::AtomicBool::new(false);
    let mut last = ContentTotals::default();
    let complete = walk_contents(&path, true, true, 100, std::time::Duration::from_secs(10), &running, &mut |totals| last = *totals);
    assert!(complete);
    assert_eq!(last, ContentTotals { folders: 2, files: 2, bytes: 5 });
    let complete = walk_contents(&path, true, true, 2, std::time::Duration::from_secs(10), &running, &mut |totals| last = *totals);
    assert!(!complete);
    assert_eq!(last.folders + last.files, 2);
    let cancelled = std::sync::atomic::AtomicBool::new(true);
    assert!(!walk_contents(&path, true, true, 100, std::time::Duration::from_secs(10), &cancelled, &mut |_| {}));
}
