use super::cache_key;

#[test]
fn replaced_file_with_its_own_icon_gets_a_new_key() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("tool.exe");
    std::fs::write(&path, b"old").unwrap();
    let path = path.to_string_lossy().into_owned();
    let before = cache_key(&path, "small");
    assert_eq!(cache_key(&path.to_uppercase(), "small"), before);
    std::fs::write(&path, b"replaced").unwrap();
    assert_ne!(cache_key(&path, "small"), before);
}

#[test]
fn shared_type_icons_keep_a_path_only_key() {
    assert_eq!(cache_key("oxyde_ext_txt", "large"), cache_key("OXYDE_EXT_TXT", "large"));
}
