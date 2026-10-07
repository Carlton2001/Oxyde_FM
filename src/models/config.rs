use crate::models::CommandError;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub show_hidden: bool,
    #[serde(default)]
    pub show_system: bool,
    #[serde(default)]
    pub use_system_icons: bool,
    #[serde(default)]
    pub date_format: String,
    #[serde(default = "default_true")]
    pub show_previews: bool,
    #[serde(default)]
    pub zip_quality: String,
    #[serde(default)]
    pub seven_zip_quality: String,
    #[serde(default)]
    pub zstd_quality: String,
    /// GPUI reference in logical pixels per rem.
    #[serde(default = "default_ui_rem_size")]
    pub ui_rem_size: u32,
    #[serde(default)]
    pub search_limit: u32,
    #[serde(default)]
    pub default_turbo_mode: bool,
    #[serde(default)]
    pub show_grid_thumbnails: bool,
    #[serde(default)]
    pub show_checkboxes: bool,
    #[serde(default = "default_true")]
    pub sync_sidebar_with_path: bool,
    #[serde(default)]
    pub show_network: bool,
    /// Window-local top-left coordinates for the operations panel.
    #[serde(default)]
    pub operations_panel_position: Option<[f32; 2]>,
    #[serde(default = "default_true")]
    pub confirm_delete: bool,
    /// Snapshots interpreted and validated by their GPUI views.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_state: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duplicate_search_options: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duplicate_search_window_size: Option<serde_json::Value>,
}

fn default_theme() -> String {
    "dark".to_string()
}
fn default_language() -> String {
    "fr".to_string()
}

fn default_true() -> bool {
    true
}

fn default_ui_rem_size() -> u32 { 16 }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            language: default_language(),
            show_hidden: false,
            show_system: false,
            use_system_icons: false,
            date_format: String::new(),
            show_previews: true,
            zip_quality: "fast".to_string(),
            seven_zip_quality: "fast".to_string(),
            zstd_quality: "fast".to_string(),
            ui_rem_size: default_ui_rem_size(),
            search_limit: 3000,
            default_turbo_mode: true,
            show_grid_thumbnails: false,
            show_checkboxes: false,
            sync_sidebar_with_path: true,
            show_network: true,
            operations_panel_position: None,
            confirm_delete: true,
            workspace_state: None,
            duplicate_search_options: None,
            duplicate_search_window_size: None,
        }
    }
}

impl AppConfig {
    pub fn for_system_locale(locale: Option<&str>) -> Self {
        Self {
            language: super::preferences::Language::from_system_locale(locale).as_str().into(),
            ..Self::default()
        }
    }

    /// Restore preferences while retaining workspace and duplicate-search state.
    pub fn reset_preferences(&mut self) {
        let workspace_state = self.workspace_state.take();
        let duplicate_search_options = self.duplicate_search_options.take();
        let duplicate_search_window_size = self.duplicate_search_window_size.take();
        let operations_panel_position = self.operations_panel_position;
        *self = Self {
            workspace_state,
            duplicate_search_options,
            duplicate_search_window_size,
            operations_panel_position,
            ..Self::default()
        };
    }
}

#[cfg(test)]
mod reset_tests {
    use super::AppConfig;

    #[test]
    fn reset_restores_defaults_and_preserves_workspace_and_search_state() {
        let mut config = AppConfig::default();
        config.theme = "light".into();
        config.language = "en".into();
        config.ui_rem_size = 24;
        config.show_hidden = true;
        config.zip_quality = "best".into();
        config.search_limit = 4500;
        config.default_turbo_mode = false;
        config.workspace_state = Some(serde_json::json!({"tabs": ["C:/"]}));
        config.duplicate_search_options = Some(serde_json::json!([true, false, true]));
        config.duplicate_search_window_size = Some(serde_json::json!([900.0, 650.0]));
        config.operations_panel_position = Some([20.0, 30.0]);
        let previous = config.clone();
        config.reset_preferences();
        assert_eq!(config.theme, "dark");
        assert_eq!(config.language, "fr");
        assert_eq!(config.ui_rem_size, 16);
        assert!(!config.show_hidden);
        assert_eq!(config.zip_quality, "fast");
        assert_eq!(config.search_limit, 3000);
        assert!(config.default_turbo_mode);
        assert_eq!(config.workspace_state, previous.workspace_state);
        assert_eq!(config.duplicate_search_options, previous.duplicate_search_options);
        assert_eq!(config.duplicate_search_window_size, previous.duplicate_search_window_size);
        assert_eq!(config.operations_panel_position, Some([20.0, 30.0]));
    }
}

