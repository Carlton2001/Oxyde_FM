use super::*;
use std::sync::Arc;

fn entry(path: &str, size: u64) -> FileEntry {
    FileEntry { path: path.into(), name: path.rsplit('/').next().unwrap().into(), size, ..Default::default() }
}

#[test]
fn the_snapshot_is_kept_until_the_selection_or_the_files_change() {
    let mut tab = TabState::new("test".into(), "C:/folder".into());
    tab.files = Arc::new(vec![entry("C:/folder/a.txt", 1), entry("C:/folder/b.txt", 2), entry("C:/folder/c.log", 4)]);
    tab.update_cache();
    tab.selected_paths = ["C:/folder/c.log", "C:/folder/a.txt", "C:/folder/gone.txt"].map(String::from).into();
    let first = tab.selection_snapshot();
    assert_eq!(*first.shown, ["C:/folder/a.txt", "C:/folder/c.log"]);
    assert_eq!((first.folders, first.file_count, first.bytes), (0, 2, 5));
    assert!(std::rc::Rc::ptr_eq(&first, &tab.selection_snapshot()));
    tab.selected_paths.remove("C:/folder/a.txt");
    let second = tab.selection_snapshot();
    assert!(!std::rc::Rc::ptr_eq(&first, &second));
    assert_eq!((second.file_count, second.bytes), (1, 4));
    tab.filter_name = "a.txt".into();
    tab.update_cache();
    assert!(tab.selection_snapshot().shown.is_empty());
}
