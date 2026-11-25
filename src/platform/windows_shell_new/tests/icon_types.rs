use super::*;

#[test]
fn icon_labels_accept_repeated_identical_types_and_reject_ambiguous_matches() {
    let mut types = IconTypes::default();
    types.add("Image &bitmap", ".BMP");
    types.add("Image bitmap", ".bmp");
    assert_eq!(types.extension(" Image bitmap "), Some(".bmp"));
    types.add("Image bitmap", ".dib");
    types.add("Image bitmap", ".bmp");
    assert_eq!(types.extension("Image bitmap"), None);
    assert_eq!(types.extension("Other"), None);
}
