use super::{origin, same_folder};

#[test]
fn a_selected_folder_is_matched_whatever_its_case_and_trailing_separator() {
    assert!(same_folder(r"C:\Users\Public", r"c:\users\public\"));
    assert!(same_folder(r"E:\", "E:"));
    assert!(!same_folder(r"C:\Users\Public", r"C:\Users\Public2"));
}

#[test]
fn the_foreground_window_is_the_origin_among_those_selecting_the_folder() {
    assert_eq!(origin(&[4, 7, 9], &7), Some(1));
}

#[test]
fn the_only_window_selecting_the_folder_stands_for_a_foreground_that_does_not() {
    assert_eq!(origin(&[4], &2), Some(0));
}

#[test]
fn a_folder_selected_nowhere_or_in_several_background_windows_has_no_origin() {
    assert_eq!(origin::<i32>(&[], &2), None);
    assert_eq!(origin(&[4, 7], &2), None);
}
