use super::*;

#[test]
fn a_copy_of_a_long_name_stays_within_255_characters() {
    let root = tempfile::tempdir().unwrap();
    let stem = "a".repeat(250);
    let source = root.path().join(format!("{stem}.txt"));
    std::fs::write(&source, b"x").unwrap();
    let target = copy_name(&source, root.path(), false, "Copie", &mut HashSet::new()).unwrap();
    let name = target.file_name().unwrap();
    assert!(fits_name(name), "{}", name.to_string_lossy());
    let text = name.to_string_lossy();
    assert!(text.ends_with(" - Copie.txt"), "{text}");
    assert!(text.starts_with("aaaa"));
    std::fs::write(&target, b"x").unwrap();
}
