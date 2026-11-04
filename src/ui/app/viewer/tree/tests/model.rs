use super::*;

fn entry(path: &str, is_dir: bool, size: u64) -> FileEntry {
    FileEntry {
        name: Path::new(path).file_name().unwrap().to_string_lossy().into_owned(),
        path: path.to_owned(),
        is_dir,
        size,
        ..FileEntry::default()
    }
}

fn sort() -> (SortColumn, SortDirection) {
    (SortColumn::Name, SortDirection::Ascending)
}

fn contents(entries: bool, subfolders: bool) -> Contents {
    Contents { entries, subfolders }
}

fn ready(tree: &mut Tree, path: &str, entries: Vec<FileEntry>) {
    tree.levels.insert(path.to_owned(), Level::Ready(Folder::new(entries, true, sort())));
}

fn names(rows: &[Row]) -> Vec<String> {
    rows.iter()
        .map(|row| match row {
            Row::Entry { file, depth, .. } => format!("{depth}:{}", file.name),
            Row::Loading { depth } => format!("{depth}:loading"),
            Row::Failed { depth } => format!("{depth}:failed"),
            Row::Empty { depth } => format!("{depth}:empty"),
            Row::More { count, depth, .. } => format!("{depth}:+{count}"),
        })
        .collect()
}

#[test]
fn complete_levels_are_sorted_folders_first_and_counted_before_cutting() {
    let mut entries: Vec<_> = (0..SHOWN_PER_FOLDER + 5).map(|index| entry(&format!("/r/f{index:04}"), false, 2)).collect();
    entries.push(entry("/r/zdir", true, 0));
    let folder = Folder::new(entries, true, sort());
    assert_eq!(folder.entries[0].name, "zdir");
    assert_eq!((folder.folders, folder.files, folder.file_bytes), (1, SHOWN_PER_FOLDER + 5, 2 * (SHOWN_PER_FOLDER as u64 + 5)));
    assert_eq!(folder.entries.len(), SHOWN_PER_FOLDER);
    assert_eq!(folder.hidden(), 6);
}

#[test]
fn truncated_levels_keep_reading_order() {
    let folder = Folder::new(vec![entry("/r/b", false, 0), entry("/r/a", false, 0)], false, sort());
    assert_eq!(folder.entries[0].name, "b");
}

#[test]
fn only_folders_with_entries_open_and_their_level_loads_once() {
    let mut tree = Tree::new("/r".into(), Listing::default());
    ready(&mut tree, "/r", vec![entry("/r/a", true, 0), entry("/r/b", true, 0), entry("/r/c.txt", false, 1)]);
    tree.contents.insert("/r/a".into(), contents(true, false));
    tree.contents.insert("/r/b".into(), contents(false, false));
    assert_eq!(tree.toggle("/r/b"), Toggle::Refused);
    assert_eq!(tree.toggle("/r/c.txt"), Toggle::Refused);
    assert_eq!(tree.toggle("/r/a"), Toggle::Load);
    assert_eq!(names(&tree.flatten()), ["0:a", "1:loading", "0:b", "0:c.txt"]);
    ready(&mut tree, "/r/a", vec![entry("/r/a/x", false, 1)]);
    assert_eq!(names(&tree.flatten()), ["0:a", "1:x", "0:b", "0:c.txt"]);
    assert_eq!(tree.toggle("/r/a"), Toggle::Changed);
    assert_eq!(names(&tree.flatten()), ["0:a", "0:b", "0:c.txt"]);
    // Reopening reuses the level already read.
    assert_eq!(tree.toggle("/r/a"), Toggle::Changed);
    assert!(matches!(tree.flatten()[0], Row::Entry { chevron: Chevron::Expanded, subfolders: false, .. }));
    assert!(matches!(tree.flatten()[2], Row::Entry { chevron: Chevron::None, .. }));
    tree.contents.insert("/r/b".into(), contents(true, true));
    assert!(matches!(tree.flatten()[2], Row::Entry { chevron: Chevron::Collapsed, subfolders: true, .. }));
}

