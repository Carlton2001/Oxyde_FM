use super::*;

#[test]
fn a_byte_order_mark_is_accepted() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.json");
    fs::write(&path, "\u{feff}{\"language\":\"de\",\"search_limit\":4000}").unwrap();
    let config = ConfigManager::read_from_path(&path).unwrap();
    assert_eq!(config.language, "de");
    assert_eq!(config.search_limit, 4000);
    assert!(!root.path().join("config.json.invalid").exists());
}

#[test]
fn a_mistyped_field_is_read_as_missing_and_keeps_the_other_settings() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.json");
    let text = r#"{"language":"de","search_limit":"many","show_hidden":true}"#;
    fs::write(&path, text).unwrap();
    let config = ConfigManager::read_from_path(&path).unwrap();
    assert_eq!(config.language, "de");
    assert!(config.show_hidden);
    let missing: AppConfig = serde_json::from_str("{}").unwrap();
    assert_eq!(config.search_limit, missing.search_limit);
    assert_eq!(fs::read_to_string(root.path().join("config.json.invalid")).unwrap(), text);
}

#[test]
fn text_that_is_not_an_object_is_still_refused() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.json");
    fs::write(&path, "{\"language\":").unwrap();
    assert!(ConfigManager::read_from_path(&path).is_err());
}
