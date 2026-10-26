use super::select_keyed;
use crate::models::error::TrashRestoreIssue;

fn keyed(content: &str, original: &str) -> (String, trash::TrashItem) {
    let original = std::path::Path::new(original);
    (content.into(), trash::TrashItem {
        id: format!("{content}.trashinfo").into(),
        name: original.file_name().unwrap().into(),
        original_parent: original.parent().unwrap().into(),
        time_deleted: 0,
    })
}

fn listing() -> Vec<(String, trash::TrashItem)> {
    vec![
        keyed("/t/files/Rapport.txt", "/home/u/Rapport.txt"),
        keyed("/t/files/rapport.txt", "/home/u/rapport.txt"),
        keyed("/t/files/note.txt", "/home/u/a/note.txt"),
        keyed("/t/files/note.2.txt", "/home/u/a/note.txt"),
    ]
}

#[test]
fn content_paths_match_exactly_without_case_folding() {
    let (selected, failures) = select_keyed(listing(), vec!["/t/files/rapport.txt".into(), "/t/files/RAPPORT.TXT".into()]);
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].original_path(), std::path::Path::new("/home/u/rapport.txt"));
    assert_eq!(failures, vec![("/t/files/RAPPORT.TXT".into(), TrashRestoreIssue::Unavailable)]);
}

#[test]
fn undo_selects_by_original_path_only_when_one_item_came_from_there() {
    let (selected, failures) = select_keyed(listing(), vec!["/home/u/Rapport.txt".into(), "/home/u/a/note.txt".into()]);
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].id, "/t/files/Rapport.txt.trashinfo");
    assert_eq!(failures, vec![("/home/u/a/note.txt".into(), TrashRestoreIssue::Ambiguous)]);
}

#[test]
fn an_item_requested_under_both_paths_is_selected_once() {
    let (selected, failures) = select_keyed(listing(), vec!["/home/u/Rapport.txt".into(), "/t/files/Rapport.txt".into()]);
    assert_eq!(selected.len(), 1);
    assert!(failures.is_empty());
}
