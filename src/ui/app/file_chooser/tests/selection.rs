use super::{Kept, Selection};
use oxyde_lib::models::FileEntry;

fn entries(names: &[&str]) -> Vec<FileEntry> {
    names.iter().map(|name| FileEntry { name: (*name).into(), path: format!("/f/{name}"), ..Default::default() }).collect()
}

fn marked(selection: &Selection) -> Vec<usize> {
    selection.positions().collect()
}

#[test]
fn presses_follow_the_main_windows_rules() {
    let mut selection = Selection::default();
    selection.only(2);
    selection.extend(5, false);
    assert_eq!(marked(&selection), [2, 3, 4, 5]);
    // Shift again replaces the range from the same anchor.
    selection.extend(0, false);
    assert_eq!(marked(&selection), [0, 1, 2]);
    selection.toggle(7);
    assert_eq!(marked(&selection), [0, 1, 2, 7]);
    // Ctrl+Shift adds the range from the new anchor.
    selection.extend(9, true);
    assert_eq!(marked(&selection), [0, 1, 2, 7, 8, 9]);
    selection.toggle(8);
    assert_eq!(marked(&selection), [0, 1, 2, 7, 9]);
    assert_eq!(selection.cursor(), Some(8));
    selection.only(4);
    assert_eq!(marked(&selection), [4]);
    selection.all(3);
    assert_eq!(marked(&selection), [0, 1, 2]);
    assert!(selection.clear());
    assert!(!selection.clear());
    // Shift without an anchor selects the entry alone.
    selection.extend(3, false);
    assert_eq!(marked(&selection), [3]);
}

#[test]
fn the_selection_follows_its_entries_when_they_move() {
    let before = entries(&["a", "b", "c", "d"]);
    let mut selection = Selection::default();
    selection.only(1);
    selection.extend(2, false);
    let kept = selection.keep(&before);
    let after = entries(&["d", "c", "x", "b"]);
    let restored = Selection::restore(&kept, &after);
    assert_eq!(marked(&restored), [1, 3]);
    assert_eq!(restored.cursor(), Some(1));
    let gone = entries(&["x"]);
    assert_eq!(Selection::restore(&kept, &gone), Selection::default());
    let left = Selection::restore(&Kept::path("/f/c".into()), &after);
    assert_eq!(marked(&left), [1]);
}

#[test]
fn a_rectangle_replaces_the_selection_and_keeps_the_anchor_only_when_combined() {
    let mut selection = Selection::default();
    selection.only(3);
    assert!(selection.cover([5, 6].into_iter().collect(), false));
    assert_eq!(marked(&selection), [5, 6]);
    // The cursor left the selection: the arrows start from its first entry.
    assert_eq!(selection.cursor(), Some(5));
    // Without an anchor, Shift selects the entry alone.
    selection.extend(8, false);
    assert_eq!(marked(&selection), [8]);
    assert!(selection.cover([1, 8].into_iter().collect(), true));
    selection.extend(10, false);
    assert_eq!(marked(&selection), [8, 9, 10]);
    assert!(!selection.cover([8, 9, 10].into_iter().collect(), true));
}