/// Reads and atomically writes the configuration used by the GPUI workspace.
pub struct ConfigManager;

impl ConfigManager {
    pub fn config_path() -> Result<std::path::PathBuf, CommandError> {
        dirs::config_dir()
            .map(|dir| dir.join("oxyde").join("config.json"))
            .ok_or_else(|| CommandError::SystemError("Failed to get config directory".into()))
    }

    pub fn read_from_path(path: &std::path::Path) -> Result<AppConfig, CommandError> {
        match fs::read_to_string(path) {
            Ok(content) => {
                serde_json::from_str(&content).map_err(|e| CommandError::Other(e.to_string()))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(AppConfig::for_system_locale(crate::platform::platform().interface_locale().as_deref()))
            }
            Err(error) => Err(CommandError::IoError(error.to_string())),
        }
    }

    /// Replace only after the complete configuration has been written successfully.
    pub fn write_to_path(path: &std::path::Path, config: &AppConfig) -> Result<(), CommandError> {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        fs::create_dir_all(parent).map_err(|e| CommandError::IoError(e.to_string()))?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .map_err(|e| CommandError::IoError(e.to_string()))?;
        serde_json::to_writer_pretty(&mut temporary, config)
            .map_err(|e| CommandError::Other(e.to_string()))?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|e| CommandError::IoError(e.to_string()))?;
        temporary
            .persist(path)
            .map_err(|e| CommandError::IoError(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::preferences::{Language, ThemeId};

    #[test]
    fn fresh_configuration_detects_language_and_preserves_saved_choices() {
        for language in Language::ALL {
            let config = AppConfig::for_system_locale(Some(language.as_str()));
            assert_eq!(config.language, language.as_str());
        }
        for locale in [None, Some(""), Some("pt-BR")] {
            assert_eq!(AppConfig::for_system_locale(locale).language, "en");
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        for language in Language::ALL {
            let config = AppConfig::for_system_locale(Some(language.as_str()));
            ConfigManager::write_to_path(&path, &config).unwrap();
            assert_eq!(ConfigManager::read_from_path(&path).unwrap().language, language.as_str());
        }
    }

    #[test]
    fn older_configs_have_defaults_and_legacy_identifiers_still_resolve() {
        let missing: AppConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(missing.theme, "dark");
        assert_eq!(missing.language, "fr");
        assert_eq!(missing.ui_rem_size, 16);
        assert!(!missing.use_system_icons);
        assert!(!missing.show_checkboxes);
        assert!(missing.sync_sidebar_with_path);
        let legacy: AppConfig =
            serde_json::from_str(r#"{"theme":"oxyde-light","language":"en-US"}"#).unwrap();
        assert_eq!(ThemeId::from_id(&legacy.theme), ThemeId::Light);
        assert_eq!(Language::from_id(&legacy.language), Language::English);
        assert_eq!(ThemeId::from_id(""), ThemeId::Dark);
        assert_eq!(ThemeId::from_id("unknown"), ThemeId::Dark);
        assert_eq!(Language::from_id("de-DE"), Language::German);
        assert_eq!(Language::from_id("unknown"), Language::French);
    }

    #[test]
    fn unknown_options_are_discarded_without_losing_current_view_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let retired = serde_json::json!({
            "font_size": 22,
            "layout": "dual",
            "group_by_date": true,
            "first_launch": false,
            "trash_drive_settings": [{"drive_path": "C:\\", "max_size_mb": 128, "use_trash": true}],
            "future_option": {"enabled": true},
        });
        let current = serde_json::json!({
            "workspace_state": {"sidebar_collapsed": true, "layout": {"kind": "pane"}},
            "duplicate_search_options": [true, false, true],
            "duplicate_search_window_size": [900.0, 650.0],
        });
        let defaults = serde_json::to_value(AppConfig::default()).unwrap();
        for (key, _) in retired.as_object().unwrap() {
            assert!(defaults.get(key).is_none());
        }
        let mut input = retired.clone();
        input.as_object_mut().unwrap().extend(current.as_object().unwrap().clone());
        fs::write(&path, serde_json::to_vec(&input).unwrap()).unwrap();
        let mut config = ConfigManager::read_from_path(&path).unwrap();
        assert_eq!(config.ui_rem_size, 16);
        config.ui_rem_size = 24;
        config.theme = "light".into();

        for reset in [false, true] {
            if reset {
                config.reset_preferences();
            }
            ConfigManager::write_to_path(&path, &config).unwrap();
            let written: serde_json::Value =
                serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            for (key, _) in retired.as_object().unwrap() {
                assert!(written.get(key).is_none());
            }
            for (key, value) in current.as_object().unwrap() {
                assert_eq!(written.get(key), Some(value));
            }
            config = ConfigManager::read_from_path(&path).unwrap();
            assert_eq!(config.ui_rem_size, if reset { 16 } else { 24 });
            assert_eq!(config.theme, if reset { "dark" } else { "light" });
        }
    }

    #[test]
    fn preferences_survive_reload_without_discarding_other_options() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, r#"{"theme":"oxyde-dark","language":"fr","search_limit":123,"duplicate_search_options":[false,true,true]}"#).unwrap();
        let mut config = ConfigManager::read_from_path(&path).unwrap();
        config.operations_panel_position = Some([42.5, 87.25]);
        config.ui_rem_size = 20;
        config.date_format = "ISO".into();
        for (theme, language) in [
            (ThemeId::Light, Language::English),
            (ThemeId::Dark, Language::French),
            (ThemeId::Light, Language::German),
            (ThemeId::Dark, Language::Spanish),
            (ThemeId::Dark, Language::Italian),
        ] {
            config.theme = theme.as_str().into();
            config.language = language.as_str().into();
            config.use_system_icons = theme == ThemeId::Light;
            config.show_checkboxes = theme == ThemeId::Light;
            config.show_network = theme == ThemeId::Light;
            config.sync_sidebar_with_path = theme == ThemeId::Light;
            config.default_turbo_mode = theme == ThemeId::Light;
            ConfigManager::write_to_path(&path, &config).unwrap();
            config = ConfigManager::read_from_path(&path).unwrap();
            assert_eq!(ThemeId::from_id(&config.theme), theme);
            assert_eq!(Language::from_id(&config.language), language);
            assert_eq!(config.search_limit, 123);
            assert_eq!(config.default_turbo_mode, theme == ThemeId::Light);
            assert_eq!(config.use_system_icons, theme == ThemeId::Light);
            assert_eq!(config.show_checkboxes, theme == ThemeId::Light);
            assert_eq!(config.sync_sidebar_with_path, theme == ThemeId::Light);
            assert_eq!(config.ui_rem_size, 20);
            assert_eq!(config.date_format, "ISO");
            assert_eq!(config.show_network, theme == ThemeId::Light);
            assert_eq!(config.operations_panel_position, Some([42.5, 87.25]));
            assert_eq!(config.duplicate_search_options, Some(serde_json::json!([false, true, true])));
        }
    }

    #[test]
    fn corrupt_config_is_reported_and_left_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, "{broken").unwrap();
        assert!(ConfigManager::read_from_path(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{broken");
        assert_eq!(
            ConfigManager::read_from_path(&dir.path().join("missing"))
                .unwrap()
                .language,
            crate::models::preferences::Language::from_system_locale(
                crate::platform::platform().interface_locale().as_deref()
            ).as_str()
        );
    }
}
