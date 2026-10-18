use super::*;
use oxyde_lib::models::file_entry::NativeFileKind;

#[test]
fn preparation_attaches_native_categories_before_publishing_filtered_files() {
    let mut tab = TabState::new("tab".into(), "C:/".into());
    let files = Arc::new(vec![
        FileEntry { name: "note.TXT".into(), ..Default::default() },
        FileEntry { name: "other.txt".into(), ..Default::default() },
        FileEntry { name: "folder.txt".into(), is_dir: true, ..Default::default() },
        FileEntry { name: "unknown.oxyde_unknown_73ae51".into(), ..Default::default() },
    ]);
    let result = tab.preparation(FileUpdate::Listing(files)).calculate();
    assert!(tab.apply_preparation(result));
    for entry in tab.filtered_files() {
        let expected = if entry.name.ends_with("TXT") || entry.name == "other.txt" { Some(NativeFileKind::Documents) } else { None };
        assert_eq!(entry.native_kind, expected, "{}", entry.name);
    }
}
