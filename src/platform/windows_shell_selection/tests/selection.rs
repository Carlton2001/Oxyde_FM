use super::shell_selection;
use crate::platform::windows_com::ComApartment;

#[test]
fn shell_array_preserves_files_from_different_directories() {
    let _com = ComApartment::sta().unwrap();
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let paths: Vec<_> = [first.path().join("first.txt"), second.path().join("second.txt")]
        .into_iter().map(|path| {
            std::fs::write(&path, b"selection").unwrap();
            path.to_string_lossy().into_owned()
        }).collect();
    let selection = shell_selection(&paths).unwrap();
    assert_eq!(unsafe { selection.GetCount().unwrap() }, 2);
}
