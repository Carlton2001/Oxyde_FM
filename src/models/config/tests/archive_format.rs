use super::AppConfig;

#[test]
fn archive_format_defaults_and_round_trips() {
    let old: AppConfig = serde_json::from_str("{}").unwrap();
    assert_eq!(old.default_archive_format, "zip");
    assert_eq!(AppConfig::default().default_archive_format, "zip");
    for format in ["zip", "7z", "tar", "zst"] {
        let mut config = AppConfig::default();
        config.default_archive_format = format.into();
        let saved = serde_json::to_string(&config).unwrap();
        let restored: AppConfig = serde_json::from_str(&saved).unwrap();
        assert_eq!(restored.default_archive_format, format);
        config.reset_preferences();
        assert_eq!(config.default_archive_format, "zip");
    }
}
