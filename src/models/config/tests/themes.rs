use super::{AppConfig, ConfigManager};
use crate::models::custom_theme::CustomTheme;
use crate::models::preferences::ThemeId;

fn theme(id: &str) -> CustomTheme {
    CustomTheme::new(id.into(), "Mine".into(), ThemeId::Dark, [0x101010, 0x202020, 0x303030, 0x404040, 0xf0f0f0, 0x3080ff])
}

#[test]
fn custom_themes_live_outside_the_configuration_and_survive_its_removal() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    let config = AppConfig { custom_themes: vec![theme("theme-1")], ..AppConfig::default() };
    ConfigManager::write_to_path(&path, &config).unwrap();
    assert!(!std::fs::read_to_string(&path).unwrap().contains("custom_themes"));
    assert!(ConfigManager::themes_path(&path).exists());

    std::fs::remove_file(&path).unwrap();
    let restored = ConfigManager::read_from_path(&path).unwrap();
    assert_eq!(restored.custom_themes, config.custom_themes);
}

#[test]
fn themes_stored_in_older_configurations_move_to_their_own_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    let legacy = serde_json::json!({"theme": "custom:theme-1", "custom_themes": [theme("theme-1")]});
    std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    let config = ConfigManager::read_from_path(&path).unwrap();
    assert_eq!(config.custom_themes, vec![theme("theme-1")]);

    ConfigManager::write_to_path(&path, &config).unwrap();
    std::fs::write(&path, "{}").unwrap();
    assert_eq!(ConfigManager::read_from_path(&path).unwrap().custom_themes, vec![theme("theme-1")]);
}

#[test]
fn a_corrupt_themes_file_is_reported_and_left_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    let themes = ConfigManager::themes_path(&path);
    std::fs::write(&themes, "{broken").unwrap();
    assert!(ConfigManager::read_from_path(&path).is_err());
    assert_eq!(std::fs::read_to_string(&themes).unwrap(), "{broken");
}
