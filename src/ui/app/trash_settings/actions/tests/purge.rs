use super::{purge_needs_confirmation, trash_targets};
use oxyde_lib::models::FileEntry;
use std::collections::HashSet;

fn item(path: &str, name: &str) -> FileEntry {
    FileEntry { path: path.into(), name: name.into(), ..Default::default() }
}

#[test]
fn emptying_the_whole_trash_always_asks() {
    assert!(purge_needs_confirmation(true, false));
    assert!(purge_needs_confirmation(true, true));
}

#[test]
fn purging_a_selection_follows_the_preference() {
    assert!(purge_needs_confirmation(false, true));
    assert!(!purge_needs_confirmation(false, false));
}

#[test]
fn targets_keep_the_listed_order_and_show_original_names() {
    let files = [item("C:\\$Recycle.Bin\\S\\$RA1.txt", "a.txt"), item("C:\\$Recycle.Bin\\S\\$RB2.txt", "b.txt")];
    let selected = HashSet::from([files[1].path.clone()]);

    let (paths, names) = trash_targets(&files, &selected, false);
    assert_eq!(paths, [files[1].path.clone()]);
    assert_eq!(names, ["b.txt"]);

    let (paths, names) = trash_targets(&files, &selected, true);
    assert_eq!(paths, [files[0].path.clone(), files[1].path.clone()]);
    assert_eq!(names, ["a.txt", "b.txt"]);
}
