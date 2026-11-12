use super::*;

#[test]
fn renamed_item_keeps_its_selection() {
    let mut tab = TabState::new("tab".into(), "/home".into());
    tab.selected_paths.insert("/home/New File".into());
    tab.selected_paths.insert("/home/other".into());
    tab.selection_anchor = Some("/home/New File".into());
    tab.keyboard_cursor = Some("/home/New File".into());

    assert!(tab.follow_renamed_item("/home/New File", "/home/notes.txt"));

    assert!(tab.selected_paths.contains("/home/notes.txt"));
    assert!(tab.selected_paths.contains("/home/other"));
    assert!(!tab.selected_paths.contains("/home/New File"));
    assert_eq!(tab.selection_anchor.as_deref(), Some("/home/notes.txt"));
    assert_eq!(tab.keyboard_cursor.as_deref(), Some("/home/notes.txt"));
}

#[test]
fn unselected_item_is_not_selected_by_its_rename() {
    let mut tab = TabState::new("tab".into(), "/home".into());
    tab.selected_paths.insert("/home/other".into());
    tab.selection_anchor = Some("/home/other".into());

    assert!(!tab.follow_renamed_item("/home/New File", "/home/notes.txt"));

    assert_eq!(tab.selected_paths.len(), 1);
    assert!(tab.selected_paths.contains("/home/other"));
    assert_eq!(tab.selection_anchor.as_deref(), Some("/home/other"));
    assert!(tab.keyboard_cursor.is_none());
}
