use super::{parse, read};

#[test]
fn names_are_whole_lines_and_empty_lines_are_skipped() {
    let names = parse("build\n\nnotes draft.txt\nlast");
    assert_eq!(names.len(), 3);
    assert!(names.contains("notes draft.txt") && names.contains("last"));
}

#[test]
fn missing_or_directory_list_hides_nothing() {
    let dir = tempfile::tempdir().unwrap();
    assert!(read(dir.path()).is_empty());
    std::fs::create_dir(dir.path().join(".hidden")).unwrap();
    assert!(read(dir.path()).is_empty());
}

#[test]
fn list_file_is_read_from_the_directory() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".hidden"), "snap\n").unwrap();
    assert!(read(dir.path()).contains("snap"));
}