#[test]
fn hidden_entries_and_empty_levels_get_their_own_rows() {
    let mut tree = Tree::new("/r".into(), Listing::default());
    let mut entries = vec![entry("/r/a", true, 0)];
    entries.extend((0..SHOWN_PER_FOLDER).map(|index| entry(&format!("/r/f{index:04}"), false, 0)));
    ready(&mut tree, "/r", entries);
    tree.contents.insert("/r/a".into(), contents(true, false));
    ready(&mut tree, "/r/a", Vec::new());
    tree.toggle("/r/a");
    let rows = tree.flatten();
    assert_eq!(names(&rows[..2]), ["0:a", "1:empty"]);
    assert!(matches!(rows.last(), Some(Row::More { count: 1, exact: true, depth: 0, folder }) if folder == "/r"));

    let mut empty = Tree::new("/e".into(), Listing::default());
    ready(&mut empty, "/e", Vec::new());
    assert_eq!(names(&empty.flatten()), ["0:empty"]);
}

#[test]
fn the_row_limit_disables_closed_folders() {
    let mut tree = Tree::new("/r".into(), Listing::default());
    let mut root = Vec::new();
    for folder in 0..=MAX_ROWS / SHOWN_PER_FOLDER {
        let path = format!("/r/d{folder:03}");
        root.push(entry(&path, true, 0));
        tree.contents.insert(path.clone(), contents(true, false));
        ready(&mut tree, &path, (0..SHOWN_PER_FOLDER).map(|index| entry(&format!("{path}/f{index:04}"), false, 0)).collect());
    }
    ready(&mut tree, "/r", root);
    let folders = MAX_ROWS / SHOWN_PER_FOLDER;
    for folder in 0..folders {
        assert_ne!(tree.toggle(&format!("/r/d{folder:03}")), Toggle::Refused);
        tree.rows = Arc::new(tree.flatten());
    }
    assert!(tree.rows.len() >= MAX_ROWS);
    let last = format!("/r/d{folders:03}");
    assert_eq!(tree.toggle(&last), Toggle::Refused);
    assert!(tree.rows.iter().any(|row| matches!(row, Row::Entry { file, chevron: Chevron::Disabled, .. } if file.path == last)));
    // Closing stays possible.
    assert_eq!(tree.toggle("/r/d000"), Toggle::Changed);
}

#[test]
fn archive_levels_include_folders_known_only_from_member_paths() {
    let archive = Path::new("/x/a.zip");
    let levels = archive_levels(archive, vec![
        entry("/x/a.zip/docs/guide/intro.md", false, 10),
        entry("/x/a.zip/docs", true, 0),
        entry("/x/a.zip/readme", false, 3),
    ]);
    let mut root: Vec<_> = levels["/x/a.zip"].iter().map(|entry| (entry.name.clone(), entry.is_dir)).collect();
    root.sort();
    assert_eq!(root, [("docs".to_owned(), true), ("readme".to_owned(), false)]);
    let guide = &levels["/x/a.zip/docs"];
    assert_eq!((guide.len(), guide[0].name.as_str(), guide[0].is_dir), (1, "guide", true));
    assert_eq!(levels["/x/a.zip/docs/guide"][0].size, 10);
    assert!(levels["/x/a.zip/docs/guide"][0].archive_path.as_deref() == Some(&archive.to_path_buf()));
    assert!(!levels.contains_key("/x/a.zip/readme"));
}

#[test]
fn archive_ratio_needs_both_sizes() {
    let info = |packed, extracted| ArchiveInfo { extracted, packed, password: false };
    assert_eq!(info(Some(25), 100).percent(), Some(25));
    assert_eq!(info(Some(1), 3).percent(), Some(33));
    assert_eq!(info(None, 100).percent(), None);
    assert_eq!(info(Some(10), 0).percent(), None);
}
