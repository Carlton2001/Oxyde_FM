use super::*;

#[test]
fn labels_accept_repeated_identical_types_and_reject_ambiguous_matches() {
    let mut types = ShellNewTypes::default();
    types.add("Image &bitmap", ".BMP", false);
    types.add("Image bitmap", ".bmp", false);
    assert_eq!(types.extension(" Image bitmap "), Some(".bmp"));
    types.add("Image bitmap", ".dib", false);
    types.add("Image bitmap", ".bmp", false);
    assert_eq!(types.extension("Image bitmap"), None);
    assert_eq!(types.extension("Other"), None);
}

#[test]
fn remembers_types_handed_to_a_program() {
    let mut types = ShellNewTypes::default();
    types.add("Shortcut", ".LNK", true);
    types.add("Text Document", ".txt", false);
    assert!(types.runs_program(".lnk"));
    assert!(!types.runs_program(".txt"));
    assert!(!types.runs_program(".bmp"));
}
