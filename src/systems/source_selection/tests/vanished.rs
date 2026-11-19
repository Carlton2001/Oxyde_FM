use super::*;

#[test]
fn a_child_gone_before_its_capture_reports_not_found() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    std::fs::create_dir(&folder).unwrap();
    let selected = SelectedEntry::select(&folder).unwrap();
    let directory = Arc::clone(selected.directory.as_ref().unwrap());
    let error = SelectedEntry::child(folder.join("gone.tmp"), directory).err().unwrap();
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
}

#[test]
fn the_walk_lists_every_child_still_present() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("folder");
    std::fs::create_dir_all(folder.join("sub")).unwrap();
    std::fs::write(folder.join("a.txt"), b"a").unwrap();
    std::fs::write(folder.join("sub").join("b.txt"), b"b").unwrap();
    let mut walk = SourceWalk::new(SelectedEntry::select(&folder).unwrap());
    let mut seen = Vec::new();
    while let Some(entry) = walk.next(&mut || Ok(())).unwrap() {
        seen.push(entry.path.strip_prefix(root.path()).unwrap().to_path_buf());
    }
    seen.sort();
    let expected: Vec<std::path::PathBuf> = ["folder", "folder/a.txt", "folder/sub", "folder/sub/b.txt"]
        .iter()
        .map(|path| path.split('/').collect())
        .collect();
    assert_eq!(seen, expected);
}
