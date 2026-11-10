use super::same_folder;

#[test]
fn a_selected_folder_is_matched_whatever_its_case_and_trailing_separator() {
    assert!(same_folder(r"C:\Users\Public", r"c:\users\public\"));
    assert!(same_folder(r"E:\", "E:"));
    assert!(!same_folder(r"C:\Users\Public", r"C:\Users\Public2"));
}
